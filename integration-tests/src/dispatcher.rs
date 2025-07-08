use crate::evm::MockHandle;
use crate::polkadot_test_net::*;
use fp_evm::PrecompileSet;
use frame_support::dispatch::{
	extract_actual_pays_fee, extract_actual_weight, GetDispatchInfo, Pays, PostDispatchInfo,
};
use frame_support::{assert_err, assert_noop, assert_ok};
use hydradx_runtime::evm::precompiles::HydraDXPrecompiles;
use hydradx_runtime::evm::WethAssetId;
use hydradx_runtime::*;
use orml_traits::MultiCurrency;
use pallet_evm::{ExitReason, ExitSucceed};
use pallet_transaction_payment::ChargeTransactionPayment;
use primitives::EvmAddress;
use sp_core::crypto::AccountId32;
use sp_core::Encode;
use sp_core::Get;
use sp_core::{ByteArray, U256};
use sp_runtime::traits::SignedExtension;
use sp_runtime::DispatchErrorWithPostInfo;
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

		let batch = RuntimeCall::Utility(
			(pallet_utility::Call::batch_all {
				calls: vec![call.clone(), call.clone(), call.clone()],
			}),
		);
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

		let batch = RuntimeCall::Utility(
			(pallet_utility::Call::batch_all {
				calls: vec![call.clone(), call.clone(), call.clone()],
			}),
		);
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

		let batch = RuntimeCall::Utility(
			(pallet_utility::Call::batch_all {
				calls: vec![call.clone()],
			}),
		);

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

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let fee_charge = initial_alice_hdx_balance - alice_hdx_balance;

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
				gas_limit: 100_000,
				max_fee_per_gas: U256::from(233_460_000),
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
			gas_limit: 100_000,
			value: U256::zero(),
			max_fee_per_gas: U256::from(233_460_000),
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
	})
}
