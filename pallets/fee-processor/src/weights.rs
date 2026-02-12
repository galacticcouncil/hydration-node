use frame_support::weights::Weight;

pub trait WeightInfo {
	fn convert() -> Weight;
}

impl WeightInfo for () {
	fn convert() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
}
