pub trait Volume<Balance> {
	fn amount_in(&self) -> Balance;
	fn amount_out(&self) -> Balance;
	fn liquidity(&self) -> Balance;
	fn updated_at(&self) -> u128;
}

pub trait VolumeProvider<AssetId, Balance> {
	type Volume: Volume<Balance>;

	fn last_entry(asset_id: AssetId) -> Option<Self::Volume>;
	fn period() -> u64;
}
