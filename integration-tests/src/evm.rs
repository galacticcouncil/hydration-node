#![cfg(test)]

use crate::polkadot_test_net::*;
use hydradx_runtime::evm::precompile::multicurrency::{Action, MultiCurrencyPrecompile};
use pallet_evm::*;
use sp_core::{blake2_256, H160, H256, U256};
use std::borrow::Cow;
use xcm_emulator::TestExt;
type CurrencyPrecompile = MultiCurrencyPrecompile<hydradx_runtime::Runtime>;
use crate::assert_balance;
use fp_evm::{Context, Transfer};
use frame_support::assert_ok;
use frame_support::codec::Encode;
use frame_support::traits::{Contains, IsType};
use hex_literal::hex;
use hydradx_runtime::evm::precompile::handle::EvmDataWriter;
use hydradx_runtime::evm::precompile::{Address, Bytes, EvmAddress};
use hydradx_runtime::evm::precompiles::{addr, HydraDXPrecompiles};
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Currencies;
use hydradx_runtime::{CallFilter, RuntimeCall, RuntimeOrigin, Tokens, TransactionPause, EVM};
use orml_traits::MultiCurrency;
use pallet_asset_registry::AssetMetadata;
use pretty_assertions::assert_eq;
use sp_core::crypto::AccountId32;

#[test]
fn precompile_for_currency_name_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = EvmDataWriter::new_with_selector(Action::Name).build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: native_asset_ethereum_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert
		let output = EvmDataWriter::new().write(Bytes::from("HDX".as_bytes())).build();
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output
			})
		);
	});
}

#[test]
fn precompile_for_currency_symbol_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), HDX, b"xHDX".to_vec(), 12u8);

		let data = EvmDataWriter::new_with_selector(Action::Symbol).build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: native_asset_ethereum_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert
		let output = EvmDataWriter::new().write(Bytes::from("xHDX".as_bytes())).build();
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output
			})
		);
	});
}

#[test]
fn precompile_for_currency_decimal_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), HDX, b"xHDX".to_vec(), 12u8);

		let data = EvmDataWriter::new_with_selector(Action::Decimals).build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: native_asset_ethereum_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert

		// 12
		let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000000000000C
			"};

		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected_output.to_vec()
			})
		);
	});
}

#[test]
fn precompile_for_total_supply_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = EvmDataWriter::new_with_selector(Action::TotalSupply).build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: native_asset_ethereum_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert

		// 950330588000000000
		let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000D3040A27CED9800
			"};

		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected_output.to_vec()
			})
		);
	});
}

#[test]
fn precompile_for_balance_of_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account().into(),
			HDX,
			100 * UNITS as i128,
		));

		let data = EvmDataWriter::new_with_selector(Action::BalanceOf)
			.write(Address::from(evm_address()))
			.build();

		let alice = alice_evm_addr();
		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: alice_evm_addr(),
				caller: alice_evm_addr(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert

		// 100 * UNITS
		let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000005AF3107A4000
			"};

		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected_output.to_vec()
			})
		);
	});
}

#[test]
fn precompile_for_transfer_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account().into(),
			HDX,
			100 * UNITS as i128,
		));

		let data = EvmDataWriter::new_with_selector(Action::Transfer)
			.write(Address::from(evm_address2()))
			.write(U256::from(86u128 * UNITS))
			.build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: evm_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert
		assert_eq!(result.unwrap().exit_status, ExitSucceed::Returned);
		assert_balance!(evm_account2(), HDX, 86u128 * UNITS);
	});
}

#[test]
fn precompile_for_currency_allowance_should_return_0_as_not_supported_yet() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = EvmDataWriter::new_with_selector(Action::Allowance)
			.write(Address::from(evm_address()))
			.write(Address::from(evm_address2()))
			.build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: native_asset_ethereum_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert
		//0
		let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected_output.to_vec()
			})
		);
	});
}

#[test]
fn precompile_for_transfer_from_should_work_only_for_caller_address() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account().into(),
			HDX,
			100 * UNITS as i128,
		));

		let data = EvmDataWriter::new_with_selector(Action::TransferFrom)
			.write(Address::from(evm_address()))
			.write(Address::from(evm_address2()))
			.write(U256::from(50u128 * UNITS))
			.build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: evm_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert
		assert_eq!(result.unwrap().exit_status, ExitSucceed::Returned);
		assert_balance!(evm_account2(), HDX, 50u128 * UNITS);
	});
}

//TODO: MANUALLY VERIFY THAT IT FAILS
#[test]
fn precompile_for_transfer_from_should_fail_when_from_address_is_different_than_caller() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account2().into(),
			HDX,
			100 * UNITS as i128,
		));

		let data = EvmDataWriter::new_with_selector(Action::TransferFrom)
			.write(Address::from(evm_address2()))
			.write(Address::from(evm_address()))
			.write(U256::from(50u128 * UNITS))
			.build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: evm_address(),
				caller: evm_address(),
				apparent_value: U256::from(10),
			},
			core_address: native_asset_ethereum_address(),
		};

		//Act
		let result = CurrencyPrecompile::execute(&mut handle);

		//Assert
		assert_eq!(
			result,
			Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("not supported".into())
			})
		);

		assert_balance!(evm_account(), HDX, 0);
	});
}

