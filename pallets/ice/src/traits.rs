use frame_support::weights::Weight;

pub trait IceWeightBounds<RuntimeCall> {
	fn transfer_weight() -> Weight;
	fn swap_weight() -> Weight;
	fn call_weight(call: &RuntimeCall) -> Weight;
}

impl<RuntimeCall> IceWeightBounds<RuntimeCall> for () {
	fn transfer_weight() -> Weight {
		Weight::from(0)
	}

	fn swap_weight() -> Weight {
		Weight::from(0)
	}

	fn call_weight(_call: &RuntimeCall) -> Weight {
		Weight::from(0)
	}
}
