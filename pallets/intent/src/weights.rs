use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_intent() -> Weight;
	fn remove_intent() -> Weight;
	fn cleanup_intent() -> Weight;
}

impl WeightInfo for () {
	fn submit_intent() -> Weight {
		Weight::default()
	}

	fn remove_intent() -> Weight {
		Weight::default()
	}

	fn cleanup_intent() -> Weight {
		Weight::default()
	}
}
