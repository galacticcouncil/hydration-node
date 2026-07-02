#![cfg(test)]

use crate::polkadot_test_net::*;
use ethabi::encode;
use ethabi::Token;
use fp_evm::ExitReason::Succeed;
use fp_evm::ExitSucceed::Returned;
use fp_evm::ExitSucceed::Stopped;
use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApi;
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::erc20_mapping::runtime_decl_for_erc_20_mapping_api::Erc20MappingApi;
use hydradx_runtime::evm::precompiles::erc20_mapping::runtime_decl_for_erc_20_mapping_api::HydraErc20Mapping;
use hydradx_runtime::Runtime;
use hydradx_runtime::{
	evm::{precompiles::handle::EvmDataWriter, Executor},
	AccountId, Currencies, EVMAccounts, Liquidation, RuntimeOrigin,
};
use hydradx_traits::evm::Erc20Encoding;
use hydradx_traits::evm::{CallContext, InspectEvmAccounts, EVM};
use orml_traits::MultiCurrency;
use pallet_currencies_rpc_runtime_api::runtime_decl_for_currencies_api::CurrenciesApi;
use pepl_support::traits::RuntimeApiErr;
use pepl_support::traits::RuntimeApiProvider;
use pepl_worker::contracts;
use pepl_worker_support as pepl_support;
use pepl_worker_support::types::Borrower;
use pepl_worker_support::types::EModeCategory;
use pepl_worker_support::types::LiquidationAmounts;
use pepl_worker_support::types::MoneyMarket;
use pepl_worker_support::types::ReserveOpp;
use pepl_worker_support::types::Timestamp;
use pepl_worker_support::types::UserConfiguration;
use pepl_worker_support::types::UserReserve;
use pepl_worker_support::Function;
use pepl_worker_support::Hydration;
use pretty_assertions::assert_eq;
use primitives::EvmAddress;
use primitives::{AssetId, Balance};
use sp_core::H160;
use sp_core::H256;
use sp_core::U256;
use sp_runtime::traits::Block;
use sp_runtime::BoundedVec;
use xcm_emulator::Network;

// v1 worker decision path (kept as dead code for the v1-vs-v2 comparison). v1 uses its own
// support crate + `ApiProvider` (a different `RuntimeApiProvider` trait), imported here with
// aliases so the parity harness can drive both workers from one underwater position.
use crate::liquidation::ApiProvider as V1ApiProvider;
use hydradx_runtime::{OriginCaller, RuntimeCall, RuntimeEvent};
use liquidation_worker_support::MoneyMarketData as V1MoneyMarketData;
use liquidation_worker_support::UserData as V1UserData;

const LOG_PREFIX: &str = "tests-log-prefix";

pub const PATH_TO_SNAPSHOT: &str = "snapshots/pepl/b49b947a954942f74e23d4f46b668fff00bc8b4f8105c8b905222a3bf76ea308";
pub const PATH_TO_SNAPSHOT_2: &str = "evm-snapshot/LIQUIDATION_SNAPSHOT";

const TARGET_HF: u128 = 1_001_000_000_000_000_000;

const HDX: AssetId = 0;
const DOT: AssetId = 5;
const DOT_UNIT: Balance = 10_000_000_000;
const WETH: AssetId = 20;
const WETH_UNIT: Balance = 1_000_000_000_000_000_000;
const ALICE_INITIAL_WETH_BALANCE: Balance = 20 * WETH_UNIT;
const ALICE_INITIAL_DOT_BALANCE: Balance = 10_000 * DOT_UNIT;

