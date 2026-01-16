#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::assert_noop;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::storage::with_transaction;
use frame_support::traits::OnFinalize;
use frame_support::traits::OnInitialize;
use frame_support::{
	assert_ok,
	sp_runtime::{FixedU128, Permill},
	traits::tokens::fungibles::Mutate,
};
use pallet_evm::ExitReason;
use hydradx_traits::evm::EvmAddress;
use hex_literal::hex;
use hydra_dx_math::ema::smoothing_from_period;
use hydradx_runtime::bifrost_account;
use hydradx_runtime::evm::precompiles::chainlink_adapter::AggregatorInterface;
use hydradx_runtime::evm::precompiles::chainlink_adapter::ChainlinkOraclePrecompile;
use hydradx_runtime::AssetLocation;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Omnipool;
use hydradx_runtime::Runtime;
use hydradx_runtime::{EmaOracle, RuntimeOrigin};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use hydradx_traits::{
	AggregatedPriceOracle,
	OraclePeriod::{self, *},
};
use sp_core::crypto::AccountId32;
use pallet_evm::PrecompileFailure;
use orml_traits::MultiCurrency;
use pallet_ema_oracle::into_smoothing;
use pallet_ema_oracle::OracleError;
use pallet_ema_oracle::BIFROST_SOURCE;
use pallet_evm::ExitSucceed;
use pallet_transaction_payment::ChargeTransactionPayment;
use precompile_utils::prelude::PrecompileOutput;
use primitives::constants::chain::{OMNIPOOL_SOURCE, XYK_SOURCE};
use sp_core::U256;
use sp_runtime::traits::SignedExtension;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::DispatchResult;
use sp_runtime::TransactionOutcome;
use sp_std::sync::Arc;
use xcm_emulator::TestExt;
use pallet_evm::Precompile;
use hydradx_runtime::evm::precompiles::chainlink_adapter::encode_oracle_address;
 use pallet_evm::Context;
use crate::evm::MockHandle;
use precompile_utils::evm::writer::EvmDataWriter;
use hydradx_runtime::EVMAccounts;
use sp_core::H160;
use crate::utils::contracts::{get_contract_bytecode, deploy_contract_code, deploy_contract, append_constructor_args};
 use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApiV5;
 use ethabi::{encode, decode, ParamType, Token};
use tiny_keccak::{Hasher, Keccak};


fn call_u256(selector: AggregatorInterface, oracle: H160) -> U256 {
	let input = EvmDataWriter::new_with_selector(selector).build();

	let mut handle = MockHandle {
		input,
		context: Context {
			address: evm_address(),
			caller: oracle,
			apparent_value: U256::zero(),
		},
		code_address: oracle,
		is_static: true,
	};

	let PrecompileOutput { output, exit_status } = ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

	assert_eq!(exit_status, ExitSucceed::Returned);
	U256::from_big_endian(&output)
}

fn call_bytes(selector: AggregatorInterface, oracle: H160) -> Vec<u8> {
	let input = EvmDataWriter::new_with_selector(selector).build();

	let mut handle = MockHandle {
		input,
		context: Context {
			address: evm_address(),
			caller: oracle,
			apparent_value: U256::zero(),
		},
		code_address: oracle,
		is_static: true,
	};

	let PrecompileOutput { output, exit_status } = ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

	assert_eq!(exit_status, ExitSucceed::Returned);
	output
}

	fn setup_chainlink_oracle_address() -> H160 {
		hydradx_run_to_next_block();
		init_omnipool();

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);
		assert_ok!(Omnipool::add_token(
			RuntimeOrigin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			5 * UNITS,
			0,
		));

		hydradx_run_to_next_block();

		assert_ok!(EmaOracle::get_price(HDX, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE));
		assert_ok!(EmaOracle::get_price(DOT, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE));

		encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE)
	}

	fn exec_precompile(input: Vec<u8>, oracle: H160) -> Result<Vec<u8>, PrecompileFailure> {
		let mut handle = MockHandle {
			input,
			context: Context {
				address: evm_address(),
				caller: H160::repeat_byte(0x11),
				apparent_value: U256::zero(),
			},
			code_address: oracle,
			is_static: true,
		};

		let out = ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle)?;
		assert_eq!(out.exit_status, ExitSucceed::Returned);
		Ok(out.output)
	}

	fn exec_precompile_err(input: Vec<u8>, oracle: H160) -> PrecompileFailure {
		let mut handle = MockHandle {
			input,
			context: Context {
				address: evm_address(),
				caller: H160::repeat_byte(0x11),
				apparent_value: U256::zero(),
			},
			code_address: oracle,
			is_static: true,
		};

		ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).err().unwrap()
	}

	fn decode_u256(output: Vec<u8>) -> U256 {
		U256::from_big_endian(&output)
	}

