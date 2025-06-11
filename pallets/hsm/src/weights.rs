#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_hsm.
pub trait WeightInfo {
	fn add_collateral_asset() -> Weight;
	fn remove_collateral_asset() -> Weight;
	fn update_collateral_asset() -> Weight;
	fn set_flash_minter() -> Weight;
	fn sell() -> Weight;
	fn buy() -> Weight;
	fn execute_arbitrage() -> Weight;
	fn on_finalize() -> Weight;
	fn calculate_sell() -> Weight;
	fn calculate_buy() -> Weight;
	fn calculate_spot_price_with_fee() -> Weight;
}
/// Default weights
#[cfg(test)]
impl WeightInfo for () {
	fn add_collateral_asset() -> Weight {
		Weight::zero()
	}

	fn remove_collateral_asset() -> Weight {
		Weight::zero()
	}

	fn update_collateral_asset() -> Weight {
		Weight::zero()
	}

	fn set_flash_minter() -> Weight {
		Weight::zero()
	}

	fn sell() -> Weight {
		Weight::zero()
	}

	fn buy() -> Weight {
		Weight::zero()
	}

	fn execute_arbitrage() -> Weight {
		Weight::zero()
	}

	fn on_finalize() -> Weight {
		Weight::zero()
	}

	fn calculate_sell() -> Weight {
		Weight::zero()
	}

	fn calculate_buy() -> Weight {
		Weight::zero()
	}

	fn calculate_spot_price_with_fee() -> Weight {
		Weight::zero()
	}
}