//SNAPSHOT_2's accounts
const PAP_CONTRACT: EvmAddress = H160(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6"));
pub const ORACLE_CALLER: EvmAddress = H160(hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
pub const ORACLE_ADDRESS: EvmAddress = H160(hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));

pub fn supply(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let context = CallContext::new_call(mm_pool, user);
	let data = EvmDataWriter::new_with_selector(Function::Supply)
		.write(asset)
		.write(amount)
		.write(user)
		.write(0u32)
		.build();

	let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(
		call_result.exit_reason,
		Succeed(Returned),
		"{:?}",
		hex::encode(call_result.value)
	);
}

pub fn borrow(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let context = CallContext::new_call(mm_pool, user);
	let data = EvmDataWriter::new_with_selector(Function::Borrow)
		.write(asset)
		.write(amount)
		.write(2u32)
		.write(0u32)
		.write(user)
		.build();

	let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 50_000_000);
	assert_eq!(
		call_result.exit_reason,
		Succeed(Returned),
		"{:?}",
		hex::encode(call_result.value)
	);
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct UserAccountData {
	pub total_collateral_base: U256,
	pub total_debt_base: U256,
	pub available_borrows_base: U256,
	pub current_liquidation_threshold: U256,
	pub ltv: U256,
	pub health_factor: U256,
}
pub fn get_user_account_data(mm_pool: EvmAddress, user: EvmAddress) -> Option<UserAccountData> {
	let context = CallContext::new_call(mm_pool, user);
	let mut data = Into::<u32>::into(Function::GetUserAccountData).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(user).as_bytes());

	let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(
		call_result.exit_reason,
		Succeed(Returned),
		"{:?}",
		hex::encode(call_result.value)
	);

	let total_collateral_base = U256::from_big_endian(&call_result.value[0..32]);
	let total_debt_base = U256::from_big_endian(&call_result.value[32..64]);
	let available_borrows_base = U256::from_big_endian(&call_result.value[64..96]);
	let current_liquidation_threshold = U256::from_big_endian(&call_result.value[96..128]);
	let ltv = U256::from_big_endian(&call_result.value[128..160]);
	let health_factor = U256::from_big_endian(&call_result.value[160..192]);

	Some(UserAccountData {
		total_collateral_base,
		total_debt_base,
		available_borrows_base,
		current_liquidation_threshold,
		ltv,
		health_factor,
	})
}

fn assert_health_factor_is_within_tolerance(health_factor: U256, target_health_factor: U256) {
	let health_factor_diff = health_factor.abs_diff(target_health_factor);
	// HF uses 18 decimal places
	assert!(
		health_factor_diff < U256::from(10).pow(15.into()),
		"HF diff: {:?}",
		health_factor_diff
	);
}

pub fn update_oracle_price(oracle_data: Vec<(&str, U256)>, oracle_address: EvmAddress, oracle_caller: EvmAddress) {
	let context = CallContext::new_call(oracle_address, oracle_caller);

	let mut data = Into::<u32>::into(Function::SetMultipleValues).to_be_bytes().to_vec();

	let mut token_string_array = Vec::new();
	let mut token_uint_array = Vec::new();

	for data in oracle_data.iter() {
		token_string_array.push(Token::String(data.0.to_string()));
		token_uint_array.push(Token::Uint(data.1));
	}

	let encoded_values = encode(&[Token::Array(token_string_array), Token::Array(token_uint_array)]);

	data.extend_from_slice(&encoded_values);

	let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 5_000_000);
	assert_eq!(
		call_result.exit_reason,
		Succeed(Stopped),
		"{:?}",
		hex::encode(call_result.value)
	);
}

fn deposit_hdx_to_protocol_account() {
	// We need to deposit HDX to omnipool account since the snapshot doesn't include System pallet
	// (native HDX balance is stored in frame_system::Account, not orml_tokens)
	let omnipool_account = hydradx_runtime::Omnipool::protocol_account();
	assert_ok!(Currencies::deposit(
		HDX,
		&omnipool_account,
		1_000_000_000_000_000_000_000u128
	));
}

pub fn get_oracle_price(asset_pair: &str) -> Option<(U256, U256)> {
	// contains addresses from mainnet and testnet to support different snapshots
	let oracle_addresses = [
		EvmAddress::from_slice(&hex!("dee629af973ebf5bf261ace12ffd1900ac715f5e")),
		EvmAddress::from_slice(&hex!("48ae7803cd09c48434e3fc5629f15fb76f0b5ce5")),
		EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917")),
		EvmAddress::from_slice(&hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52")),
	];

	for oracle_address in oracle_addresses.iter() {
		let context = CallContext::new_view(*oracle_address);
		let mut data = Into::<u32>::into(Function::GetValue).to_be_bytes().to_vec();
		let encoded_value = encode(&[Token::String(asset_pair.to_string())]);
		data.extend_from_slice(&encoded_value);

		let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 5_000_000);
		if call_result.exit_reason == Succeed(Returned) {
			let price = U256::from_big_endian(&call_result.value[0..32]);
			let timestamp = U256::from_big_endian(&call_result.value[32..64]);

			if !price.is_zero() {
				return Some((price, timestamp));
			} else {
				continue;
			}
		} else {
			continue;
		}
	}

	None
}

#[derive(sp_core::RuntimeDebug)]
pub struct ApiProvider<C>(pub C);

