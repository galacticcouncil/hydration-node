use core::marker::PhantomData;

use frame_support::{traits::Get, weights::Weight};
use hydra_dx_math::omnipool::types::BalanceUpdate;
use hydradx_traits::AggregatedPriceOracle;
use hydradx_traits::{liquidity_mining::PriceAdjustment, OnLiquidityChangedHandler, OnTradeHandler, OraclePeriod};
use pallet_circuit_breaker::WeightInfo;
use pallet_ema_oracle::OnActivityHandler;
use pallet_ema_oracle::Price;
use pallet_omnipool::traits::{AssetInfo, ExternalPriceProvider, OmnipoolHooks};
use primitives::{AssetId, Balance};
use sp_runtime::traits::Zero;
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128};
use warehouse_liquidity_mining::GlobalFarmData;

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

/// Passes ema oracle price to the omnipool.
pub struct EmaOraclePriceAdapter<Period, Runtime>(PhantomData<(Period, Runtime)>);

impl<Period, Runtime> ExternalPriceProvider<AssetId, Price> for EmaOraclePriceAdapter<Period, Runtime>
where
	Period: Get<OraclePeriod>,
	Runtime: pallet_ema_oracle::Config + pallet_omnipool::Config,
{
	type Error = DispatchError;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<Price, Self::Error> {
		let (price, _) =
			pallet_ema_oracle::Pallet::<Runtime>::get_price(asset_a, asset_b, Period::get(), OMNIPOOL_SOURCE)
				.map_err(|_| pallet_omnipool::Error::<Runtime>::PriceDifferenceTooHigh)?;
		Ok(price)
	}

	fn get_price_weight() -> Weight {
		pallet_ema_oracle::Pallet::<Runtime>::get_price_weight()
	}
}

pub struct PriceAdjustmentAdapter<Runtime, LMInstance>(PhantomData<(Runtime, LMInstance)>);

impl<Runtime, LMInstance> PriceAdjustment<GlobalFarmData<Runtime, LMInstance>>
	for PriceAdjustmentAdapter<Runtime, LMInstance>
where
	Runtime: warehouse_liquidity_mining::Config<LMInstance>
		+ pallet_ema_oracle::Config
		+ pallet_omnipool_liquidity_mining::Config,
{
	type Error = DispatchError;
	type PriceAdjustment = FixedU128;

	fn get(global_farm: &GlobalFarmData<Runtime, LMInstance>) -> Result<Self::PriceAdjustment, Self::Error> {
		let (price, _) = pallet_ema_oracle::Pallet::<Runtime>::get_price(
			global_farm.reward_currency.into(),
			global_farm.incentivized_asset.into(), //LRNA
			OraclePeriod::TenMinutes,
			OMNIPOOL_SOURCE,
		)
		.map_err(|_| pallet_omnipool_liquidity_mining::Error::<Runtime>::PriceAdjustmentNotAvailable)?;

		FixedU128::checked_from_rational(price.n, price.d).ok_or(ArithmeticError::Overflow.into())
	}
}
