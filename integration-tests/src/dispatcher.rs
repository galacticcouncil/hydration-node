use crate::evm::{create_dispatch_handle, gas_price};
use crate::polkadot_test_net::*;
use crate::utils::accounts::MockAccount;
use fp_evm::PrecompileSet;
use frame_support::assert_ok;
use frame_support::dispatch::{
	extract_actual_pays_fee, extract_actual_weight, GetDispatchInfo, Pays, PostDispatchInfo,
};
use hydradx_runtime::evm::precompiles::{HydraDXPrecompiles, DISPATCH_ADDR};
use hydradx_runtime::evm::WethAssetId;
use hydradx_runtime::*;
use orml_traits::MultiCurrency;
use pallet_evm::{ExitReason, ExitSucceed};
use pallet_evm_accounts::EvmNonceProvider;
use pallet_transaction_multi_payment::EVMPermit;
use pallet_transaction_payment::ChargeTransactionPayment;
use precompile_utils::prelude::PrecompileOutput;
use primitives::EvmAddress;
use sp_core::Get;
use sp_core::{ByteArray, U256};
use sp_core::{Encode, Pair};
use sp_runtime::traits::{IdentifyAccount, SignedExtension};
use sp_runtime::{DispatchErrorWithPostInfo, MultiSigner};
use test_utils::last_events;
use xcm_emulator::TestExt;

fn testnet_manager_address() -> EvmAddress {
	hex!["52341e77341788Ebda44C8BcB4C8BD1B1913B204"].into()
}

fn pad_to_32_bytes(bytes: &[u8]) -> [u8; 32] {
	let mut padded = [0u8; 32];
	padded[..bytes.len()].copy_from_slice(bytes);
	padded
}

fn testnet_manager() -> AccountId {
	pad_to_32_bytes(testnet_manager_address().as_bytes()).into()
}

#[test]
fn testnet_aave_manager_can_be_set_as_dispatcher() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Dispatcher::aave_manager_account(),
			pad_to_32_bytes(hex!["aa7e0000000000000000000000000000000aa7e0"].as_ref()).into()
		);
		assert_ok!(hydradx_runtime::Dispatcher::note_aave_manager(
			hydradx_runtime::RuntimeOrigin::root(),
			testnet_manager()
		));
		assert_eq!(hydradx_runtime::Dispatcher::aave_manager_account(), testnet_manager());
	});
}

#[test]
fn dispatch_as_aave_admin_can_modify_supply_cap_on_testnet() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Dispatcher::note_aave_manager(
			hydradx_runtime::RuntimeOrigin::root(),
			testnet_manager()
		));
		assert_ok!(hydradx_runtime::Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			EVMAccounts::account_id(testnet_manager_address()),
			WethAssetId::get(),
			1_000_000_000_000_000_000u128,
			0
		));
		let set_cap_data = hex!["571f03e50000000000000000000000000000000000000000000000000000000100000005000000000000000000000000000000000000000000000000000000000006c81c"].into();
		let call = Box::new(RuntimeCall::EVM(pallet_evm::Call::call {
			source: EvmAddress::from_slice(&testnet_manager().as_slice()[0..20]),
			target: hex!["5AFf8be73B6AA6890DaCe9483a6AE9CEfA002795"].into(),
			input: set_cap_data,
			gas_limit: 100_000,
			value: U256::zero(),
			max_fee_per_gas: U256::from(233_460_000),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
		}));
		assert_ok!(Dispatcher::dispatch_as_aave_manager(
			RuntimeOrigin::root(),
			call.clone()
		));
		let event = last_events::<RuntimeEvent, Runtime>(1)
			.into_iter()
			.take(1)
			.next()
			.unwrap();
		match event {
			RuntimeEvent::Dispatcher(pallet_dispatcher::Event::AaveManagerCallDispatched {
				result: Ok(..), ..
			}) => {}
			_ => panic!("Unexpected event: {:?}", event),
		}
	});
}

#[test]
fn dispatch_with_extra_gas_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = crate::utils::contracts::deploy_contract("GasEater", crate::contracts::deployer());
		let erc20 = crate::erc20::bind_erc20(contract);

		// Act
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: erc20,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone(), call.clone(), call.clone()],
		});
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(ALICE.into()),
			Box::new(batch.clone()),
			130_000,
		));

		//Assert
		assert_eq!(Currencies::free_balance(erc20, &BOB.into()), 300);
	});
}

