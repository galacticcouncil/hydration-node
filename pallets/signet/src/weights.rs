#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions for this pallet
pub trait WeightInfo {
    fn initialize() -> Weight;
    fn emit_custom_event() -> Weight;
}

/// For tests - just returns simple weights
impl WeightInfo for () {
    fn initialize() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    
    fn emit_custom_event() -> Weight {
        Weight::from_parts(5_000_000, 0)
    }
}