use crate::types::{Balance, ResolvedIntent};
use frame_support::weights::Weight;
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::router::Trade;
//use serde::Deserialize;
use sp_runtime::traits::Bounded;
use sp_runtime::{FixedU128, Permill};

pub trait IceWeightBounds<RuntimeCall, Route> {
	fn transfer_weight() -> Weight;
	fn sell_weight(route: Route) -> Weight;
	fn buy_weight(route: Route) -> Weight;
	fn call_weight(call: &RuntimeCall) -> Weight;
}

impl<RuntimeCall, Route> IceWeightBounds<RuntimeCall, Route> for () {
	fn transfer_weight() -> Weight {
		Weight::zero()
	}

	fn sell_weight(_route: Route) -> Weight {
		Weight::zero()
	}

	fn buy_weight(_route: Route) -> Weight {
		Weight::zero()
	}

	fn call_weight(_call: &RuntimeCall) -> Weight {
		Weight::zero()
	}
}

pub trait Solver<Intent> {
	type Error;

	fn solve(intents: Vec<Intent>) -> Result<Vec<ResolvedIntent>, Self::Error>;
}

#[derive(Debug)]
pub struct OmnipoolAssetInfo<AssetId> {
	pub asset_id: AssetId,
	pub reserve: Balance,
	pub hub_reserve: Balance,
	pub decimals: u8,
	pub fee: Permill,
	pub hub_fee: Permill,
}

impl<AssetId> OmnipoolAssetInfo<AssetId> {
	pub fn reserve_as_f64(&self) -> f64 {
		FixedU128::from_rational(self.reserve, 10u128.pow(self.decimals as u32)).to_float()
	}

	pub fn hub_reserve_as_f64(&self) -> f64 {
		FixedU128::from_rational(self.hub_reserve, 10u128.pow(12u32)).to_float()
	}

	pub fn fee_as_f64(&self) -> f64 {
		FixedU128::from_rational(
			self.fee.deconstruct() as u128,
			Permill::max_value().deconstruct() as u128,
		)
		.to_float()
	}

	pub fn hub_fee_as_f64(&self) -> f64 {
		FixedU128::from_rational(
			self.hub_fee.deconstruct() as u128,
			Permill::max_value().deconstruct() as u128,
		)
		.to_float()
	}
	#[cfg(test)]
	pub fn reserve_no_decimals(&self) -> Balance {
		self.reserve / 10u128.pow(self.decimals as u32)
	}
	#[cfg(test)]
	pub fn hub_reserve_no_decimals(&self) -> Balance {
		self.hub_reserve / 10u128.pow(12u32)
	}
}

pub trait OmnipoolInfo<AssetId> {
	fn assets(filter: Option<Vec<AssetId>>) -> Vec<OmnipoolAssetInfo<AssetId>>;
}

pub trait Routing<AssetId> {
	fn get_route(asset_a: AssetId, asset_b: AssetId) -> Vec<Trade<AssetId>>;
	fn calculate_amount_out(route: &[Trade<AssetId>], amount_in: Balance) -> Result<Balance, ()>;
	fn calculate_amount_in(route: &[Trade<AssetId>], amount_out: Balance) -> Result<Balance, ()>;
	// should return price Hub/Asset
	fn hub_asset_price(asset: AssetId) -> Result<Ratio, ()>;
}