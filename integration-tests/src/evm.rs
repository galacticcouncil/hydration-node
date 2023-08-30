#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::codec::Encode;
use hydradx_runtime::evm::precompile::multicurrency::{Action, MultiCurrencyPrecompile};
use pallet_evm::*;
use sp_core::{blake2_256, H160, H256, U256};
use xcm_emulator::TestExt;
type CurrencyPrecompile = MultiCurrencyPrecompile<hydradx_runtime::Runtime>;
//use pallet_evm::Transfer;
use fp_evm::{Context, Transfer};
use hydradx_runtime::evm::precompile::handle::EvmDataWriter;
use hydradx_runtime::evm::precompile::{Bytes, EvmAddress};
use hydradx_runtime::evm::precompiles::{addr, HydraDXPrecompiles};
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
				address: alice_evm_addr(),
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
fn dispatch_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = EvmDataWriter::new_with_selector(Action::Name).build();

		let mut handle = MockHandle {
			input: data,
			context: Context {
				address: alice_evm_addr(),
				caller: native_asset_ethereum_address(),
				apparent_value: U256::from(10),
			},
			core_address: DISPATCH_ADDR,
		};

		//Act
		let prec = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
		let result = prec.execute(&mut handle);

		assert!(result.is_some());

		//Assert
		/*	let output = EvmDataWriter::new().write(Bytes::from("HDX".as_bytes())).build();
		assert_eq!(
			result,
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output
			})
		);*/
	});
}

const DISPATCH_ADDR: H160 = addr(1025);

pub const ALICE_ACCOUNT: AccountId = AccountId::new([1u8; 32]);

pub fn alice_evm_addr() -> H160 {
	//H160::from(hex_literal::hex!("1000000000000000000000000000000000000001"))
	//EvmAddressMapping::<hydradx_runtime::Runtime>::get_default_evm_address(&ALICE)

	account_to_default_evm_address(&ALICE_ACCOUNT)
}

// Creates a an EvmAddress from an AccountId by appending the bytes "evm:" to
// the account_id and hashing it.
fn account_to_default_evm_address(account_id: &impl Encode) -> EvmAddress {
	let payload = (b"evm:", account_id);
	EvmAddress::from_slice(&payload.using_encoded(blake2_256)[0..20])
}

pub fn native_asset_ethereum_address() -> H160 {
	H160::from(hex_literal::hex!("0000000000000000000100000000000000000000"))
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
