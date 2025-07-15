use frame_support::weights::Weight;

pub trait WeightInfo {
	fn set_asset_fee_config() -> Weight;
	fn remove_asset_fee_config() -> Weight;
}

/// Default weights for tests.
#[cfg(test)]
impl WeightInfo for () {
	fn set_asset_fee_config() -> Weight {
		Weight::zero()
	}

	fn remove_asset_fee_config() -> Weight {
		Weight::zero()
	}
}
