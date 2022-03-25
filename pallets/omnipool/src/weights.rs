use frame_support::weights::Weight;

/// Weight functions needed for claims.
pub trait WeightInfo {
	fn add_token() -> Weight;
	fn add_liquidity() -> Weight;
}

impl WeightInfo for () {
	fn add_token() -> Weight {
		0 as Weight
	}

	fn add_liquidity() -> Weight {
		0 as Weight
	}
}
