use crate::types::AssetReserveState;
use hydra_dx_math::omnipool::types::AssetStateChange;
use sp_runtime::DispatchError;

pub struct AssetInfo<AssetId, Balance>
where
	Balance: Default + Clone,
{
	pub asset_id: AssetId,
	pub before: AssetReserveState<Balance>,
	pub after: AssetReserveState<Balance>,
	pub delta_changes: AssetStateChange<Balance>,
}

impl<AssetId, Balance> AssetInfo<AssetId, Balance>
where
	Balance: Default + Clone,
{
	pub fn new(
		asset_id: AssetId,
		before_state: &AssetReserveState<Balance>,
		after_state: &AssetReserveState<Balance>,
		delta_changes: &AssetStateChange<Balance>,
	) -> Self {
		Self {
			asset_id,
			before: (*before_state).clone(),
			after: (*after_state).clone(),
			delta_changes: (*delta_changes).clone(),
		}
	}
}

pub trait OmnipoolHooks<AssetId, Balance>
where
	Balance: Default + Clone,
{
	type Error;
	fn on_liquidity_changed(asset: AssetInfo<AssetId, Balance>) -> Result<(), Self::Error>;
	fn on_trade(
		asset_in: AssetInfo<AssetId, Balance>,
		asset_out: AssetInfo<AssetId, Balance>,
	) -> Result<(), Self::Error>;
}

impl<AssetId, Balance> OmnipoolHooks<AssetId, Balance> for ()
where
	Balance: Default + Clone,
{
	type Error = DispatchError;

	fn on_liquidity_changed(_: AssetInfo<AssetId, Balance>) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_trade(_: AssetInfo<AssetId, Balance>, _: AssetInfo<AssetId, Balance>) -> Result<(), Self::Error> {
		Ok(())
	}
}