impl<B: Block, C> RuntimeApiProvider<B> for ApiProvider<C>
where
	C: EthereumRuntimeRPCApi<B> + Erc20MappingApi<B> + CurrenciesApi<B, AssetId, AccountId, Balance>,
{
	fn call(
		&self,
		_block: B::Hash,
		from: EvmAddress,
		to: EvmAddress,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr> {
		Ok(C::call(
			from,
			to,
			data,
			Zero::zero(),
			gas_limit,
			None,
			None,
			None,
			false,
			None,
			None,
		)?)
	}

	fn minimum_balance(&self, _block: B::Hash, asset_id: AssetId) -> Result<Balance, RuntimeApiErr> {
		Ok(C::minimum_balance(asset_id))
	}

	fn address_to_asset(&self, _block: B::Hash, address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr> {
		Ok(C::address_to_asset(address))
	}

	fn timestamp(&self, _block: <B as Block>::Hash) -> Option<Timestamp> {
		Runtime::current_block()
			.expect("runtime to have current_block")
			.header
			.timestamp
			.checked_div(1_000)
	}
}

#[test]
fn fetch_money_market_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hollar = H160(hex!["531a654d1696ed52e7275a8cede955e82620f99a"]);
		let eth = H160(hex!["0000000000000000000000000000000100000022"]);
		let dot = H160(hex!["0000000000000000000000000000000100000005"]);
		let usdc = H160(hex!["0000000000000000000000000000000100000016"]);

		let block = hydradx_runtime::System::block_hash(hydradx_runtime::System::block_number());
		let api = ApiProvider::<Runtime>(Runtime);

		let hydration = pepl_support::Hydration::new(
			contracts::RUNTIME_API_CALLER,
			contracts::POOL_ADDRESS_PROVIDER,
			LOG_PREFIX,
		);

		let mm = hydration.fetch_money_market(&api, block);
		assert!(mm.is_some());

		let mm = mm.expect("MomneyMarket to be some");
		let mm_hollar = mm.reserves.get(&hollar).expect("MoneyMarket to have HOLLAR");
		assert_eq!(mm_hollar.address, hollar);
		assert_eq!(mm_hollar.symbol, "HOLLAR".to_string());
		assert_eq!(mm_hollar.price, U256::from(100_000_000_u128));
		assert_eq!(mm_hollar.asset_id, 222);
		assert_eq!(mm_hollar.emode, None);
		assert_eq!(mm_hollar.data.configuration, U256::from(2671197528984125440_u128));

		let mm_eth = mm.reserves.get(&eth).expect("MoneyMarket to have ETH");
		assert_eq!(mm_eth.address, eth);
		assert_eq!(mm_eth.symbol, "ETH".to_string());
		assert_eq!(mm_eth.price, U256::from(230_350_568_365_u128));
		assert_eq!(mm_eth.asset_id, 34);
		assert_eq!(
			mm_eth.emode,
			Some(EModeCategory {
				liquidation_threshold: 9_000_u16,
				liquidation_bonus: 10_450_u16
			})
		);

		let mm_dot = mm.reserves.get(&dot).expect("MoneyMarket to have DOT");
		assert_eq!(mm_dot.address, dot);
		assert_eq!(mm_dot.symbol, "DOT".to_string());
		assert_eq!(mm_dot.price, U256::from(120_665_467_u128));
		assert_eq!(mm_dot.asset_id, 5);
		assert_eq!(
			mm_dot.emode,
			Some(EModeCategory {
				liquidation_threshold: 9_200_u16,
				liquidation_bonus: 10_450_u16
			})
		);

		let mm_usdc = mm.reserves.get(&usdc).expect("MoneyMarket to have USDC");
		assert_eq!(mm_usdc.address, usdc);
		assert_eq!(mm_usdc.symbol, "USDC".to_string());
		assert_eq!(mm_usdc.price, U256::from(99_981_389_u128));
		assert_eq!(mm_usdc.asset_id, 22);
		assert_eq!(
			mm_usdc.emode,
			Some(EModeCategory {
				liquidation_threshold: 9_300_u16,
				liquidation_bonus: 10_150_u16
			})
		);
	});
}

