#![cfg(test)]

use crate::{assert_balance, polkadot_test_net::*};
use fp_evm::{Context, Transfer};
use frame_support::{assert_ok, dispatch::GetDispatchInfo, sp_runtime::codec::Encode, traits::Contains};
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::{
	evm::precompiles::{
		addr,
		handle::EvmDataWriter,
		multicurrency::{Action, MultiCurrencyPrecompile},
		Address, Bytes, EvmAddress, HydraDXPrecompiles,
	},
	AssetRegistry, Balances, CallFilter, Currencies, RuntimeCall, RuntimeOrigin, Tokens, TransactionPause, EVM,
};
use orml_traits::MultiCurrency;
use pallet_evm::*;
use pretty_assertions::assert_eq;
use sp_core::{blake2_256, H160, H256, U256};
use sp_runtime::{traits::SignedExtension, FixedU128, Permill};
use std::borrow::Cow;
use xcm_emulator::TestExt;

const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

mod currency_precompile {
	use super::*;
	use pretty_assertions::assert_eq;

	type AllHydraDXPrecompile = HydraDXPrecompiles<hydradx_runtime::Runtime>;
	type CurrencyPrecompile = MultiCurrencyPrecompile<hydradx_runtime::Runtime>;

	#[test]
	fn all_hydra_precompile_should_match_native_asset_address() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Action::Name).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = AllHydraDXPrecompile::new().execute(&mut handle);

			//Assert
			assert!(result.is_some());
			let output = EvmDataWriter::new().write(Bytes::from("HDX".as_bytes())).build();

			assert_eq!(
				result,
				Some(Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output
				}))
			);
		});
	}

	#[test]
	fn all_hydra_precompile_should_match_asset_address_with_max_asset_value() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Action::Name).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				core_address: H160::from(hex!("00000000000000000000000000000001ffffffff")),
				is_static: true,
			};

			//Act
			let result = AllHydraDXPrecompile::new().execute(&mut handle);

			//Assert
			assert!(result.is_some());
			assert_eq!(
				result,
				Some(Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("Non-existing asset.".into()),
				}))
			);
		});
	}

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
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
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
			AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), HDX, b"xHDX".to_vec(), 12u8).unwrap();

			let data = EvmDataWriter::new_with_selector(Action::Symbol).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
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
			AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), HDX, b"xHDX".to_vec(), 12u8).unwrap();

			let data = EvmDataWriter::new_with_selector(Action::Decimals).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
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
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert

			// 950331588000000000
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000D30418B5192A800								  
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
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Action::BalanceOf)
				.write(Address::from(evm_address()))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: alice_evm_addr(),
					caller: alice_evm_addr(),
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
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
				evm_account(),
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
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: false,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(result.unwrap().exit_status, ExitSucceed::Returned);
			assert_balance!(evm_account2(), HDX, 86u128 * UNITS);
		});
	}

	#[test]
	fn precompile_for_currency_approve_allowance_should_fail_as_not_supported() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Action::Approve)
				.write(Address::from(evm_address2()))
				.write(U256::from(50u128 * UNITS))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
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
		});
	}

	#[test]
	fn precompile_for_currency_allowance_should_fail_as_not_supported() {
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
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: true,
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
		});
	}

	#[test]
	fn precompile_for_transfer_from_should_fail_as_not_supported() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				evm_account(),
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
					apparent_value: U256::from(0),
				},
				core_address: native_asset_ethereum_address(),
				is_static: false,
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
			assert_balance!(evm_account2(), HDX, 0);
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
			gas_price(),
			None,
			Some(U256::zero()),
			[].into()
		));

		//Assert
		assert!(Tokens::free_balance(WETH, &evm_account()) < balance - 10u128.pow(16));
	});
}

#[test]
fn dispatch_transfer_should_not_work_with_insufficient_fees() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
			.to_vec();
		let insufficient_gas_price = gas_price() - U256::one();

		//Act
		let call = EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			DISPATCH_ADDR,
			data,
			U256::from(0),
			1000000,
			insufficient_gas_price,
			None,
			Some(U256::zero()),
			[].into(),
		);

		//Assert
		call.expect_err("Expected GasPriceTooLow error");
	});
}

