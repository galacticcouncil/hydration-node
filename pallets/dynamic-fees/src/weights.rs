use frame_support::weights::Weight;

pub trait WeightInfo {
	fn set_asset_fee() -> Weight;
	fn remove_asset_fee() -> Weight;
}

/// Default weights for tests.
#[cfg(test)]
impl WeightInfo for () {
	fn set_asset_fee() -> Weight {
		Weight::zero()
	}

	fn remove_asset_fee() -> Weight {
		Weight::zero()
	}
}
