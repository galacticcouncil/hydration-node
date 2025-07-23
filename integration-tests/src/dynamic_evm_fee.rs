#![cfg(test)]

use crate::{assert_balance, polkadot_test_net::*};
use fp_evm::{Context, Transfer};
use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApi;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Mutate;
use frame_support::{assert_ok, dispatch::GetDispatchInfo, sp_runtime::codec::Encode, traits::Contains};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_core::bounded_vec::BoundedVec;

use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
use hydradx_runtime::evm::EvmAddress;
use hydradx_runtime::evm::ExtendedAddressMapping;
use hydradx_runtime::evm::Function;
use hydradx_runtime::{
	evm::precompiles::{
		handle::EvmDataWriter, multicurrency::MultiCurrencyPrecompile, Address, Bytes, HydraDXPrecompiles,
	},
	AssetRegistry, Balances, CallFilter, Currencies, EVMAccounts, Omnipool, RuntimeCall, RuntimeOrigin, Tokens,
	TransactionPause, EVM,
};
use hydradx_runtime::{DynamicEvmFee, XYK};
use hydradx_traits::router::{PoolType, Trade};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_evm::*;
use pretty_assertions::assert_eq;
use primitives::{AssetId, Balance};
use sp_core::{blake2_256, H160, H256, U256};
use sp_runtime::TransactionOutcome;
use sp_runtime::{traits::SignedExtension, DispatchError, FixedU128, Permill};
use std::{borrow::Cow, cmp::Ordering};
use xcm_emulator::TestExt;

pub const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

#[test]
fn gas_price_scales_when_setting_other_eth_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();

		let min_gas_price = hydradx_runtime::DynamicEvmFee::min_gas_price();
		assert_eq!(min_gas_price.0, U256::from(36881265));

		DynamicEvmFee::set_evm_asset(hydradx_runtime::RuntimeOrigin::root(), WRAPPED_ETH);
		hydradx_run_to_next_block();

		let min_gas_price = hydradx_runtime::DynamicEvmFee::min_gas_price();
		assert_eq!(min_gas_price.0, U256::from(36316607));
	})
}

pub fn init_omnipool_with_oracle_for_block_10() {
	init_omnipol();
	hydradx_run_to_next_block();
	do_trade_to_populate_oracle(WETH, DOT, 1_000_000_000_000);
	do_trade_to_populate_oracle(WRAPPED_ETH, DOT, 1_000_000_000_000);
	let to = 20;
	let from = 11;
	for _ in from..=to {
		hydradx_run_to_next_block();
		do_trade_to_populate_oracle(DOT, HDX, 1_000_000_000_000);
		do_trade_to_populate_oracle(DAI, HDX, 1_000_000_000_000);
		do_trade_to_populate_oracle(WETH, DOT, 1_000_000_000_000);
		do_trade_to_populate_oracle(WRAPPED_ETH, DOT, 1_000_000_000_000);
	}
}

fn do_trade_to_populate_oracle(asset_1: AssetId, asset_2: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_2,
		amount,
		Balance::MIN
	));
}

pub fn init_omnipol() {
	let native_price = FixedU128::from_rational(29903049701668757, 73927734532192294158);
	let dot_price = FixedU128::from_rational(103158291366950047, 4566210555614178);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let acc = hydradx_runtime::Omnipool::protocol_account();

	let stable_amount = 50_000_000 * UNITS * 1_000_000;
	let dot_amount: Balance = 4566210555614178u128;
	let native_amount: Balance = 73927734532192294158u128;
	let weth_amount: Balance = 1074271742496220564487u128;
	let weth_price = FixedU128::from_rational(67852651072676287, 1074271742496220564487);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DOT,
		dot_amount,
		0
	));
	Balances::set_balance(&acc, native_amount);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		WETH,
		weth_amount,
		0
	));

	//We add another ETH token as there are multiple ETH tokens on PROD
	let wrapped_eth_amount: Balance = 1004271742496220564487u128;
	let wrapped_eth_price = FixedU128::from_rational(69852651072676287, 1074271742496220564487);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		WRAPPED_ETH,
		wrapped_eth_amount,
		0
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
		DOT,
		dot_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		WETH,
		weth_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		WRAPPED_ETH,
		wrapped_eth_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), acc, DAI, stable_amount, 0));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		hydradx_runtime::Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
	));
}

// TODO: test that we charge approximatelly same fee on evm as with extrinsics directly
pub fn gas_price() -> U256 {
	U256::from(hydradx_runtime::evm::DEFAULT_BASE_FEE_PER_GAS)
}

impl MockHandle {
	pub fn new_dispatch(sender: H160, data: Vec<u8>) -> Self {
		Self {
			input: data,
			context: Context {
				address: DISPATCH_ADDR,
				caller: sender,
				apparent_value: U256::zero(),
			},
			code_address: DISPATCH_ADDR,
			is_static: true,
		}
	}
}

pub fn create_dispatch_handle(data: Vec<u8>) -> MockHandle {
	MockHandle::new_dispatch(evm_address(), data)
}

pub fn native_asset_ethereum_address() -> H160 {
	H160::from(hex!("0000000000000000000000000000000100000000"))
}

pub fn dai_ethereum_address() -> H160 {
	H160::from(hex!("0000000000000000000000000000000100000002"))
}

pub struct MockHandle {
	pub input: Vec<u8>,
	pub context: Context,
	pub code_address: H160,
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
		self.code_address
	}

	fn input(&self) -> &[u8] {
		&self.input
	}

	fn context(&self) -> &Context {
		&self.context
	}

	fn origin(&self) -> H160 {
		todo!()
	}

	fn is_static(&self) -> bool {
		self.is_static
	}

	fn gas_limit(&self) -> Option<u64> {
		None
	}

	fn is_contract_being_constructed(&self, address: H160) -> bool {
		todo!()
	}
}

fn create_xyk_pool_with_amounts(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}