#[test]
fn dispatch_with_extra_gas_should_fail_when_extra_gas_is_not_enough() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = crate::utils::contracts::deploy_contract("GasEater", crate::contracts::deployer());
		let erc20 = crate::erc20::bind_erc20(contract);

		// Act
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: erc20,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone(), call.clone(), call.clone()],
		});
		let result =
			Dispatcher::dispatch_with_extra_gas(RuntimeOrigin::signed(ALICE.into()), Box::new(batch.clone()), 50_000);

		assert!(result.is_err());

		//Assert
		assert_eq!(Currencies::free_balance(erc20, &BOB.into()), 0);
	});
}

#[test]
fn dispatch_with_extra_gas_should_pay_for_extra_gas_used_when_it_is_not_used() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let hydra_contract = crate::utils::contracts::deploy_contract("HydraToken", crate::contracts::deployer());
		let contract = crate::utils::contracts::deploy_contract("GasEater", crate::contracts::deployer());
		let hydra_erc20 = crate::erc20::bind_erc20(hydra_contract);
		let erc20 = crate::erc20::bind_erc20(contract);

		// Get HydraToken tx fee
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: hydra_erc20,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone()],
		});

		let dispatch_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch.clone()),
			extra_gas: 50_000,
		});
		let info = dispatch_call.get_dispatch_info();
		let info_len = dispatch_call.encoded_size();

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &dispatch_call, &info, info_len);
		assert_ok!(&pre);
		let result = dispatch_call.dispatch(RuntimeOrigin::signed(ALICE.into()));
		assert_ok!(result);
		assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&result.unwrap(),
			info_len,
			&Ok(())
		));
		assert_eq!(Currencies::free_balance(hydra_erc20, &BOB.into()), 100);

		let alice_balance_final = Currencies::free_balance(HDX, &ALICE.into());
		let hydra_paid_fee = initial_alice_hdx_balance - alice_balance_final;

		// Get GasEater tx fee
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: erc20,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone()],
		});

		let dispatch_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch.clone()),
			extra_gas: 50_000,
		});
		let info = dispatch_call.get_dispatch_info();
		let info_len = dispatch_call.encoded_size();

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &dispatch_call, &info, info_len);
		assert_ok!(&pre);
		let result = dispatch_call.dispatch(RuntimeOrigin::signed(ALICE.into()));
		assert_ok!(result);
		assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&result.unwrap(),
			info_len,
			&Ok(())
		));
		assert_eq!(Currencies::free_balance(erc20, &BOB.into()), 100);

		let alice_balance_final = Currencies::free_balance(HDX, &ALICE.into());
		let gas_eater_paid_fee = initial_alice_hdx_balance - alice_balance_final;
		assert_eq!(
			gas_eater_paid_fee, hydra_paid_fee,
			"GasEater transfer should cost the same as HydraToken transfer: {:?} == {:?}",
			gas_eater_paid_fee, hydra_paid_fee
		);
	});
}

