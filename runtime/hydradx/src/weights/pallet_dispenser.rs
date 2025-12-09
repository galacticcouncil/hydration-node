use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub use pallet_dispenser::weights::*;

pub struct HydraWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> pallet_dispenser::WeightInfo for HydraWeight<T> {
	fn initialize() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}

	fn request_fund() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}

	fn set_faucet_balance() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}

	fn unpause() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}

	fn pause() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}
}