#[test]
fn fetch_borrower_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let dot = H160(hex!["0000000000000000000000000000000100000005"]);
		let vdot = H160(hex!["000000000000000000000000000000010000000f"]);
		let usdc = H160(hex!["0000000000000000000000000000000100000016"]);

		let block_number = hydradx_runtime::System::block_number();
		let block = hydradx_runtime::System::block_hash(block_number);
		let api = ApiProvider::<Runtime>(Runtime);

		let now = Runtime::current_block()
			.expect("runtime to have current_block")
			.header
			.timestamp
			/ 1_000;

		let exp_dot = Some(UserReserve {
			collateral: U256::from(23_262_963_331_u128),
			debt: U256::from(0_u128),
		});
		let expt_usdc = Some(UserReserve {
			collateral: U256::from(13_556_378_052_u128),
			debt: U256::from(30_440_549_054_u128),
		});
		let exp_vdot = Some(UserReserve {
			collateral: U256::from(6_888_450_327_u128),
			debt: U256::from(0_u128),
		});

		let mut exp_reserves: Vec<Option<UserReserve>> = vec![None; 22];
		exp_reserves[0] = expt_usdc.clone();
		exp_reserves[3] = exp_dot.clone();
		exp_reserves[4] = exp_vdot.clone();

		let expected = Borrower {
			configuration: UserConfiguration(U256::from(643_u128)),
			address: H160(hex!["288e0dbd476cbfc7dfc1268c00b9e5081e9d9b1a"]),
			emode_id: Some(U256::from(1_u128)),
			reserves: exp_reserves,
			total_debt: U256::from(30_440_549_054_u128),
			total_collateral: U256::from(43_707_791_710_u128),
			updated_at: block_number,
		};

		let hydration = pepl_support::Hydration::new(
			contracts::RUNTIME_API_CALLER,
			contracts::POOL_ADDRESS_PROVIDER,
			LOG_PREFIX,
		);

		let mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch MoneyMarket data to work");

		let who = H160(hex!("288e0dbd476cbfc7dfc1268c00b9e5081e9d9b1a"));
		let borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, who, now)
			.expect("fetch borrower to work");

		//Assert
		assert_eq!(borrower, expected);

		//Assert indexes works
		let idx = mm.reserves.get(&dot).expect("DOT to be in reserves").idx;
		assert_eq!(borrower.reserves[idx], exp_dot);

		let idx = mm.reserves.get(&vdot).expect("VDOT to be in reserves").idx;
		assert_eq!(borrower.reserves[idx], exp_vdot);

		let idx = mm.reserves.get(&usdc).expect("USDC to be in reserves").idx;
		assert_eq!(borrower.reserves[idx], expt_usdc);
	});
}

#[test]
fn calculate_debt_to_liquidate_with_same_collateral_and_debt_asset() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		// Arrange
		deposit_hdx_to_protocol_account();

		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		let caller = EVMAccounts::evm_address(&AccountId::from(CHARLIE));
		assert_ok!(Currencies::deposit(DOT, &CHARLIE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &CHARLIE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone())));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let api = ApiProvider::<Runtime>(Runtime);
		let hydration = pepl_support::Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let block = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = hydration.fetch_pool(&api, block).expect("fetch_pool() to work");
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		let collateral_weth_amount: Balance = 10 * WETH_UNIT;
		let collateral_dot_amount = 5_000 * DOT_UNIT;
		supply(
			pool_contract,
			alice_evm_address,
			weth_asset_address,
			collateral_weth_amount,
		);
		supply(
			pool_contract,
			alice_evm_address,
			dot_asset_address,
			collateral_dot_amount,
		);

		let borrow_dot_amount: Balance = 5_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);

		hydradx_run_to_next_block();

		// calculate HF before price update
		let mut mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch_money_market() to work");

		let now_evm = api.timestamp(block).expect("get timestamp for block to work");

		// HF > 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		// update MM and UserData structs based on future price
		let dot_address = get_asset_address(&mm, "DOT").expect("MoneyMarket to have DOT");
		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 6 / 2;
		mm.update_price(dot_address, new_price.into())
			.expect("MoneyMarket update price to work");

		let mut borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, alice_evm_address, now_evm)
			.expect("alice to have account");

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);

		let debt_reserve = mm.reserves.get(&dot_address).expect("MoneyMarket to have DOT");
		let coll_reserve = mm.reserves.get(&dot_address).expect("MoneyMarket to have DOT");
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency,
			collateral_in_base_currency,
		} = mm.calc_debt_to_liquidate(&borrower, target_health_factor, coll_reserve, debt_reserve)
			.expect("debt to liquidate calculation to work");

		borrower
			.update_reserve(coll_reserve.idx, ReserveOpp::SubCollateral(collateral_in_base_currency))
			.expect("Borrower to have DOT");
		borrower
			.update_reserve(coll_reserve.idx, ReserveOpp::SubDebt(debt_in_base_currency))
			.expect("Borrower to have DOT");

		let target_hf_diff = target_health_factor.abs_diff(
			borrower
				.calc_health_factor(&mm)
				.expect("borrower's health factor calculation to work"),
		);

		assert!(
			target_hf_diff
				< U256::from(1_000_000_000_000_000_000u128)
					.checked_div(10_000u128.into())
					.unwrap()
		);

		// update the price
		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 6 / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(
			vec![("DOT/USD", U256::from_big_endian(&data[0..32]))],
			ORACLE_ADDRESS,
			ORACLE_CALLER,
		);

		// ensure that the health_factor < 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			DOT, // collateral
			DOT, // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
			None,
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, target_health_factor);
	});
}

