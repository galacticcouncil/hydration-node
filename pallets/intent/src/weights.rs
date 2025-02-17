use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_intent() -> Weight;
}

impl WeightInfo for () {
	fn submit_intent() -> Weight {
		Weight::default()
	}
}
