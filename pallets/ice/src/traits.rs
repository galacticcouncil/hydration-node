use frame_support::weights::Weight;

pub trait IceWeightBounds<RuntimeCall, Route> {
	fn transfer_weight() -> Weight;
	fn sell_weight(route: Route) -> Weight;
	fn buy_weight(route: Route) -> Weight;
	fn call_weight(call: &RuntimeCall) -> Weight;
}

impl<RuntimeCall, Route> IceWeightBounds<RuntimeCall, Route> for () {
	fn transfer_weight() -> Weight {
		Weight::zero()
	}

	fn sell_weight(_route: Route) -> Weight {
		Weight::zero()
	}

	fn buy_weight(_route: Route) -> Weight {
		Weight::zero()
	}

	fn call_weight(_call: &RuntimeCall) -> Weight {
		Weight::zero()
	}
}
