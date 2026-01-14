use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_intent() -> Weight;
	fn cancel_intent() -> Weight;
	fn cleanup_intent() -> Weight;
}

impl WeightInfo for () {
	fn submit_intent() -> Weight {
		Weight::default()
	}

	fn cancel_intent() -> Weight {
		Weight::default()
	}

	fn cleanup_intent() -> Weight {
		Weight::default()
	}
}
