use frame_support::weights::Weight;

pub trait IceWeightBounds<RuntimeCall, Route> {
	fn transfer_weight() -> Weight;
	fn swap_weight(route: &Route) -> Weight;
	fn call_weight(call: &RuntimeCall) -> Weight;
}

impl<RuntimeCall, Route> IceWeightBounds<RuntimeCall, Route> for () {
	fn transfer_weight() -> Weight {
		Weight::from(0)
	}

	fn swap_weight(_route: &Route) -> Weight {
		Weight::from(0)
	}

	fn call_weight(_call: &RuntimeCall) -> Weight {
		Weight::from(0)
	}
}
