use sp_runtime::DispatchResult;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
	fn bind_address(account: AccountId) -> DispatchResult;
}