#[test]
fn calculate_debt_to_liquidate_with_different_collateral_and_debt_asset_and_debt_price_change() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		// Arrange
		deposit_hdx_to_protocol_account();

		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		let caller = EVMAccounts::evm_address(&AccountId::from(CHARLIE));
		assert_ok!(Currencies::deposit(DOT, &CHARLIE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &CHARLIE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let api = ApiProvider(Runtime);
		let hydration = Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let block = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = hydration.fetch_pool(&api, block).expect("fetch_pool to works");

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		let collateral_weth_amount: Balance = 10 * WETH_UNIT;
		let collateral_dot_amount = 5_000 * DOT_UNIT;
		supply(
			pool_contract,
			alice_evm_address,
			weth_asset_address,
			collateral_weth_amount,
		);
		supply(
			pool_contract,
			alice_evm_address,
			dot_asset_address,
			collateral_dot_amount,
		);

		let borrow_dot_amount: Balance = 5_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);

		hydradx_run_to_next_block();

		let mut mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch_money_market to works");

		let now_evm = api.timestamp(block).expect("get timestamp for block to work")
			+ primitives::constants::time::SECS_PER_BLOCK; // our calculations "happen" in the next block

		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 5 / 2;
		mm.update_price(dot_asset_address, new_price.into())
			.expect("update price to work");

		let borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, alice_evm_address, now_evm)
			.expect("fetch_borrower to work");

		let debt_reserve = mm.reserves.get(&dot_asset_address).expect("MoneyMarket to have DOT");
		let coll_reserve = mm.reserves.get(&weth_asset_address).expect("MoneyMarket to have WETH");
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = mm.calc_debt_to_liquidate(&borrower, TARGET_HF.into(), coll_reserve, debt_reserve)
			.expect("borrowers debt to liquidate calculation to work");

		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 5 / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(
			vec![("DOT/USD", U256::from_big_endian(&data[0..32]))],
			ORACLE_ADDRESS,
			ORACLE_CALLER,
		);

		// ensure that the health_factor < 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH, // collateral
			DOT,  // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
			None,
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, TARGET_HF.into());
	});
}

#[test]
fn calculate_debt_to_liquidate_collateral_amount_is_not_sufficient_to_reach_target_health_factor() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		// Arrange
		deposit_hdx_to_protocol_account();

		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		let caller = EVMAccounts::evm_address(&AccountId::from(CHARLIE));
		assert_ok!(Currencies::deposit(DOT, &CHARLIE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &CHARLIE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let api = ApiProvider(Runtime);
		let hydration = Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let block = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = hydration.fetch_pool(&api, block).expect("fetch_pool() to work");
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		let collateral_weth_amount: Balance = 10 * WETH_UNIT;
		let collateral_dot_amount = 5_000 * DOT_UNIT;
		supply(
			pool_contract,
			alice_evm_address,
			weth_asset_address,
			collateral_weth_amount,
		);
		supply(
			pool_contract,
			alice_evm_address,
			dot_asset_address,
			collateral_dot_amount,
		);

		let borrow_dot_amount: Balance = 5_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);

		hydradx_run_to_next_block();

		let mut mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch_money_market() to work");

		let now_evm = api.timestamp(block).expect("get timestamp for block to work")
			+ primitives::constants::time::SECS_PER_BLOCK; // our calculations "happen" in the next block

		let new_price = get_oracle_price("WETH/USD").unwrap().0.as_u128() / 3;
		mm.update_price(weth_asset_address, new_price.into())
			.expect("MoneyMarket.update_price() to work");

		let borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, alice_evm_address, now_evm)
			.expect("fetch_borrower() to work");

		let debt_reserve = mm.reserves.get(&dot_asset_address).expect("MoneyMarket to have DOT");
		let coll_reserve = mm.reserves.get(&weth_asset_address).expect("MoneyMarket to have WETH");
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = mm.calc_debt_to_liquidate(&borrower, TARGET_HF.into(), coll_reserve, debt_reserve)
			.expect("calc_debt_to_liquidate() to work");

		// update WETH price
		let (price, timestamp) = get_oracle_price("WETH/USD").unwrap();
		let price = price.as_u128() / 3;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(
			vec![("WETH/USD", U256::from_big_endian(&data[0..32]))],
			ORACLE_ADDRESS,
			ORACLE_CALLER,
		);

		// ensure that the health_factor < 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		let weth_reserve = mm.reserves.get(&weth_asset_address).expect("MoneyMarket to have WETH");
		let coll_reserve = hydration
			.fetch_borrower_collateral_and_convert_to_base(&api, block, borrower.address, weth_reserve, now_evm)
			.expect("fetch_borrower_collateral_and_convert_to_base() to work");

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH, // collateral
			DOT,  // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
			None,
		));

		let mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch_money_market() to work");
		let borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, alice_evm_address, now_evm)
			.expect("fetch_borrower() to work");

		// Assert
		let remaining_coll_reserve = hydration
			.fetch_borrower_collateral_and_convert_to_base(&api, block, borrower.address, weth_reserve, now_evm)
			.expect("fetch_borrower_collateral_and_convert_to_base() to work");

		assert!(remaining_coll_reserve < coll_reserve / 1_000);
	});
}

