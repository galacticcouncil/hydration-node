// SPDX-License-Identifier: Apache-2.0

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn giga_stake() -> Weight;
	fn giga_unstake() -> Weight;
	fn unlock() -> Weight;
	fn set_pool_contract() -> Weight;
	fn cancel_unstake() -> Weight;
}

impl WeightInfo for () {
	fn giga_stake() -> Weight {
		Weight::zero()
	}
	fn giga_unstake() -> Weight {
		Weight::zero()
	}
	fn unlock() -> Weight {
		Weight::zero()
	}
	fn set_pool_contract() -> Weight {
		Weight::zero()
	}
	fn cancel_unstake() -> Weight {
		Weight::zero()
	}
}
