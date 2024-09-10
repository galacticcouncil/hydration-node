use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_intent() -> Weight;
	fn submit_solution() -> Weight;
	fn execute_solution() -> Weight;
}

impl WeightInfo for () {
	fn submit_intent() -> Weight {
		Weight::default()
	}

	fn submit_solution() -> Weight {
		Weight::default()
	}

	fn execute_solution() -> Weight {
		Weight::default()
	}
}
