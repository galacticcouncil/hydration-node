use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_solution() -> Weight;
	fn set_protocol_fee() -> Weight;
}

impl WeightInfo for () {
	fn submit_solution() -> Weight {
		Weight::default()
	}

	fn set_protocol_fee() -> Weight {
		Weight::default()
	}
}
