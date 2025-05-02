use crate::evm::MockHandle;
use crate::polkadot_test_net::*;
use fp_evm::PrecompileSet;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::evm::precompiles::HydraDXPrecompiles;
use hydradx_runtime::evm::WethAssetId;
use hydradx_runtime::*;
use orml_traits::MultiCurrency;
use primitives::EvmAddress;

use scraper::BOB;

use sp_core::crypto::AccountId32;
use sp_core::Encode;
use sp_core::Get;
use sp_core::{ByteArray, U256};
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