#[test]
fn dispatch_with_extra_gas_should_not_refund_extra_gas_correctly() {
	// Test scenario to compare the fees paid for two transactions with different extra gas limits.
	// The expectation is that the fees paid much be higher for the second transaction

	// Fee charged on tx submit
	let mut fee_charge_1 = 0;
	let mut fee_charge_2 = 0;

	// Final fees paid after refund
	let mut actual_fee_paid_1 = 0;
	let mut actual_fee_paid_2 = 0;
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = crate::utils::contracts::deploy_contract("GasEater", crate::contracts::deployer());
		let erc20 = crate::erc20::bind_erc20(contract);

		// Get GasEater tx fee
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: erc20,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone()],
		});

		let dispatch_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch.clone()),
			extra_gas: 50_000,
		});
		let info = dispatch_call.get_dispatch_info();
		let info_len = dispatch_call.encoded_size();

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &dispatch_call, &info, info_len);
		assert_ok!(&pre);
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		fee_charge_1 = initial_alice_hdx_balance - alice_hdx_balance;

		let result = dispatch_call.dispatch(RuntimeOrigin::signed(ALICE.into()));
		assert_ok!(result);
		assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&result.unwrap(),
			info_len,
			&Ok(())
		));
		assert_eq!(Currencies::free_balance(erc20, &BOB.into()), 100);

		let alice_balance_final = Currencies::free_balance(HDX, &ALICE.into());
		actual_fee_paid_1 = initial_alice_hdx_balance - alice_balance_final;
	});

	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = crate::utils::contracts::deploy_contract("GasEater", crate::contracts::deployer());
		let erc20 = crate::erc20::bind_erc20(contract);

		// Get GasEater tx fee
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: erc20,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone()],
		});

		let dispatch_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch.clone()),
			extra_gas: 1_050_000,
		});
		let info = dispatch_call.get_dispatch_info();
		let info_len = dispatch_call.encoded_size();

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &dispatch_call, &info, info_len);
		assert_ok!(&pre);
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		fee_charge_2 = initial_alice_hdx_balance - alice_hdx_balance;

		let result = dispatch_call.dispatch(RuntimeOrigin::signed(ALICE.into()));
		assert_ok!(result);
		assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&result.unwrap(),
			info_len,
			&Ok(())
		));
		assert_eq!(Currencies::free_balance(erc20, &BOB.into()), 100);

		let alice_balance_final = Currencies::free_balance(HDX, &ALICE.into());
		actual_fee_paid_2 = initial_alice_hdx_balance - alice_balance_final;
	});

	// Fee charged on tx submit should be higher in the second case
	assert!(
		fee_charge_2 > fee_charge_1,
		"Fee charged on tx submit should be higher in the second case: {:?} > {:?}",
		fee_charge_2,
		fee_charge_1
	);

	// the two tx fees should be the same because it should refund correctly the unused gas
	assert!(
		actual_fee_paid_1 < actual_fee_paid_2,
		"Paid fee should be higher for the second tx: {:?} < {:?}",
		actual_fee_paid_1,
		actual_fee_paid_2
	);
}

#[test]
fn dispatch_with_extra_gas_should_charge_extra_gas_when_calls_fail() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: 1234,
			amount: 100,
		});

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call.clone()],
		});

		let dispatch_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch.clone()),
			extra_gas: 0,
		});
		let info = dispatch_call.get_dispatch_info();
		let info_len = dispatch_call.encoded_size();

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &dispatch_call, &info, info_len);
		assert_ok!(&pre);

		let result = dispatch_call.dispatch(RuntimeOrigin::signed(ALICE.into()));
		assert!(result.is_err());
		let r = result.unwrap_err();
		let _ = ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&r.post_info,
			info_len,
			&Ok(()),
		);

		let final_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let no_gas_fee = initial_alice_hdx_balance - final_alice_hdx_balance;

		let dispatch_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch.clone()),
			extra_gas: 100_000,
		});
		let info = dispatch_call.get_dispatch_info();
		let info_len = dispatch_call.encoded_size();

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &dispatch_call, &info, info_len);
		assert_ok!(&pre);
		let result = dispatch_call.dispatch(RuntimeOrigin::signed(ALICE.into()));
		assert!(result.is_err());
		let r = result.unwrap_err();
		let _ = ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&r.post_info,
			info_len,
			&Ok(()),
		);

		let final_alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let extra_gas_fee = initial_alice_hdx_balance - final_alice_hdx_balance;
		assert!(
			no_gas_fee < extra_gas_fee,
			"No gas fee should be less than extra gas fee"
		);
	});
}

