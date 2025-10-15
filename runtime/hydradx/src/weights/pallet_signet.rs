use frame_support::weights::Weight;
use sp_runtime::traits::Get;
use sp_std::marker::PhantomData;

pub use pallet_signet::weights::*;

pub struct HydraWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> pallet_signet::weights::WeightInfo for HydraWeight<T> {
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
        Weight::from_parts(50_000_000, 0)
    }
    
    fn respond(r: u32) -> Weight {
        Weight::from_parts(10_000_000, 0)
            .saturating_add(Weight::from_parts(1_000_000, 0).saturating_mul(r.into()))
    }
    
    fn respond_error(e: u32) -> Weight {
        Weight::from_parts(10_000_000, 0)
            .saturating_add(Weight::from_parts(500_000, 0).saturating_mul(e.into()))
    }
    
    fn read_respond() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
}