pub fn deploy_clamped_oracle(
	deployer: EvmAddress,
	primary: EvmAddress,
	secondary: EvmAddress,
	max_diff_bps: u64,
) -> EvmAddress {
	let initcode = get_contract_bytecode("ClampedOracle");

	let code = append_constructor_args(
		initcode,
		vec![
			Token::Address(primary.into()),
			Token::Address(secondary.into()),
			Token::Uint(U256::from(max_diff_bps)),
		],
	);

	deploy_contract_code(code, deployer)
}

fn selector(sig: &str) -> [u8; 4] {
	let mut keccak = Keccak::v256();
	let mut out = [0u8; 32];
	keccak.update(sig.as_bytes());
	keccak.finalize(&mut out);
	[out[0], out[1], out[2], out[3]]
}

fn calldata(sig: &str, args: Vec<Token>) -> Vec<u8> {
	let mut data = Vec::with_capacity(4 + 32 * args.len());
	data.extend_from_slice(&selector(sig));
	data.extend_from_slice(&encode(&args));
	data
}

fn evm_view(caller: EvmAddress, contract: EvmAddress, data: Vec<u8>) -> Vec<u8> {
	let info = hydradx_runtime::Runtime::call(
		caller,
		contract,
		data,
		U256::zero(),
		U256::from(15_000_000u64),
		None,
		None,
		None,
		true,
		None,
	);

	let out = info.unwrap();
	match out.exit_reason {
		ExitReason::Succeed(_) => out.value,
		reason => panic!("{:?}", reason),
	}
}

fn evm_call(caller: EvmAddress, contract: EvmAddress, data: Vec<u8>) {
	let info = hydradx_runtime::Runtime::call(
		caller,
		contract,
		data,
		U256::zero(),
		U256::from(15_000_000u64),
		None,
		None,
		None,
		false,
		None,
	);

	let out = info.unwrap();
	match out.exit_reason {
		ExitReason::Succeed(_) => {}
		reason => panic!("{:?}", reason),
	}
}

fn decode_u8(output: Vec<u8>) -> u8 {
	let t = decode(&[ParamType::Uint(8)], &output).unwrap();
	match &t[0] {
		Token::Uint(v) => v.low_u32() as u8,
		_ => panic!("unexpected return"),
	}
}

#[test]
fn aggregator_should_be_callable_after_setup() {
	TestNet::reset();

	Hydra::execute_with(|| {
		hydradx_run_to_next_block();
		init_omnipool();

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);
		assert_ok!(Omnipool::add_token(
			RuntimeOrigin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			5 * UNITS,
			0,
		));

		hydradx_run_to_next_block();

		assert_ok!(EmaOracle::get_price(HDX, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE));
		assert_ok!(EmaOracle::get_price(DOT, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE));

		let oracle = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let decimals = call_u256(AggregatorInterface::Decimals, oracle);
		assert_eq!(decimals, U256::from(8u64));

		let price = call_u256(AggregatorInterface::LatestAnswer, oracle);
		assert!(price > U256::zero());
	});
}

	#[test]
	fn aggregator_should_fail_for_non_existing_oracle_address() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let oracle = H160::from(hex!("000001026f6d6e69706f6f6c0000007b0000007c"));

			let input = EvmDataWriter::new_with_selector(AggregatorInterface::LatestAnswer).build();

			let mut handle = MockHandle {
				input,
				context: Context {
					address: evm_address(),
					caller: oracle,
					apparent_value: U256::zero(),
				},
				code_address: oracle,
				is_static: true,
			};

			let res = ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle);
			assert!(res.is_err());
		});
	}