#[test]
fn dispatch_evm_call_should_work_when_evm_call_succeeds() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Verify that LastEvmCallExitReason storage is cleaned before execution
		assert_eq!(Dispatcher::last_evm_call_exit_reason(), None);

		// Arrange: Deploy a valid contract to interact with
		let contract = crate::utils::contracts::deploy_contract("HydraToken", crate::contracts::deployer());
		let stop_code_contract = crate::utils::contracts::deploy_contract_code(
			hex!["608080604052346013576067908160188239f35b5f80fdfe6004361015600b575f80fd5b5f3560e01c6306fdde0314601d575f80fd5b34602d575f366003190112602d57005b5f80fdfea264697066735822122072cd2025c9922b7f29b4174f1e2d766386a8ecbaab35dc5921cda0fa301dcb3e64736f6c634300081e0033"].to_vec(),
			crate::contracts::deployer(),
		); // name() function selector returns "stopped"

		assert_ok!(hydradx_runtime::Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			WethAssetId::get(),
			1_000_000_000_000_000_000u128,
			0
		));

		// Helper function to create EVM calls with common parameters
		let create_evm_call = |target| {
			Box::new(RuntimeCall::EVM(pallet_evm::Call::call {
				source: evm_address(),
				target,
				input: hex!["06fdde03"].to_vec(), // name() function selector
				value: U256::zero(),
				gas_limit: 1_000_000,
				max_fee_per_gas: gas_price(),
				max_priority_fee_per_gas: None,
				nonce: None,
				access_list: vec![],
			}))
		};

		// Create test cases with different targets
		let call_succeed_returned = create_evm_call(contract);
		let call_succeed_stopped = create_evm_call(stop_code_contract);

		// Act: Dispatch the EVM calls
		assert_ok!(Dispatcher::dispatch_evm_call(
			evm_signed_origin(evm_address()),
			call_succeed_returned
		));

		// Verify that LastEvmCallExitReason storage has expected Returned value
		assert_eq!(
			Dispatcher::last_evm_call_exit_reason(),
			Some(ExitReason::Succeed(ExitSucceed::Returned))
		);

		assert_ok!(Dispatcher::dispatch_evm_call(
			evm_signed_origin(evm_address()),
			call_succeed_stopped
		));

		// Verify that LastEvmCallExitReason storage has expected Stopped value
		assert_eq!(
			Dispatcher::last_evm_call_exit_reason(),
			Some(ExitReason::Succeed(ExitSucceed::Stopped))
		);

		// Produce the next block and ensure the key is gone at the next block
		hydradx_run_to_next_block();
		assert_eq!(
			Dispatcher::last_evm_call_exit_reason(),
			None,
			"Storage key should stay empty in subsequent blocks"
		);
	});
}

#[test]
fn dispatch_evm_call_should_fail_with_invalid_function_selector() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Verify that LastEvmCallExitReason storage is cleaned before execution
		assert_eq!(Dispatcher::last_evm_call_exit_reason(), None);

		// Arrange
		assert_ok!(hydradx_runtime::Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			WethAssetId::get(),
			1_000_000_000_000_000_000u128,
			0
		));

		// Deploy a contract to test with
		let contract = crate::utils::contracts::deploy_contract("HydraToken", crate::contracts::deployer());

		// Create an EVM call with an invalid function selector
		let call = RuntimeCall::EVM(pallet_evm::Call::call {
			source: evm_address(),
			target: contract,
			input: hex!["12345678"].to_vec(), // Invalid function selector
			gas_limit: 1_000_000,
			value: U256::zero(),
			max_fee_per_gas: gas_price(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
		});
		let call_data = call.get_dispatch_info();
		let boxed_call = Box::new(call);

		// Act
		let result = Dispatcher::dispatch_evm_call(evm_signed_origin(evm_address()), boxed_call);

		// Assert
		// The dispatch should fail with EvmCallFailed error
		assert_eq!(
			result,
			Err(DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(extract_actual_weight(&result, &call_data)),
					pays_fee: extract_actual_pays_fee(&result, &call_data),
				},
				error: pallet_dispatcher::Error::<Runtime>::EvmCallFailed.into(),
			})
		);

		// Verify that LastEvmCallExitReason storage is cleaned after faulty execution
		assert_eq!(Dispatcher::last_evm_call_exit_reason(), None);
	});
}

#[test]
fn dispatch_evm_call_should_fail_with_not_evm_call_error() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange: Create a non-EVM call
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: 1234,
			amount: 100,
		});
		let boxed_call = Box::new(call.clone());

		// Record EVM nonce before
		let evm_nonce_before = <hydradx_runtime::evm::EvmNonceProvider as EvmNonceProvider>::get_nonce(evm_address());

		// Act & Assert: The dispatch should fail with NotEvmCall error
		let result = Dispatcher::dispatch_evm_call(evm_signed_origin(evm_address()), boxed_call);
		assert_eq!(
			result,
			Err(DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: None,
					pays_fee: Pays::Yes,
				},
				error: pallet_dispatcher::Error::<Runtime>::NotEvmCall.into(),
			})
		);

		// EVM nonce should not change when extrinsic fails pre-EVM
		assert_eq!(
			<hydradx_runtime::evm::EvmNonceProvider as EvmNonceProvider>::get_nonce(evm_address()),
			evm_nonce_before
		);
	})
}

