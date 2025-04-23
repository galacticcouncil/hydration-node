#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_helpers {
	use crate::EVMAccounts;
	use hydradx_traits::evm::CallContext;
	use primitive_types::U256;
	use primitives::AccountId;
	use sp_runtime::DispatchResult;

	pub struct HsmBenchmarkHelper;

	impl pallet_hsm::traits::BenchmarkHelper<AccountId> for HsmBenchmarkHelper {
		fn bind_address(account: AccountId) -> DispatchResult {
			EVMAccounts::bind_evm_address(crate::RuntimeOrigin::signed(account))
		}
	}

	pub struct DummyEvmForHsm;
	impl hydradx_traits::evm::EVM<pallet_hsm::types::CallResult> for DummyEvmForHsm {
		fn call(_context: CallContext, _data: Vec<u8>, _value: U256, _gas: u64) -> pallet_hsm::types::CallResult {
			(
				pallet_evm::ExitReason::Succeed(pallet_evm::ExitSucceed::Stopped),
				vec![],
			)
		}

		fn view(_context: CallContext, _data: Vec<u8>, _gas: u64) -> pallet_hsm::types::CallResult {
			(
				pallet_evm::ExitReason::Succeed(pallet_evm::ExitSucceed::Stopped),
				vec![],
			)
		}
	}
}