fn account_to_default_evm_address(account_id: &impl Encode) -> EvmAddress {
	let payload = (b"evm:", account_id);
	EvmAddress::from_slice(&payload.using_encoded(blake2_256)[0..20])
}

pub fn alice_evm_addr() -> H160 {
	//H160::from(hex_literal::hex!("1000000000000000000000000000000000000001"))
	account_to_default_evm_address(&ALICE)
}

#[test]
fn dispatch_should_work_with_remark() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let mut handle = create_dispatch_handle(hex!["0107081337"].to_vec());

		//Act
		let prec = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
		let result = prec.execute(&mut handle);

		//Assert
		assert_eq!(
			result.unwrap(),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Stopped,
				output: Default::default(),
			})
		)
	});
}

#[test]
fn dispatch_should_work_with_transfer() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
			.to_vec();
		let balance = Tokens::free_balance(WETH, &evm_account());

		//Act
		assert_ok!(EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			DISPATCH_ADDR,
			data,
			U256::from(0),
			1000000,
			gwei(1),
			None,
			Some(U256::zero()),
			[].into()
		));

		//Assert
		assert!(Tokens::free_balance(WETH, &evm_account()) < balance - 1 * 10u128.pow(16));
	});
}

#[test]
fn dispatch_should_respect_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let balance = Tokens::free_balance(WETH, &evm_account());
		let amount = 1 * 10u128.pow(16);
		let gas_limit = 1000000;
		let gas_price = gwei(1);
		let transfer_call = RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: ALICE.into(),
			currency_id: WETH,
			amount,
		});
		assert!(CallFilter::contains(&transfer_call));
		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::root(),
			b"Tokens".to_vec(),
			b"transfer".to_vec()
		));
		assert!(!CallFilter::contains(&transfer_call));

		//Act
		assert_ok!(EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			DISPATCH_ADDR,
			transfer_call.encode(),
			U256::from(0),
			gas_limit,
			gas_price,
			None,
			Some(U256::zero()),
			[].into(),
		));

		//Assert
		let new_balance = Tokens::free_balance(WETH, &evm_account());
		assert!(new_balance < balance, "fee wasn't charged");
		assert!(new_balance > balance - amount, "more than fee was taken from account");
		assert_eq!(
			new_balance,
			balance - (U256::from(gas_limit) * gas_price).as_u128(),
			"gas limit was not charged"
		);
		assert_eq!(
			HydraDXPrecompiles::<hydradx_runtime::Runtime>::new()
				.execute(&mut create_dispatch_handle(transfer_call.encode()))
				.unwrap(),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::Other(Cow::from("dispatch execution failed: CallFiltered"))
			})
		);
	});
}

#[test]
fn complete_fee_should_be_transferred_to_treasury() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let balance = Tokens::free_balance(WETH, &evm_account());
		let treasury_balance = Tokens::free_balance(WETH, &Treasury::account_id());
		let issuance = Tokens::total_issuance(WETH);

		//Act
		assert_ok!(EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			evm_address(),
			[].into(),
			U256::from(0),
			1000000,
			gwei(1),
			None,
			Some(U256::zero()),
			[].into()
		));

		//Assert
		let new_balance = Tokens::free_balance(WETH, &evm_account());
		let new_treasury_balance = Tokens::free_balance(WETH, &Treasury::account_id());
		let fee = balance - new_balance;
		assert!(fee > 0);
		assert_eq!(fee, gwei(1).as_u128() * 21000);
		assert_eq!(treasury_balance + fee, new_treasury_balance);
		assert_eq!(issuance, Tokens::total_issuance(WETH));
	});
}

// TODO: test that we charge approximatelly same fee on evm as with extrinsics directly

const DISPATCH_ADDR: H160 = addr(1025);

fn gwei(value: u128) -> U256 {
	U256::from(value) * U256::from(10_u128.pow(9))
}

fn create_dispatch_handle(data: Vec<u8>) -> MockHandle {
	MockHandle {
		input: data,
		context: Context {
			address: DISPATCH_ADDR,
			caller: evm_address(),
			apparent_value: U256::zero(),
		},
		core_address: DISPATCH_ADDR,
	}
}

pub fn native_asset_ethereum_address() -> H160 {
	H160::from(hex!("0000000000000000000100000000000000000000"))
}

pub struct MockHandle {
	pub input: Vec<u8>,
	pub context: Context,
	pub core_address: H160,
}

impl PrecompileHandle for MockHandle {
	fn call(
		&mut self,
		_: H160,
		_: Option<Transfer>,
		_: Vec<u8>,
		_: Option<u64>,
		_: bool,
		_: &Context,
	) -> (ExitReason, Vec<u8>) {
		unimplemented!()
	}

	fn record_cost(&mut self, _: u64) -> Result<(), ExitError> {
		Ok(())
	}

	fn remaining_gas(&self) -> u64 {
		unimplemented!()
	}

	fn log(&mut self, _: H160, _: Vec<H256>, _: Vec<u8>) -> Result<(), ExitError> {
		unimplemented!()
	}

	fn code_address(&self) -> H160 {
		self.core_address
	}

	fn input(&self) -> &[u8] {
		&self.input
	}

	fn context(&self) -> &Context {
		&self.context
	}

	fn is_static(&self) -> bool {
		unimplemented!()
	}

	fn gas_limit(&self) -> Option<u64> {
		None
	}
}