#[test]
fn calculate_debt_to_liquidate_with_weth_as_debt() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		// Arrange
		deposit_hdx_to_protocol_account();

		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		let caller = EVMAccounts::evm_address(&AccountId::from(CHARLIE));
		assert_ok!(Currencies::deposit(DOT, &CHARLIE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &CHARLIE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let api = ApiProvider(Runtime);
		let hydration = Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let block = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = hydration.fetch_pool(&api, block).expect("fetch_pool() to work");
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		let collateral_weth_amount: Balance = 20 * WETH_UNIT;
		let collateral_dot_amount = 5_000 * DOT_UNIT;
		supply(
			pool_contract,
			alice_evm_address,
			weth_asset_address,
			collateral_weth_amount,
		);
		supply(
			pool_contract,
			alice_evm_address,
			dot_asset_address,
			collateral_dot_amount,
		);

		let borrow_weth_amount: Balance = 21 * WETH_UNIT;
		borrow(pool_contract, alice_evm_address, weth_asset_address, borrow_weth_amount);

		hydradx_run_to_next_block();

		let mut mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch_money_market() to work");

		let now_evm = api.timestamp(block).expect("get timestamp for block to work")
			+ primitives::constants::time::SECS_PER_BLOCK; // our calculations "happen" in the next block

		let new_price = get_oracle_price("WETH/USD").unwrap().0.as_u128() * 5 / 2;
		mm.update_price(weth_asset_address, new_price.into())
			.expect("update_price() to work");

		let borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, alice_evm_address, now_evm)
			.expect("fetch_borrower() to work");

		let debt_reserve = mm.reserves.get(&weth_asset_address).expect("MoneyMarket to have WETH");
		let coll_reserve = mm.reserves.get(&weth_asset_address).expect("MoneyMarket to have WETH");
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = mm.calc_debt_to_liquidate(&borrower, TARGET_HF.into(), coll_reserve, debt_reserve)
			.expect("calc_debt_to_liquidate() to work");

		let (price, timestamp) = get_oracle_price("WETH/USD").unwrap();
		let price = price.as_u128() * 5 / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(
			vec![("WETH/USD", U256::from_big_endian(&data[0..32]))],
			ORACLE_ADDRESS,
			ORACLE_CALLER,
		);

		// ensure that the health_factor < 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH, // collateral
			WETH, // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
			None,
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, TARGET_HF.into());
	});
}

#[test]
fn calculate_debt_to_liquidate_with_two_different_assets() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		// Arrange
		deposit_hdx_to_protocol_account();

		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		let caller = EVMAccounts::evm_address(&AccountId::from(CHARLIE));
		assert_ok!(Currencies::deposit(DOT, &CHARLIE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &CHARLIE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let api = ApiProvider(Runtime);
		let hydration = Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let block = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = hydration.fetch_pool(&api, block).expect("fetch_pool() to work");
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		let collateral_weth_amount: Balance = 10 * WETH_UNIT;
		supply(
			pool_contract,
			alice_evm_address,
			weth_asset_address,
			collateral_weth_amount,
		);

		let borrow_dot_amount: Balance = 2_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);

		hydradx_run_to_next_block();

		let mut mm = hydration
			.fetch_money_market(&api, block)
			.expect("fetch_money_market() to work");

		let now_evm = api.timestamp(block).expect("get timestamp for block to work");

		// ensure that the health_factor > 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 7 / 5;
		mm.update_price(dot_asset_address, new_price.into())
			.expect("update_price() to work");

		let borrower = hydration
			.fetch_borrower(&api, block, block_number, &mm, alice_evm_address, now_evm)
			.expect("fetch_borrower() to work");

		let debt_reserve = mm.reserves.get(&dot_asset_address).expect("MoneyMarket to have DOT");
		let coll_reserve = mm.reserves.get(&weth_asset_address).expect("MoneyMarket to have WETH");
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = mm.calc_debt_to_liquidate(&borrower, TARGET_HF.into(), coll_reserve, debt_reserve)
			.expect("calc_debt_to_liquidate() to work");

		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 7 / 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(
			vec![("DOT/USD", U256::from_big_endian(&data[0..32]))],
			ORACLE_ADDRESS,
			ORACLE_CALLER,
		);

		// ensure that the health_factor < 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH, // collateral
			DOT,  // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
			None,
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, TARGET_HF.into());
	});
}

