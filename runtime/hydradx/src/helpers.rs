#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_helpers {
	use crate::{EVMAccounts, Runtime, RuntimeOrigin, Tokens};
	use evm::ExitRevert::Reverted;
	use evm::{ExitReason, ExitSucceed};
	use hydradx_traits::evm::{CallContext, InspectEvmAccounts};
	use orml_traits::MultiCurrencyExtended;
	use pallet_hsm::ERC20Function;
	use primitive_types::U256;
	use primitives::{AccountId, Balance, EvmAddress};
	use sp_runtime::{DispatchResult};
	use sp_std::prelude::*;

	pub struct HsmBenchmarkHelper;

	impl pallet_hsm::traits::BenchmarkHelper<AccountId> for HsmBenchmarkHelper {
		fn bind_address(account: AccountId) -> DispatchResult {
			EVMAccounts::bind_evm_address(RuntimeOrigin::signed(account))
		}
	}

	pub struct DummyEvmForHsm;
	impl hydradx_traits::evm::EVM<pallet_hsm::types::CallResult> for DummyEvmForHsm {
		fn call(context: CallContext, data: Vec<u8>, _value: U256, _gas: u64) -> pallet_hsm::types::CallResult {
			// For the HSM benchmarks - since we mock the evm executor here, we still need to update balances of HOLLAR
			if data.len() >= 4 {
				let function_bytes: [u8; 4] = data[0..4].try_into().unwrap_or([0; 4]);
				let function_u32 = u32::from_be_bytes(function_bytes);

				if let Ok(function) = ERC20Function::try_from(function_u32) {
					match function {
						ERC20Function::Mint => {
							// Should include recipient (32 bytes) and amount (32 bytes) parameters after the 4-byte selector
							if data.len() >= 4 + 32 + 32 {
								// Extract recipient address (padded to 32 bytes in ABI encoding)
								let recipient_bytes: [u8; 32] = data[4..4 + 32].try_into().unwrap_or([0; 32]);
								let recipient_evm = EvmAddress::from_slice(&recipient_bytes[12..32]);

								// Extract amount (32 bytes)
								let amount_bytes: [u8; 32] = data[4 + 32..4 + 64].try_into().unwrap_or([0; 32]);
								let amount = U256::from_big_endian(&amount_bytes);

								// Convert to Balance and account IDs for our operation
								if let Ok(amount) = Balance::try_from(amount) {
									let recipient = EVMAccounts::account_id(recipient_evm);
									let hollar_id = <Runtime as pallet_hsm::Config>::HollarId::get();

									// Increase the balance of the recipient
									let _ = Tokens::update_balance(hollar_id, &recipient, amount as i128);

									return (ExitReason::Succeed(ExitSucceed::Stopped), vec![]);
								}
							}
						}
						ERC20Function::Burn => {
							// Should include amount (32 bytes) parameter after the 4-byte selector
							if data.len() >= 4 + 32 {
								// Extract amount (32 bytes)
								let amount_bytes: [u8; 32] = data[4..4 + 32].try_into().unwrap_or([0; 32]);
								let amount = U256::from_big_endian(&amount_bytes);

								// Convert to Balance and account IDs for our operation
								if let Ok(amount) = Balance::try_from(amount) {
									let sender = context.sender;
									let account_id = EVMAccounts::account_id(sender);
									let hollar_id = <Runtime as pallet_hsm::Config>::HollarId::get();

									// Decrease the balance of the caller
									let _ = Tokens::update_balance(hollar_id, &account_id, -(amount as i128));

									return (ExitReason::Succeed(ExitSucceed::Stopped), vec![]);
								}
							}
						}
					}
				}
			}

			(ExitReason::Revert(Reverted), vec![])
		}

		fn view(_context: CallContext, _data: Vec<u8>, _gas: u64) -> pallet_hsm::types::CallResult {
			(ExitReason::Succeed(ExitSucceed::Stopped), vec![])
		}
	}
}
