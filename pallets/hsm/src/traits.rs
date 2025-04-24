#[cfg(feature = "runtime-benchmarks")]
use sp_runtime::DispatchResult;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
	fn bind_address(account: AccountId) -> DispatchResult;
	// Must register as Erc20 asset in registry with some contract address
	fn set_hollar_as_erc20() -> DispatchResult;
	// for benchmarking, we dont use real evm executor,
	// sp we need to fool the currency balance provider little bit
	fn set_hollar_as_token() -> DispatchResult;
}
