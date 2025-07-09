#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_helpers {
	use crate::{AssetRegistry, EVMAccounts, RegistryStrLimit, Runtime, RuntimeOrigin, Tokens};
	use evm::ExitRevert::Reverted;
	use evm::{ExitReason, ExitSucceed};
	use frame_support::storage::with_transaction;
	use frame_support::BoundedVec;
	use hydradx_traits::evm::{CallContext, InspectEvmAccounts};
	use hydradx_traits::{AssetKind, Create};
	use orml_traits::{MultiCurrency, MultiCurrencyExtended};
	use pallet_hsm::ERC20Function;
	use primitive_types::U256;
	use primitives::{AccountId, AssetId, Balance, EvmAddress};
	use sp_core::crypto::AccountId32;
	use sp_runtime::{DispatchResult, TransactionOutcome};
	use sp_std::prelude::*;
	use std::marker::PhantomData;

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
						ERC20Function::FlashLoan => {
							if data.len() >= 4 + 32 + 32 + 32 {
								// Extract recipient address (padded to 32 bytes in ABI encoding)
								let receiver: [u8; 32] = data[4..4 + 32].try_into().unwrap_or([0; 32]);
								let _receiver_evm = hydradx_traits::evm::EvmAddress::from_slice(&receiver[12..32]);

								let hollar: [u8; 32] = data[4 + 32..4 + 32 + 32].try_into().unwrap_or([0; 32]);
								let _hollar_evm = hydradx_traits::evm::EvmAddress::from_slice(&hollar[12..32]);

								let amount_bytes: [u8; 32] = data[4 + 32 + 32..4 + 32 + 32 + 32].try_into().unwrap();
								let amount = U256::from_big_endian(&amount_bytes);

								let arb_data = data[4 + 32 + 32 + 32 + 32 + 32..].to_vec();
								let arb_account: AccountId32 = receiver.into();
								let arb_evm = EVMAccounts::evm_address(&arb_account);
								let arb_account = EVMAccounts::account_id(arb_evm);
								let hollar_id = <Runtime as pallet_hsm::Config>::HollarId::get();
								let _ =
									Tokens::update_balance(hollar_id, &arb_account, amount.as_u128() as i128).unwrap();

								let alice_evm = EVMAccounts::evm_address(&arb_account);
								pallet_hsm::Pallet::<Runtime>::execute_arbitrage_with_flash_loan(
									alice_evm,
									amount.as_u128(),
									&arb_data,
								)
								.unwrap();
								let _ = Tokens::update_balance(hollar_id, &arb_account, -(amount.as_u128() as i128))
									.unwrap();
								return (ExitReason::Succeed(ExitSucceed::Returned), vec![]);
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

	#[cfg(feature = "runtime-benchmarks")]
	pub struct CircuitBreakerBenchmarkHelper<T>(PhantomData<T>);

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: pallet_circuit_breaker::Config> pallet_circuit_breaker::types::BenchmarkHelper<AccountId, AssetId, Balance>
		for CircuitBreakerBenchmarkHelper<T>
	{
		fn deposit(who: AccountId, asset_id: AssetId, amount: Balance) -> DispatchResult {
			Tokens::deposit(asset_id, &who, amount)
		}

		fn register_asset(asset_id: AssetId, deposit_limit: Balance) -> DispatchResult {
			let asset_name: BoundedVec<u8, RegistryStrLimit> = asset_id
				.to_le_bytes()
				.to_vec()
				.try_into()
				.map_err(|_| "BoundedConversionFailed")?;

			with_transaction(|| {
				TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
					Some(asset_id),
					Some(asset_name.clone()),
					AssetKind::Token,
					1,
					None,
					None,
					None,
					Some(deposit_limit),
				))
			})?;

			Ok(())
		}
	}
}
