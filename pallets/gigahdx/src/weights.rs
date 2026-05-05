// SPDX-License-Identifier: Apache-2.0

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn giga_stake() -> Weight;
	fn giga_unstake() -> Weight;
	fn unlock() -> Weight;
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
}