#[test]
fn dispatch_evm_call_via_precompile_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let stop_code_contract = crate::utils::contracts::deploy_contract_code(
			hex!["608080604052346013576067908160188239f35b5f80fdfe6004361015600b575f80fd5b5f3560e01c6306fdde0314601d575f80fd5b34602d575f366003190112602d57005b5f80fdfea264697066735822122072cd2025c9922b7f29b4174f1e2d766386a8ecbaab35dc5921cda0fa301dcb3e64736f6c634300081e0033"].to_vec(),
			crate::contracts::deployer(),
		); // name() function selector returns "stopped"

		assert_ok!(hydradx_runtime::Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			WethAssetId::get(),
			1_000_000_000_000_000_000u128,
			0
		));

		let inner_runtime_call = RuntimeCall::EVM(pallet_evm::Call::call {
			source: evm_address(),
			target: stop_code_contract,
			input: hex!["06fdde03"].to_vec(), // name() function selector
			value: U256::zero(),
			gas_limit: 100_000,
			max_fee_per_gas: U256::from(233_460_000),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
		});

		let outer_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_evm_call {
			call: Box::new(inner_runtime_call),
		});

		// SCALE‑encode the entire outer call for precompile
		let data = outer_call.encode();

		// Build a mocked EVM precompile handle which basically simulates a MetaMask
		// transaction calling the Frontier dispatch precompile (`DISPATCH_ADDR`)
		// from the default test EVM account.
		let mut handle = create_dispatch_handle(data);

		// Execute all HydraDX precompiles (this includes the standard
		// Frontier Dispatch precompile wired at `DISPATCH_ADDR`).
		let precompiles = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
		let result = precompiles.execute(&mut handle);

		// The dispatch precompile should succeed and stop.
		assert_eq!(
			result.unwrap(),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Stopped,
				output: Default::default(),
			})
		);
	});
}

#[test]
fn dispatch_evm_call_with_batch_should_not_increase_nonce_internally() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange: Deploy a valid contract to interact with
		let contract = crate::utils::contracts::deploy_contract("HydraToken", crate::contracts::deployer());

		let account = MockAccount::new(evm_account());

		// Fund WETH for EVM gas and set WETH as fee currency for simplicity
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			account.address(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(account.address()),
			WETH,
		));

		// Build two identical EVM calls dispatched via the Dispatcher pallet
		let call1 = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_evm_call {
			call: Box::new(RuntimeCall::EVM(pallet_evm::Call::call {
				source: evm_address(),
				target: contract,
				input: hex!["06fdde03"].to_vec(), // name() function selector
				value: U256::zero(),
				gas_limit: 1_000_000,
				max_fee_per_gas: gas_price(),
				max_priority_fee_per_gas: None,
				nonce: None,
				access_list: vec![],
			})),
		});
		let call2 = call1.clone();
		let call3 = call1.clone();

		// Wrap both calls into a single batch_all
		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![call1, call2, call3],
		});

		// Record system nonce before dispatch
		let nonce_before = account.nonce();

		// EVM & permit nonces before
		let evm_nonce_before = hydradx_runtime::evm::EvmNonceProvider::get_nonce(evm_address());
		let permit_nonce_before =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				evm_address(),
			);

		// Encode and execute via the Dispatch precompile entry point
		let data = batch.encode();
		let mut handle = create_dispatch_handle(data);
		let precompiles = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
		let result = precompiles.execute(&mut handle).unwrap();

		// The precompile should stop successfully
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Stopped,
				output: Default::default()
			})
		);

		// Assert that the account system nonce DID NOT change (batch runs inside one EVM tx)
		assert_eq!(account.nonce(), nonce_before);
		// EVM nonce should also be unchanged (runner restores it for non-raw-ETH paths)
		assert_eq!(
			hydradx_runtime::evm::EvmNonceProvider::get_nonce(evm_address()),
			evm_nonce_before
		);
		// Permit nonce should remain unchanged (no permit used)
		assert_eq!(
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				evm_address()
			),
			permit_nonce_before
		);
	});
}

