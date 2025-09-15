use frame_support::weights::Weight;
use sp_runtime::traits::Get;
use sp_std::marker::PhantomData;

pub use pallet_signet::weights::*;

pub struct HydraWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> pallet_signet::weights::WeightInfo for HydraWeight<T> {
    fn emit_custom_event() -> Weight {
        Weight::from_parts(2_500_000, 0)
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
}