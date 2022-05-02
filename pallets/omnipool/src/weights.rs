use frame_support::weights::Weight;

/// Weight functions needed for claims.
pub trait WeightInfo {
	fn initialize_pool() -> Weight;
	fn add_token() -> Weight;
	fn add_liquidity() -> Weight;
	fn remove_liquidity() -> Weight;
	fn sell() -> Weight;
	fn buy() -> Weight;
	fn set_asset_tradable_state() -> Weight;
}

impl WeightInfo for () {
	fn initialize_pool() -> Weight {
		0
	}

	fn add_token() -> Weight {
		0
	}

	fn add_liquidity() -> Weight {
		0
	}

	fn remove_liquidity() -> Weight {
		0
	}

	fn sell() -> Weight {
		0
	}
	fn buy() -> Weight {
		0
	}
	fn set_asset_tradable_state() -> Weight {
		0
	}
}
