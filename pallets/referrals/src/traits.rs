pub use hydradx_traits::gigahdx::Convert;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId, Balance> {
	// Should prepare everything that provides price for selected asset
	// Amount returned is minted into pot account in benchmarks.
	fn prepare_convertible_asset_and_amount() -> (AssetId, Balance);
}
