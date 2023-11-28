pub trait Convert<AccountId, AssetId, Balance> {
	type Error;

	fn convert(who: AccountId, asset_from: AssetId, asset_to: AssetId, amount: Balance)
		-> Result<Balance, Self::Error>;
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId, Balance> {
	fn prepare_convertible_asset_and_amount() -> (AssetId, Balance);
}
