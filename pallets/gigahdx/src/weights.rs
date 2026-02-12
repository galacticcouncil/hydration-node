use frame_support::weights::Weight;

pub trait WeightInfo {
	fn giga_stake() -> Weight;
	fn giga_unstake() -> Weight;
	fn unlock() -> Weight;
}

/// Stub weights for development. Replace with benchmarked weights.
impl WeightInfo for () {
	fn giga_stake() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn giga_unstake() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn unlock() -> Weight {
		Weight::from_parts(50_000_000, 0)
	}
}