#[test]
fn dispatch_evm_call_batch_via_call_permit_should_increase_permit_nonce_once() {
	TestNet::reset();
	Hydra::execute_with(|| {
		use hydradx_runtime::evm::precompiles::{CALLPERMIT, DISPATCH_ADDR};
		use libsecp256k1::{sign, Message, SecretKey};

		// Prepare account and balances
		let user_evm_address = crate::utils::accounts::alith_evm_address();
		let user_secret_key = crate::utils::accounts::alith_secret_key();
		let user_acc = MockAccount::new(crate::utils::accounts::alith_truncated_account());

		// Fund HDX for tx fee and WETH for inner EVM gas
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000i128,
		));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));

		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(user_acc.address()),
			WETH,
		));

		// Deploy a simple contract and build two identical EVM calls dispatched via Dispatcher
		let contract = crate::utils::contracts::deploy_contract("HydraToken", crate::contracts::deployer());
		let inner_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_evm_call {
			call: Box::new(RuntimeCall::EVM(pallet_evm::Call::call {
				source: user_evm_address,
				target: contract,
				input: hex!("06fdde03").to_vec(), // name() function selector
				value: U256::zero(),
				gas_limit: 1_000_000,
				max_fee_per_gas: gas_price(),
				max_priority_fee_per_gas: None,
				nonce: None,
				access_list: vec![],
			})),
		});
		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![inner_call.clone(), inner_call],
		});

		// Prepare Call Permit for the batch call to be dispatched via DISPATCH precompile
		let gas_limit = 1_000_000u64;
		let deadline = U256::from(1_000_000_000_000u128);
		let permit_message =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::zero(),
				batch.encode(),
				gas_limit * 10,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).expect("valid secret key");
		let message = Message::parse(&permit_message);
		let (rs, v) = sign(&message, &secret_key);

		// Read permit nonce before
		let permit_nonce_before =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		let evm_nonce_before = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		let user_acc_nonce_before = user_acc.nonce();

		// Dispatch the permit (unsigned)
		assert_ok!(hydradx_runtime::MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::zero(),
			batch.encode(),
			gas_limit * 10,
			deadline,
			v.serialize(),
			sp_core::H256::from(rs.r.b32()),
			sp_core::H256::from(rs.s.b32()),
		));

		// Assert that the permit nonce increased exactly once
		let permit_nonce_after =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce_after, permit_nonce_before + U256::one());
		assert_eq!(user_acc.nonce(), user_acc_nonce_before);
		// EVM nonce should not change on the permit path (runner restores it)
		assert_eq!(
			hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address),
			evm_nonce_before
		);
	});
}

#[test]
fn dispatch_evm_call_with_failing_signed_batch_should_increase_nonce_once() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let account = MockAccount::new(MultiSigner::from(pair.public()).into_account());

		// Fund native balance for fees
		assert_ok!(Tokens::set_balance(
			RuntimeOrigin::root(),
			account.address(),
			WETH,
			to_ether(1),
			0
		));

		// Bind an EVM address to this Substrate account
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(account.address())));
		let evm_address = EVMAccounts::evm_address(&account.address());

		// Record nonces before
		let evm_nonce_before = <hydradx_runtime::evm::EvmNonceProvider as EvmNonceProvider>::get_nonce(evm_address);

		// Prepare a simple inner EVM call to the DISPATCH precompile; we'll batch it 3x
		let inner_evm_call = RuntimeCall::EVM(pallet_evm::Call::call {
			source: evm_address,
			target: DISPATCH_ADDR,
			input: hex!["12345678"].to_vec(), // Invalid function selector
			value: U256::zero(),
			gas_limit: 1_000_000,
			max_fee_per_gas: gas_price(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
		});
		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![inner_evm_call.clone(), inner_evm_call.clone(), inner_evm_call],
		});

		// Outer EVM call to DISPATCH precompile with encoded batch
		let evm_call = RuntimeCall::EVM(pallet_evm::Call::call {
			source: evm_address,
			target: DISPATCH_ADDR,
			input: batch.encode(),
			value: U256::zero(),
			gas_limit: 1_000_000,
			max_fee_per_gas: gas_price(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
		});

		// Act: dispatch as a signed extrinsic
		crate::utils::executive::assert_executive_apply_signed_extrinsic(evm_call, pair);

		// EVM nonce should also increase exactly by one on the signed path
		// Note: currently EVM nonce uses the same storage as system nonce,
		// 	and here nonce is incremented by SignedExtra's CheckNonce
		assert_eq!(
			<hydradx_runtime::evm::EvmNonceProvider as EvmNonceProvider>::get_nonce(evm_address),
			evm_nonce_before + U256::one()
		);
	});
}