#[test]
fn clamped_oracle_should_deploy() {
	TestNet::reset();

	Hydra::execute_with(|| {

		let primary: EvmAddress = deploy_contract("MockAggregator", crate::erc20::deployer());

		let secondary: EvmAddress = encode_oracle_address(
	HDX,
	DOT,
	OraclePeriod::Short,
	OMNIPOOL_SOURCE,
);

		let clamped_init = get_contract_bytecode("ClampedOracle");
		let clamped_code = append_constructor_args(
			clamped_init,
			vec![
				Token::Address(ethabi::Address::from_slice(primary.as_bytes())),
				Token::Address(ethabi::Address::from_slice(secondary.as_bytes())),
				Token::Uint(U256::from(1000u64)),
			],
		);

		let clamped: EvmAddress = deploy_contract_code(clamped_code, crate::erc20::deployer());

		let deployed = hydradx_runtime::Runtime::account_code_at(clamped);
		assert_ne!(deployed, vec![0; deployed.len()]);
	});
}


#[test]
fn clamped_oracle_latest_answer_uses_chainlink_secondary_and_clamps() {
	TestNet::reset();

	Hydra::execute_with(|| {
		hydradx_run_to_next_block();
		init_omnipool();

		let token_price = sp_runtime::FixedU128::from_inner(25_650_000_000_000_000_000);
		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			token_price,
			sp_runtime::Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			5 * UNITS,
			0,
		));

		hydradx_run_to_next_block();

		let deployer: EvmAddress = crate::erc20::deployer();

		let primary: EvmAddress = deploy_contract("MockAggregator", deployer);

		let secondary: EvmAddress = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let secondary_price = {
			let out = evm_view(deployer, secondary, calldata("latestAnswer()", vec![]));
			let s = decode_u256_int256(out);
			assert!(s > U256::zero());
			s
		};

		let max_diff_bps = 1000u64;

		let clamped_code = append_constructor_args(
			get_contract_bytecode("ClampedOracle"),
			vec![
				Token::Address(ethabi::Address::from_slice(primary.as_bytes())),
				Token::Address(ethabi::Address::from_slice(secondary.as_bytes())),
				Token::Uint(U256::from(max_diff_bps)),
			],
		);

		let clamped: EvmAddress = deploy_contract_code(clamped_code, deployer);

		let high = secondary_price * U256::from(12_000u64) / U256::from(10_000u64);

		evm_call(
			deployer,
			primary,
			calldata(
				"pushAnswer(int256,uint256)",
				vec![Token::Int(high.into()), Token::Uint(U256::from(123u64))],
			),
		);

		let out = evm_view(deployer, clamped, calldata("latestAnswer()", vec![]));
		let got = decode_u256_int256(out);

		let expected = secondary_price * U256::from(10_000u64 + max_diff_bps) / U256::from(10_000u64);
		assert_eq!(got, expected);
	});
}

#[test]
fn clamped_oracle_primary_zero_falls_back_to_chainlink_secondary() {
	TestNet::reset();

	Hydra::execute_with(|| {
		hydradx_run_to_next_block();
		init_omnipool();

		let token_price = sp_runtime::FixedU128::from_inner(25_650_000_000_000_000_000);
		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			token_price,
			sp_runtime::Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			5 * UNITS,
			0,
		));

		hydradx_run_to_next_block();

		let deployer: EvmAddress = crate::erc20::deployer();
		let primary: EvmAddress = deploy_contract("MockAggregator", deployer);
		let secondary: EvmAddress = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let secondary_price = decode_u256_int256(evm_view(deployer, secondary, calldata("latestAnswer()", vec![])));

		let clamped: EvmAddress = deploy_contract_code(
			append_constructor_args(
				get_contract_bytecode("ClampedOracle"),
				vec![
					Token::Address(ethabi::Address::from_slice(primary.as_bytes())),
					Token::Address(ethabi::Address::from_slice(secondary.as_bytes())),
					Token::Uint(U256::from(1000u64)),
				],
			),
			deployer,
		);

		evm_call(
			deployer,
			primary,
			calldata(
				"pushAnswer(int256,uint256)",
				vec![Token::Int(U256::zero().into()), Token::Uint(U256::from(1u64))],
			),
		);

		let got = decode_u256_int256(evm_view(deployer, clamped, calldata("latestAnswer()", vec![])));
		assert_eq!(got, secondary_price);
	});
}

#[test]
fn clamped_oracle_latest_answer_clamps_above_band_using_chainlink_secondary() {
	TestNet::reset();

	Hydra::execute_with(|| {
		seed_oracle_price_via_omnipool();

		let who: AccountId32 = AccountId32::from(ALICE);
		let deployer: EvmAddress = EVMAccounts::evm_address(&who);

		let primary = deploy_contract("MockAggregator", deployer);
		let secondary = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let s = read_chainlink_secondary_price(deployer, secondary);
		assert!(s > U256::zero());

		let max_diff_bps = 1000u64;
		let clamped = deploy_clamped_oracle(deployer, primary, secondary, max_diff_bps);

		let high = s * U256::from(12_000u64) / U256::from(10_000u64);
		set_primary_answer(deployer, primary, high, 111);

		let got = read_clamped_price(deployer, clamped);
		let expected = s * U256::from(10_000u64 + max_diff_bps) / U256::from(10_000u64);
		assert_eq!(got, expected);
	});
}

