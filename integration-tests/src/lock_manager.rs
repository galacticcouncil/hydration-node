#![cfg(test)]

use crate::polkadot_test_net::*;

use fp_evm::{Context, ExitSucceed, PrecompileOutput};
use hydradx_runtime::evm::precompiles::{handle::EvmDataWriter, Address, HydraDXPrecompiles, LOCK_MANAGER};
use hydradx_runtime::evm::ExtendedAddressMapping;
use hydradx_runtime::Runtime;
use pallet_evm::*;
use pretty_assertions::assert_eq;
use sp_core::{keccak_256, H160, U256};
use xcm_emulator::TestExt;

use crate::evm::MockHandle;

/// Compute the 4-byte selector for `getLockedBalance(address,address)`.
fn get_locked_balance_selector() -> u32 {
	let hash = keccak_256(b"getLockedBalance(address,address)");
	u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]])
}

/// Build call data for `getLockedBalance(address,address)`.
fn build_call_data(token: H160, account: H160) -> Vec<u8> {
	EvmDataWriter::new_with_selector(get_locked_balance_selector())
		.write(Address(token))
		.write(Address(account))
		.build()
}

/// Build a MockHandle targeting the LockManager precompile.
fn build_handle(caller: H160, data: Vec<u8>) -> MockHandle {
	MockHandle {
		input: data,
		context: Context {
			address: LOCK_MANAGER,
			caller,
			apparent_value: U256::zero(),
		},
		code_address: LOCK_MANAGER,
		is_static: true,
	}
}

/// Resolve an EVM H160 address to a Substrate AccountId using the same
/// mapping the runtime (and the precompile) uses.
fn account_id_of(addr: H160) -> primitives::AccountId {
	<ExtendedAddressMapping as AddressMapping<primitives::AccountId>>::into_account_id(addr)
}

/// Dummy token address (unused by precompile, but required by the ABI).
const TOKEN: H160 = H160([0u8; 20]);
const WRONG_TOKEN: H160 = H160([0xff; 20]);

#[test]
fn get_locked_balance_returns_zero_when_no_lock() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let caller = evm_address();
		let query_addr = evm_address2();
		let data = build_call_data(TOKEN, query_addr);
		let mut handle = build_handle(caller, data);

		let result = pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle);

		let expected = U256::zero().to_big_endian();
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);
	});
}

#[test]
fn get_locked_balance_returns_correct_value() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let query_addr = evm_address();
		let substrate_account = account_id_of(query_addr);
		let lock_amount: u128 = 500 * UNITS;

		// Seed the voting lock directly in storage.
		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&substrate_account, lock_amount);

		let data = build_call_data(TOKEN, query_addr);
		let mut handle = build_handle(query_addr, data);

		let result = pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle);

		let expected = U256::from(lock_amount).to_big_endian();
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);
	});
}

#[test]
fn get_locked_balance_after_lock_update() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let query_addr = evm_address();
		let substrate_account = account_id_of(query_addr);

		// Seed initial lock.
		let initial: u128 = 500 * UNITS;
		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&substrate_account, initial);

		let data = build_call_data(TOKEN, query_addr);
		let mut handle = build_handle(query_addr, data.clone());

		let result = pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle);

		let expected = U256::from(initial).to_big_endian();
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);

		// Update the lock to a higher value.
		let updated: u128 = 1_000 * UNITS;
		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&substrate_account, updated);

		let mut handle2 = build_handle(query_addr, data);
		let result2 = pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle2);

		let expected2 = U256::from(updated).to_big_endian();
		assert_eq!(
			result2,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected2.to_vec(),
			})
		);
	});
}

#[test]
fn get_locked_balance_works_for_different_accounts() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let alice_evm = evm_address();
		let bob_evm = evm_address2();
		let alice_sub = account_id_of(alice_evm);
		let bob_sub = account_id_of(bob_evm);

		let alice_lock: u128 = 100 * UNITS;
		let bob_lock: u128 = 999 * UNITS;

		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&alice_sub, alice_lock);
		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&bob_sub, bob_lock);

		// Query Alice.
		let data_alice = build_call_data(TOKEN, alice_evm);
		let mut handle_alice = build_handle(alice_evm, data_alice);
		let result_alice =
			pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle_alice);

		let expected_alice = U256::from(alice_lock).to_big_endian();
		assert_eq!(
			result_alice,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected_alice.to_vec(),
			})
		);

		// Query Bob.
		let data_bob = build_call_data(TOKEN, bob_evm);
		let mut handle_bob = build_handle(bob_evm, data_bob);
		let result_bob = pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle_bob);

		let expected_bob = U256::from(bob_lock).to_big_endian();
		assert_eq!(
			result_bob,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected_bob.to_vec(),
			})
		);
	});
}

#[test]
fn get_locked_balance_ignores_token_address_because_only_gigahdx_is_lockable_for_now() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let query_addr = evm_address();
		let substrate_account = account_id_of(query_addr);
		let lock_amount: u128 = 500 * UNITS;

		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&substrate_account, lock_amount);

		//Act
		let data_correct = build_call_data(TOKEN, query_addr);
		let mut handle_correct = build_handle(query_addr, data_correct);
		let result_correct =
			pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle_correct);

		let data_wrong = build_call_data(WRONG_TOKEN, query_addr);
		let mut handle_wrong = build_handle(query_addr, data_wrong);
		let result_wrong =
			pallet_evm_precompile_lock_manager::LockManagerPrecompile::<Runtime>::execute(&mut handle_wrong);

		//Assert
		let expected = U256::from(lock_amount).to_big_endian();
		assert_eq!(
			result_correct,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);
		assert_eq!(
			result_wrong,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);
	});
}

#[test]
fn get_locked_balance_via_precompile_set() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let query_addr = evm_address();
		let substrate_account = account_id_of(query_addr);
		let lock_amount: u128 = 750 * UNITS;

		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::insert(&substrate_account, lock_amount);

		let data = build_call_data(TOKEN, query_addr);
		let mut handle = build_handle(query_addr, data);

		// Route through the full HydraDXPrecompiles set — verifies 0x0806 dispatches correctly.
		let precompiles = HydraDXPrecompiles::<Runtime>::new();
		let result = precompiles.execute(&mut handle);

		let expected = U256::from(lock_amount).to_big_endian();
		assert_eq!(
			result.unwrap(),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);
	});
}