fn get_asset_address(mm: &MoneyMarket, symbol: &str) -> Option<EvmAddress> {
	mm.reserves.iter().find_map(|(&addr, r)| {
		if r.symbol == symbol {
			return Some(addr);
		}
		return None;
	})
}

// ============================================================================
// Tier-1 parity harness: drive BOTH the v2 (`pepl-worker`) and v1
// (`liquidation-worker-support`) decision paths against the SAME synthetic
// underwater borrower, and assert each restores the health factor. This is the
// v1-vs-v2 behavioural comparison; v1 is kept as dead code purely to run these.
// ============================================================================

/// Sets up an underwater borrower on `LIQUIDATION_SNAPSHOT`: binds EVM accounts, sets the
/// borrowing contract, supplies WETH + DOT collateral, borrows DOT, then triples the DOT oracle
/// price so the health factor drops below 1. Returns `(pool_contract, borrower_evm, caller)`.
/// Must be called inside `hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(..)`.
fn create_unhealthy_borrower() -> (EvmAddress, EvmAddress, EvmAddress) {
	deposit_hdx_to_protocol_account();
	hydradx_run_to_next_block();

	let pallet_acc = Liquidation::account_id();
	let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
	let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

	let caller = EVMAccounts::evm_address(&AccountId::from(CHARLIE));
	assert_ok!(Currencies::deposit(DOT, &CHARLIE.into(), ALICE_INITIAL_DOT_BALANCE));
	assert_ok!(Currencies::deposit(WETH, &CHARLIE.into(), ALICE_INITIAL_WETH_BALANCE));
	assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
	assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

	assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));
	assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into())));
	assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone())));

	let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

	let api = ApiProvider::<Runtime>(Runtime);
	let hydration = Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);
	let block_number = hydradx_runtime::System::block_number();
	let block = hydradx_runtime::System::block_hash(block_number);
	let pool_contract = hydration.fetch_pool(&api, block).expect("fetch_pool() to work");
	assert_ok!(Liquidation::set_borrowing_contract(
		RuntimeOrigin::root(),
		pool_contract
	));
	assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

	supply(pool_contract, alice_evm_address, weth_asset_address, 10 * WETH_UNIT);
	supply(pool_contract, alice_evm_address, dot_asset_address, 5_000 * DOT_UNIT);
	borrow(pool_contract, alice_evm_address, dot_asset_address, 5_000 * DOT_UNIT);
	hydradx_run_to_next_block();

	// healthy before the price move
	let usr = get_user_account_data(pool_contract, alice_evm_address).unwrap();
	assert!(usr.health_factor > U256::from(1_000_000_000_000_000_000u128));

	// triple the DOT price (DOT is the debt) -> HF < 1
	let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
	let price = price.as_u128() * 6 / 2;
	let timestamp = timestamp.as_u128() + 6;
	let mut data = price.to_be_bytes().to_vec();
	data.extend_from_slice(timestamp.to_be_bytes().as_ref());
	update_oracle_price(
		vec![("DOT/USD", U256::from_big_endian(&data[0..32]))],
		ORACLE_ADDRESS,
		ORACLE_CALLER,
	);

	let usr = get_user_account_data(pool_contract, alice_evm_address).unwrap();
	assert!(usr.health_factor < U256::from(1_000_000_000_000_000_000u128));

	(pool_contract, alice_evm_address, caller)
}