// Nonce tests

fn dispatch_evm_call_with_params(is_batch: bool, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>) {
	// Arrange: Deploy a valid contract to interact with
	let contract = crate::utils::contracts::deploy_contract("HydraToken", crate::contracts::deployer());

	// Arrange
	let account = MockAccount::new(evm_account());

	// Fund WETH for EVM gas and set WETH as fee currency for simplicity
	assert_ok!(hydradx_runtime::Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		account.address(),
		WETH,
		to_ether(1).try_into().unwrap(),
	));

	assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
		RuntimeOrigin::signed(account.address()),
		WETH,
	));

	// Build two identical EVM calls dispatched via the Dispatcher pallet
	let inner_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_evm_call {
		call: Box::new(RuntimeCall::EVM(pallet_evm::Call::call {
			source: evm_address(),
			target: contract,
			input: hex!["06fdde03"].to_vec(), // name() function selector
			value: U256::zero(),
			gas_limit: 1_000_000,
			max_fee_per_gas: gas_price(),
			max_priority_fee_per_gas,
			nonce,
			access_list: vec![],
		})),
	});
	let mut call = inner_call.clone();
	if is_batch {
		call = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![inner_call.clone(), inner_call.clone()],
		});
	}

	// Encode and execute via Executive
	let data = call.encode();
	let mut handle = create_dispatch_handle(data);
	let precompiles = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
	let result = precompiles.execute(&mut handle).unwrap();

	// The precompile should stop successfully
	assert_eq!(
		result,
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Stopped,
			output: Default::default()
		})
	);
}

#[test]
fn dispatch_evm_call_without_batch_should_increase_nonce_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let account = MockAccount::new(evm_account());

		// Record system nonce before dispatch
		let initial_nonce = account.nonce();

		// EVM & permit nonces before
		let initial_evm_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(evm_address());
		let initial_permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				evm_address(),
			);

		// No increment
		dispatch_evm_call_with_params(false, None, None);
		assert_eq!(account.nonce(), initial_nonce);
		assert_eq!(evm::EvmNonceProvider::get_nonce(evm_address()), initial_evm_nonce);

		// Should increment
		dispatch_evm_call_with_params(false, Some(15_000.into()), None);
		let last_evm_nonce = evm::EvmNonceProvider::get_nonce(evm_address());

		assert_eq!(account.nonce(), initial_nonce + 1);
		assert_eq!(last_evm_nonce, initial_evm_nonce + 1);

		dispatch_evm_call_with_params(false, None, Some(last_evm_nonce.into()));
		let last_evm_nonce = evm::EvmNonceProvider::get_nonce(evm_address());

		assert_eq!(account.nonce(), initial_nonce + 2);
		assert_eq!(last_evm_nonce, initial_evm_nonce + 2);

		dispatch_evm_call_with_params(false, Some(15_000.into()), Some(last_evm_nonce.into()));
		assert_eq!(account.nonce(), initial_nonce + 3);
		assert_eq!(evm::EvmNonceProvider::get_nonce(evm_address()), initial_evm_nonce + 3);

		// We didn't use permit at all. Should stay unchanged
		assert_eq!(
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				evm_address()
			),
			initial_permit_nonce
		);
	});
}

#[test]
fn dispatch_evm_call_with_batch_should_increase_nonce_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let account = MockAccount::new(evm_account());

		// Record system nonce before dispatch
		let initial_nonce = account.nonce();
		let get_evm_nonce = || evm::EvmNonceProvider::get_nonce(evm_address());

		// EVM & permit nonces before
		let initial_evm_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(evm_address());
		let initial_permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				evm_address(),
			);

		// No increment
		dispatch_evm_call_with_params(true, None, None);
		assert_eq!(account.nonce(), initial_nonce);
		assert_eq!(get_evm_nonce(), initial_evm_nonce);

		// Explicit nonce in precompile will fail
		// dispatch_evm_call_with_params(true, None, Some(get_evm_nonce()));

		// Should increment
		dispatch_evm_call_with_params(true, Some(15_000.into()), None);
		assert_eq!(account.nonce(), initial_nonce + 1);
		assert_eq!(get_evm_nonce(), initial_evm_nonce + 1);

		// We didn't use permit at all. Should stay unchanged
		assert_eq!(
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				evm_address()
			),
			initial_permit_nonce
		);
	});
}
