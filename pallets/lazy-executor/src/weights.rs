#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_staking.
pub trait WeightInfo {
	fn process_queue_base_weight() -> Weight;
}

/// Weights for pallet_staking using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	fn process_queue_base_weight() -> Weight {
		Weight::from_parts(1_000, 2_000)
	}
}