#[test]
fn dispatch_should_respect_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let balance = Tokens::free_balance(WETH, &evm_account());
		let amount = 10u128.pow(16);
		let gas_limit = 1000000;
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
			gas_price(),
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
			balance - (U256::from(gas_limit) * gas_price()).as_u128(),
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
fn compare_fee_between_evm_and_native_omnipool_calls() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Set alice with as fee currency and fund it
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			WETH,
		));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			WETH,
			100 * UNITS as i128,
		));

		//Fund evm account with HDX to dispatch omnipool sell
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			HDX,
			100 * UNITS as i128,
		));

		init_omnipool_with_oracle_for_block_10();
		let treasury_eth_balance = Tokens::free_balance(WETH, &Treasury::account_id());
		let alice_weth_balance = Tokens::free_balance(WETH, &AccountId::from(ALICE));

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: UNITS,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		//Execute omnipool via EVM
		assert_ok!(EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			DISPATCH_ADDR,
			omni_sell.encode(),
			U256::from(0),
			gas_limit,
			gas_price(),
			None,
			Some(U256::zero()),
			[].into(),
		));

		//Pre dispatch the native omnipool call - so withdrawring only the fees for the execution
		let info = omni_sell.get_dispatch_info();
		let len: usize = 1;
		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0).pre_dispatch(
				&AccountId::from(ALICE),
				&omni_sell,
				&info,
				len,
			)
		);

		//Determine fees and compare
		let alice_new_weth_balance = Tokens::free_balance(WETH, &AccountId::from(ALICE));
		let fee_weth_native = alice_weth_balance - alice_new_weth_balance;

		let new_treasury_eth_balance = Tokens::free_balance(WETH, &Treasury::account_id());
		let fee_weth_evm = new_treasury_eth_balance - treasury_eth_balance;

		let fee_difference = fee_weth_evm - fee_weth_native;

		let relative_fee_difference = FixedU128::from_rational(fee_difference, fee_weth_native);
		let tolerated_fee_difference = FixedU128::from_rational(20, 100);

		// EVM fees should be higher
		assert!(fee_difference > 0);

		// EVM fees should be not higher than 20%
		assert!(relative_fee_difference < tolerated_fee_difference);
	})
}

fn init_omnipool_with_oracle_for_block_10() {
	init_omnipol();
	//do_trade_to_populate_oracle(DAI, HDX, UNITS);
	set_relaychain_block_number(10);
	//do_trade_to_populate_oracle(DAI, HDX, UNITS);
}

pub fn init_omnipol() {
	let native_price = FixedU128::from_float(0.5);
	let stable_price = FixedU128::from_float(0.7);
	let acc = hydradx_runtime::Omnipool::protocol_account();

	let stable_amount: Balance = 5_000_000_000_000_000_000_000u128;
	let native_amount: Balance = 5_000_000_000_000_000_000_000u128;
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DAI,
		stable_amount,
		0
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		acc,
		HDX,
		native_amount as i128,
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		hydradx_runtime::Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
	));
}

// TODO: test that we charge approximatelly same fee on evm as with extrinsics directly

const DISPATCH_ADDR: H160 = addr(1025);

fn gas_price() -> U256 {
	U256::from(8 * 10_u128.pow(7))
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
		is_static: true,
	}
}

pub fn native_asset_ethereum_address() -> H160 {
	H160::from(hex!("0000000000000000000000000000000100000000"))
}

pub struct MockHandle {
	pub input: Vec<u8>,
	pub context: Context,
	pub core_address: H160,
	pub is_static: bool,
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

	fn record_external_cost(
		&mut self,
		_ref_time: Option<u64>,
		_proof_size: Option<u64>,
		_storage_growth: Option<u64>,
	) -> Result<(), ExitError> {
		Ok(())
	}

	fn refund_external_cost(&mut self, _ref_time: Option<u64>, _proof_size: Option<u64>) {}

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
		self.is_static
	}

	fn gas_limit(&self) -> Option<u64> {
		None
	}
}
