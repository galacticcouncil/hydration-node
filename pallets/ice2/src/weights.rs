use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_solution() -> Weight;
}

impl WeightInfo for () {
	fn submit_solution() -> Weight {
		Weight::default()
	}
}
