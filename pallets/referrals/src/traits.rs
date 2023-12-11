pub trait Convert<AccountId, AssetId, Balance> {
	type Error;

	fn convert(who: AccountId, asset_from: AssetId, asset_to: AssetId, amount: Balance)
		-> Result<Balance, Self::Error>;
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId, Balance> {
	// Should prepare everything that provides price for selected asset
	// Amount returned is minted into pot account in benchmarks.
	fn prepare_convertible_asset_and_amount() -> (AssetId, Balance);
}
