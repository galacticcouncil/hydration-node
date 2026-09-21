use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_solution() -> Weight;
	fn set_protocol_fee() -> Weight;
	fn set_solver_mode() -> Weight;
	fn update_routing() -> Weight;
	fn price_derivation() -> Weight;
}

impl WeightInfo for () {
	fn submit_solution() -> Weight {
		Weight::default()
	}

	fn set_protocol_fee() -> Weight {
		Weight::default()
	}

	fn set_solver_mode() -> Weight {
		Weight::default()
	}

	fn update_routing() -> Weight {
		Weight::default()
	}

	fn price_derivation() -> Weight {
		Weight::default()
	}
}
