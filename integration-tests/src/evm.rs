#![cfg(test)]

use crate::polkadot_test_net::*;
use hydradx_runtime::evm::precompile::multicurrency::{Action, MultiCurrencyPrecompile};
use pallet_evm::*;
use sp_core::{H160, H256, U256};
use xcm_emulator::TestExt;
type CurrencyPrecompile = MultiCurrencyPrecompile<hydradx_runtime::Runtime>;
use fp_evm::{Context, Transfer};
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::evm::precompile::handle::EvmDataWriter;
use hydradx_runtime::evm::precompile::Bytes;
use hydradx_runtime::evm::precompiles::{addr, HydraDXPrecompiles};
use hydradx_runtime::{Tokens, EVM};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;

#[test]
fn evm1() {
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

// TODO: test dispatch should respect call filter

// TODO: test EVM fees should be handled the same way as substrate ones - tranfered to treasury

#[test]
fn dispatch_should_work_with_transfer() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
			.to_vec();
		let balance = Tokens::free_balance(WETH, &evm_account());
		let transferred = 1 * 10u128.pow(16);

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
		let new_balance = Tokens::free_balance(WETH, &evm_account());
		assert!(new_balance < balance - transferred);
		println!("fee: {:?}", balance - (new_balance + transferred));
	});
}

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
