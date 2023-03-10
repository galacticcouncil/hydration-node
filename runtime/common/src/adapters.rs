use core::marker::PhantomData;

use frame_support::{traits::Get, weights::Weight};
use hydradx_traits::{OnLiquidityChangedHandler, OnTradeHandler};
use pallet_ema_oracle::OnActivityHandler;
use pallet_omnipool::traits::{AssetInfo, OmnipoolHooks};
use primitives::{AssetId, Balance};
use sp_runtime::DispatchError;

/// Passes on trade and liquidity data from the omnipool to the oracle.
pub struct OmnipoolHookAdapter<Lrna, Runtime>(PhantomData<(Lrna, Runtime)>);

/// The source of the data for the oracle.
pub const OMNIPOOL_SOURCE: [u8; 8] = *b"omnipool";

impl<Lrna, Runtime> OmnipoolHooks<AssetId, Balance> for OmnipoolHookAdapter<Lrna, Runtime>
where
	Lrna: Get<AssetId>,
	Runtime: pallet_ema_oracle::Config,
{
	type Error = DispatchError;

	fn on_liquidity_changed(asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_liquidity_changed(
			OMNIPOOL_SOURCE,
			asset.asset_id,
			Lrna::get(),
			*asset.delta_changes.delta_reserve,
			*asset.delta_changes.delta_hub_reserve,
			asset.after.reserve,
			asset.after.hub_reserve,
		)
		.map_err(|(_, e)| e)
	}

	fn on_trade(
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

		Ok(weight1.saturating_add(weight2))
	}

	fn on_liquidity_changed_weight() -> Weight {
		OnActivityHandler::<Runtime>::on_liquidity_changed_weight()
	}

	fn on_trade_weight() -> Weight {
		OnActivityHandler::<Runtime>::on_trade_weight().saturating_mul(2)
	}
}