#[test]
fn clamped_oracle_latest_answer_clamps_below_band_using_chainlink_secondary() {
	TestNet::reset();

	Hydra::execute_with(|| {
		seed_oracle_price_via_omnipool();

		let who: AccountId32 = AccountId32::from(ALICE);
		let deployer: EvmAddress = EVMAccounts::evm_address(&who);

		let primary = deploy_contract("MockAggregator", deployer);
		let secondary = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let s = read_chainlink_secondary_price(deployer, secondary);
		assert!(s > U256::zero());

		let max_diff_bps = 1000u64;
		let clamped = deploy_clamped_oracle(deployer, primary, secondary, max_diff_bps);

		let low = s * U256::from(8_000u64) / U256::from(10_000u64);
		set_primary_answer(deployer, primary, low, 222);

		let got = read_clamped_price(deployer, clamped);
		let expected = s * U256::from(10_000u64 - max_diff_bps) / U256::from(10_000u64);
		assert_eq!(got, expected);
	});
}

#[test]
fn clamped_oracle_within_band_returns_primary() {
	TestNet::reset();

	Hydra::execute_with(|| {
		seed_oracle_price_via_omnipool();

		let who: AccountId32 = AccountId32::from(ALICE);
		let deployer: EvmAddress = EVMAccounts::evm_address(&who);

		let primary = deploy_contract("MockAggregator", deployer);
		let secondary = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let s = read_chainlink_secondary_price(deployer, secondary);
		assert!(s > U256::zero());

		let max_diff_bps = 1000u64;
		let clamped = deploy_clamped_oracle(deployer, primary, secondary, max_diff_bps);

		let within = s * U256::from(10_050u64) / U256::from(10_000u64);
		set_primary_answer(deployer, primary, within, 333);

		let got = read_clamped_price(deployer, clamped);
		assert_eq!(got, within);
	});
}

#[test]
fn clamped_oracle_decimals_is_8() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let who: AccountId32 = AccountId32::from(ALICE);
		let deployer: EvmAddress = EVMAccounts::evm_address(&who);

		let primary = deploy_contract("MockAggregator", deployer);
		let secondary = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

		let clamped = deploy_clamped_oracle(deployer, primary, secondary, 1000);

		let out = evm_view(deployer, clamped, calldata("decimals()", vec![]));
		assert_eq!(decode_u8(out), 8u8);
	});
}

fn decode_u256_uint256(output: Vec<u8>) -> U256 {
    let t = ethabi::decode(&[ethabi::ParamType::Uint(256)], &output).unwrap();
    match &t[0] {
        ethabi::Token::Uint(u) => *u,
        _ => panic!("unexpected return"),
    }
}

fn decode_u256_int256(output: Vec<u8>) -> U256 {
    let t = ethabi::decode(&[ethabi::ParamType::Int(256)], &output).unwrap();
    match &t[0] {
        ethabi::Token::Int(i) => *i,
        _ => panic!("unexpected return"),
    }
}

fn seed_oracle_price_via_omnipool() {
	hydradx_run_to_next_block();
	init_omnipool();

	let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		token_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
		HDX,
		DOT,
		5 * UNITS,
		0,
	));

	hydradx_run_to_next_block();
}

fn set_primary_answer(deployer: EvmAddress, primary: EvmAddress, ans: U256, ts: u64) {
	evm_call(
		deployer,
		primary,
		calldata(
			"pushAnswer(int256,uint256)",
			vec![Token::Int(ans), Token::Uint(U256::from(ts))],
		),
	);
}

fn read_chainlink_secondary_price(deployer: EvmAddress, secondary: EvmAddress) -> U256 {
	let out = evm_view(deployer, secondary, calldata("latestAnswer()", vec![]));
	decode_u256_uint256(out)
}

fn read_clamped_price(deployer: EvmAddress, clamped: EvmAddress) -> U256 {
	let out = evm_view(deployer, clamped, calldata("latestAnswer()", vec![]));
	decode_u256_int256(out)
}