pub trait PriceProvider<AssetId> {
	type Price;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price>;
}
