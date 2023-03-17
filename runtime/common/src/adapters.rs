use core::marker::PhantomData;

use frame_support::{traits::Get, weights::Weight};
use hydra_dx_math::omnipool::types::BalanceUpdate;
use hydradx_traits::{OnLiquidityChangedHandler, OnTradeHandler};
use pallet_ema_oracle::OnActivityHandler;
use pallet_omnipool::traits::{AssetInfo, OmnipoolHooks};
use primitives::{AssetId, Balance};
use sp_runtime::DispatchError;
/// Passes on trade and liquidity data from the omnipool to the oracle.
pub struct OmnipoolHookAdapter<Origin, Lrna, Runtime>(PhantomData<(Origin, Lrna, Runtime)>);

/// The source of the data for the oracle.
pub const OMNIPOOL_SOURCE: [u8; 8] = *b"omnipool";

impl<Origin, Lrna, Runtime> OmnipoolHooks<Origin, AssetId, Balance> for OmnipoolHookAdapter<Origin, Lrna, Runtime>
where
	Lrna: Get<AssetId>,
	Runtime: pallet_ema_oracle::Config + pallet_circuit_breaker::Config,
{
	type Error = DispatchError;

	fn on_liquidity_changed(_origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		let weight1 = OnActivityHandler::<Runtime>::on_liquidity_changed(
			OMNIPOOL_SOURCE,
			asset.asset_id,
			Lrna::get(),
			*asset.delta_changes.delta_reserve,
			*asset.delta_changes.delta_hub_reserve,
			asset.after.reserve,
			asset.after.hub_reserve,
		)
		.map_err(|(_, e)| e)?;

		let weight2 = match asset.delta_changes.delta_reserve.into() {
			BalanceUpdate::Increase(amount) => pallet_circuit_breaker::Pallet::<Runtime>::ensure_add_liquidity_limit(
				asset.asset_id.into(),
				asset.before.reserve.into(),
				amount.into(),
			)?,
			BalanceUpdate::Decrease(amount) => {
				pallet_circuit_breaker::Pallet::<Runtime>::ensure_remove_liquidity_limit(
					asset.asset_id.into(),
					asset.before.reserve.into(),
					amount.into(),
				)?
			}
		};

		Ok(weight1.saturating_add(weight2))
	}

	fn on_trade(
		_origin: Origin,
		asset_in: AssetInfo<AssetId, Balance>,
		asset_out: AssetInfo<AssetId, Balance>,
	) -> Result<Weight, Self::Error> {
		let weight1 = OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			asset_in.asset_id,
			Lrna::get(),
			*asset_in.delta_changes.delta_reserve,
			*asset_in.delta_changes.delta_hub_reserve,
			asset_in.after.reserve,
			asset_in.after.hub_reserve,
		)
		.map_err(|(_, e)| e)?;

		let weight2 = OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			Lrna::get(),
			asset_out.asset_id,
			*asset_out.delta_changes.delta_hub_reserve,
			*asset_out.delta_changes.delta_reserve,
			asset_out.after.hub_reserve,
			asset_out.after.reserve,
		)
		.map_err(|(_, e)| e)?;

		let amount_in = match asset_in.delta_changes.delta_reserve.into() {
			BalanceUpdate::Increase(am) => am,
			BalanceUpdate::Decrease(am) => am,
		};

		let amount_out = match asset_out.delta_changes.delta_reserve.into() {
			BalanceUpdate::Increase(am) => am,
			BalanceUpdate::Decrease(am) => am,
		};

		let weight3 = pallet_circuit_breaker::Pallet::<Runtime>::ensure_pool_state_change_limit(
			asset_in.asset_id.into(),
			asset_in.before.reserve.into(),
			amount_in.into(),
			asset_out.asset_id.into(),
			asset_out.before.reserve.into(),
			amount_out.into(),
		)?;

		Ok(weight1.saturating_add(weight2).saturating_add(weight3))
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
		.map_err(|(_, e)| e)
	}

	fn on_liquidity_changed_weight() -> Weight {
		OnActivityHandler::<Runtime>::on_liquidity_changed_weight()
	}

	fn on_trade_weight() -> Weight {
		OnActivityHandler::<Runtime>::on_trade_weight().saturating_mul(2)
	}
}
