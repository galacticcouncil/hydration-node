use frame_support::weights::Weight;

pub trait WeightInfo {
	fn claim_rewards() -> Weight;
	fn drain_stuck_rewards() -> Weight;
}

/// Stub weights for development.
impl WeightInfo for () {
	fn claim_rewards() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn drain_stuck_rewards() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
}