/// Fetch the v2 money-market + borrower and run `decide_liquidation` for `borrower_evm`.
fn v2_decision(caller: EvmAddress, borrower_evm: EvmAddress) -> pepl_worker::LiquidationDecision {
	let api = ApiProvider::<Runtime>(Runtime);
	let hydration = Hydration::new(caller, PAP_CONTRACT, LOG_PREFIX);
	let block_number = hydradx_runtime::System::block_number();
	let block = hydradx_runtime::System::block_hash(block_number);
	let now = api.timestamp(block).expect("timestamp");

	let mut mm = hydration
		.fetch_money_market(&api, block)
		.expect("v2 fetch_money_market");
	let borrower = hydration
		.fetch_borrower(&api, block, block_number, &mm, borrower_evm, now)
		.expect("v2 fetch_borrower");

	let cfg = pepl_worker::LiquidationTaskConfig {
		target_hf: TARGET_HF,
		log_prefix: LOG_PREFIX.to_string(),
		..Default::default()
	};
	pepl_worker::decide_liquidation(&cfg, &mut mm, &borrower).expect("v2 should decide to liquidate")
}

/// Fetch the v1 money-market + user and run `get_best_liquidation_option`. Returns the on-chain
/// `(collateral_asset_id, debt_asset_id, debt_to_liquidate)` v1 would submit.
fn v1_decision(caller: EvmAddress, borrower_evm: EvmAddress) -> (AssetId, AssetId, Balance) {
	let block_number = hydradx_runtime::System::block_number();
	let block = hydradx_runtime::System::block_hash(block_number);
	let now = ApiProvider::<Runtime>(Runtime).timestamp(block).expect("timestamp");

	let mut mm = V1MoneyMarketData::<hydradx_runtime::Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
		V1ApiProvider<Runtime>,
	>(V1ApiProvider::<Runtime>(Runtime), block, PAP_CONTRACT, caller)
	.expect("v1 MoneyMarketData::new");

	let user = V1UserData::new(V1ApiProvider::<Runtime>(Runtime), block, &mm, borrower_evm, now, caller)
		.expect("v1 UserData::new");

	let opt = mm
		.get_best_liquidation_option::<V1ApiProvider<Runtime>>(&user, U256::from(TARGET_HF), None)
		.expect("v1 option calc")
		.expect("v1 should find a liquidation option");

	let coll = mm
		.address_to_asset(opt.collateral_asset)
		.expect("v1 collateral asset id");
	let debt = mm.address_to_asset(opt.debt_asset).expect("v1 debt asset id");
	let amount: Balance = opt
		.debt_to_liquidate
		.try_into()
		.expect("v1 debt_to_liquidate fits u128");
	(coll, debt, amount)
}

#[test]
fn v2_decide_liquidation_should_restore_hf_when_borrower_is_underwater() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		let (pool_contract, borrower_evm, caller) = create_unhealthy_borrower();

		let decision = v2_decision(caller, borrower_evm);

		// pinned exact decision: v2 chooses DOT collateral / DOT debt on this scenario
		assert_eq!(decision.collateral_asset, DOT);
		assert_eq!(decision.debt_asset, DOT);
		assert_eq!(decision.debt_to_cover, 17_190_053_835_700);
		assert_eq!(decision.priority, 200_363);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			decision.collateral_asset,
			decision.debt_asset,
			borrower_evm,
			decision.debt_to_cover,
			BoundedVec::new(),
			Some(decision.priority),
		));

		let usr = get_user_account_data(pool_contract, borrower_evm).unwrap();
		assert_health_factor_is_within_tolerance(usr.health_factor, U256::from(TARGET_HF));
	});
}

#[test]
fn v1_and_v2_should_choose_same_liquidation_when_borrower_is_underwater() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT_2).execute_with(|| {
		let (pool_contract, borrower_evm, caller) = create_unhealthy_borrower();

		// both decisions are read-only, computed on the same underwater state
		let v2 = v2_decision(caller, borrower_evm);
		let (v1_coll, v1_debt, v1_amount) = v1_decision(caller, borrower_evm);

		// exact parity: v1 and v2 choose the identical liquidation (same assets AND amount)
		assert_eq!(v1_coll, DOT);
		assert_eq!(v1_debt, DOT);
		assert_eq!(v1_amount, 17_190_053_835_700);
		assert_eq!(v1_coll, v2.collateral_asset, "collateral asset mismatch v1 vs v2");
		assert_eq!(v1_debt, v2.debt_asset, "debt asset mismatch v1 vs v2");
		assert_eq!(v1_amount, v2.debt_to_cover, "debt amount mismatch v1 vs v2");

		// executing v1's choice restores HF (proves v1 works through the same harness)
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			v1_coll,
			v1_debt,
			borrower_evm,
			v1_amount,
			BoundedVec::new(),
			None,
		));
		let usr = get_user_account_data(pool_contract, borrower_evm).unwrap();
		assert_health_factor_is_within_tolerance(usr.health_factor, U256::from(TARGET_HF));
	});
}
