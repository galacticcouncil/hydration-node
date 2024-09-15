use frame_support::weights::Weight;

pub trait IceWeightBounds<RuntimeCall> {
	fn transfer_weight() -> Result<Weight, ()>;
	fn swap_weight() -> Result<Weight, ()>;
	fn call_weight(call: &RuntimeCall) -> Result<Weight, ()>;
}

impl<RuntimeCall> IceWeightBounds<RuntimeCall> for () {
	fn transfer_weight() -> Result<Weight, ()> {
		Ok(Weight::from(0))
	}

	fn swap_weight() -> Result<Weight, ()> {
		Ok(Weight::from(0))
	}

	fn call_weight(_call: &RuntimeCall) -> Result<Weight, ()> {
		Ok(Weight::from(0))
	}
}
