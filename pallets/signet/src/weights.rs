#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions for this pallet
pub trait WeightInfo {
    fn initialize() -> Weight;
    fn update_deposit() -> Weight;
    fn withdraw_funds() -> Weight;
    fn sign() -> Weight;
    fn sign_respond() -> Weight;
    fn respond(r: u32) -> Weight;  // r = number of responses
    fn respond_error(e: u32) -> Weight;  // e = number of errors
    fn read_respond() -> Weight;
    fn get_signature_deposit() -> Weight;
}

/// For tests - just returns simple weights
impl WeightInfo for () {
    fn initialize() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    
    fn update_deposit() -> Weight {
        Weight::from_parts(8_000_000, 0)
    }
    
    fn withdraw_funds() -> Weight {
        Weight::from_parts(35_000_000, 0)
    }
    
    fn sign() -> Weight {
        Weight::from_parts(45_000_000, 0)
    }
    
    fn sign_respond() -> Weight {
        Weight::from_parts(50_000_000, 0)  // Slightly more than sign
    }
    
    fn respond(r: u32) -> Weight {
        // Base weight + per-response weight
        Weight::from_parts(10_000_000, 0)
            .saturating_add(Weight::from_parts(1_000_000, 0).saturating_mul(r.into()))
    }
    
    fn respond_error(e: u32) -> Weight {
        // Base weight + per-error weight
        Weight::from_parts(10_000_000, 0)
            .saturating_add(Weight::from_parts(500_000, 0).saturating_mul(e.into()))
    }
    
    fn read_respond() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    
    fn get_signature_deposit() -> Weight {
        Weight::from_parts(5_000_000, 0)
    }
}