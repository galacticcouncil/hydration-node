use core::marker::PhantomData;

use frame_support::{traits::Get, weights::Weight};
use hydra_dx_math::ema::{round_u256_to_rational, EmaPrice};
use hydra_dx_math::omnipool::types::BalanceUpdate;
use hydra_dx_math::support::rational::Rounding;
use hydra_dx_math::types::Ratio;
use hydradx_traits::oracle::AggregatedPriceOracle;
use hydradx_traits::{NativePriceOracle, OnLiquidityChangedHandler, OnTradeHandler, OraclePeriod, PriceOracle};
use pallet_circuit_breaker::WeightInfo;
use pallet_ema_oracle::{OnActivityHandler, OracleError, Price};
use pallet_omnipool::traits::{AssetInfo, OmnipoolHooks};
use primitive_types::U128;
use primitives::{AssetId, Balance};
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
/// Passes on trade and liquidity data from the omnipool to the oracle.
pub struct OmnipoolHookAdapter<Origin, Lrna, Runtime>(PhantomData<(Origin, Lrna, Runtime)>);

/// The source of the data for the oracle.
pub const OMNIPOOL_SOURCE: [u8; 8] = *b"omnipool";

impl<Origin, Lrna, Runtime> OmnipoolHooks<Origin, AssetId, Balance> for OmnipoolHookAdapter<Origin, Lrna, Runtime>
where
	Lrna: Get<AssetId>,
	Runtime: pallet_ema_oracle::Config + pallet_circuit_breaker::Config + frame_system::Config<Origin = Origin>,
{
	type Error = DispatchError;

	fn on_liquidity_changed(origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_liquidity_changed(
			OMNIPOOL_SOURCE,
			asset.asset_id,
			Lrna::get(),
			*asset.delta_changes.delta_reserve,
			*asset.delta_changes.delta_hub_reserve,
			asset.after.reserve,
			asset.after.hub_reserve,
		)
		.map_err(|(_, e)| e)?;

		match asset.delta_changes.delta_reserve {
			BalanceUpdate::Increase(amount) => pallet_circuit_breaker::Pallet::<Runtime>::ensure_add_liquidity_limit(
				origin,
				asset.asset_id.into(),
				asset.before.reserve.into(),
				amount.into(),
			)?,
			BalanceUpdate::Decrease(amount) => {
				pallet_circuit_breaker::Pallet::<Runtime>::ensure_remove_liquidity_limit(
					origin,
					asset.asset_id.into(),
					asset.before.reserve.into(),
					amount.into(),
				)?
			}
		};

		Ok(Self::on_liquidity_changed_weight())
	}

	fn on_trade(
		_origin: Origin,
		asset_in: AssetInfo<AssetId, Balance>,
		asset_out: AssetInfo<AssetId, Balance>,
	) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			asset_in.asset_id,
			Lrna::get(),
			*asset_in.delta_changes.delta_reserve,
			*asset_in.delta_changes.delta_hub_reserve,
			asset_in.after.reserve,
			asset_in.after.hub_reserve,
		)
		.map_err(|(_, e)| e)?;

		OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			Lrna::get(),
			asset_out.asset_id,
			*asset_out.delta_changes.delta_hub_reserve,
			*asset_out.delta_changes.delta_reserve,
			asset_out.after.hub_reserve,
			asset_out.after.reserve,
		)
		.map_err(|(_, e)| e)?;

		let amount_in = *asset_in.delta_changes.delta_reserve;
		let amount_out = *asset_out.delta_changes.delta_reserve;

		pallet_circuit_breaker::Pallet::<Runtime>::ensure_pool_state_change_limit(
			asset_in.asset_id.into(),
			asset_in.before.reserve.into(),
			amount_in.into(),
			asset_out.asset_id.into(),
			asset_out.before.reserve.into(),
			amount_out.into(),
		)?;

		Ok(Self::on_trade_weight())
	}

	fn on_hub_asset_trade(_origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			Lrna::get(),
			asset.asset_id,
			*asset.delta_changes.delta_hub_reserve,
			*asset.delta_changes.delta_reserve,
			asset.after.hub_reserve,
			asset.after.reserve,
		)
		.map_err(|(_, e)| e)?;

		let amount_out = *asset.delta_changes.delta_reserve;

		pallet_circuit_breaker::Pallet::<Runtime>::ensure_pool_state_change_limit(
			Lrna::get().into(),
			Balance::zero().into(),
			Balance::zero().into(),
			asset.asset_id.into(),
			asset.before.reserve.into(),
			amount_out.into(),
		)?;

		Ok(Self::on_trade_weight())
	}

	fn on_liquidity_changed_weight() -> Weight {
		let w1 = OnActivityHandler::<Runtime>::on_liquidity_changed_weight();
		let w2 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_add_liquidity_limit()
			.max(<Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_remove_liquidity_limit());
		let w3 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::on_finalize_single(); // TODO: implement and use on_finalize_single_liquidity_limit_entry benchmark
		w1.saturating_add(w2).saturating_add(w3)
	}

	fn on_trade_weight() -> Weight {
		let w1 = OnActivityHandler::<Runtime>::on_trade_weight().saturating_mul(2);
		let w2 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_pool_state_change_limit();
		let w3 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::on_finalize_single(); // TODO: implement and use on_finalize_single_trade_limit_entry benchmark
		w1.saturating_add(w2).saturating_add(w3)
	}
}

pub struct OmnipoolPriceProviderAdapter<AssetId, Runtime, Lrna>(PhantomData<(AssetId, Runtime, Lrna)>);

impl<AssetId, Runtime, Lrna> PriceOracle<AssetId, EmaPrice> for OmnipoolPriceProviderAdapter<AssetId, Runtime, Lrna>
where
	Runtime: pallet_ema_oracle::Config,
	u32: From<AssetId>,
	Lrna: Get<AssetId>,
{
	//TODO: talk with Alex as we need to change the oracle to return one. Otherwise we return one here in the code
	fn price(asset_a: AssetId, asset_b: AssetId, period: OraclePeriod) -> Option<EmaPrice> {
		let price_asset_a_lrna = pallet_ema_oracle::Pallet::<Runtime>::get_price(
			asset_a.into(),
			Lrna::get().into(),
			period,
			OMNIPOOL_SOURCE,
		);

		let price_asset_a_lrna = match price_asset_a_lrna {
			Ok(price) => price.0,
			Err(OracleError::SameAsset) => EmaPrice::from(1),
			Err(_) => return None,
		};

		let price_lrna_asset_b = pallet_ema_oracle::Pallet::<Runtime>::get_price(
			Lrna::get().into(),
			asset_b.into(),
			period,
			OMNIPOOL_SOURCE,
		);

		let price_lrna_asset_b = match price_lrna_asset_b {
			Ok(price) => price.0,
			Err(OracleError::SameAsset) => EmaPrice::from(1),
			Err(_) => return None,
		};

		let nominator = U128::full_mul(price_asset_a_lrna.n.into(), price_lrna_asset_b.n.into());
		let denominator = U128::full_mul(price_asset_a_lrna.d.into(), price_lrna_asset_b.d.into());

		let price_in_ema_price = round_u256_to_rational((nominator, denominator), Rounding::Nearest);

		Some(price_in_ema_price)
	}
}
