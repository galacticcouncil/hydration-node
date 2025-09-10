#![cfg(test)]

use crate::polkadot_test_net::*;
use ethabi::ethereum_types::H160;
use ethabi::{encode, Token};
use fp_evm::{
	ExitReason::Succeed,
	ExitSucceed::{Returned, Stopped},
};
use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApi;
use frame_support::BoundedVec;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::erc20_mapping::runtime_decl_for_erc_20_mapping_api::Erc20MappingApi;
use hydradx_runtime::{
	evm::{
		precompiles::{erc20_mapping::HydraErc20Mapping, handle::EvmDataWriter},
		Executor,
	},
	AssetId, Balance, Block, BlockT, Currencies, EVMAccounts, Liquidation, OriginCaller, Router, Runtime, RuntimeCall,
	RuntimeEvent, RuntimeOrigin, Treasury,
};
use hydradx_traits::{
	evm::{CallContext, Erc20Encoding, EvmAddress, EVM},
	router::{AssetPair, RouteProvider},
};
use liquidation_worker_support::*;
use orml_traits::currency::MultiCurrency;
use sp_api::ApiError;
use sp_core::{H256, U256};
use sp_runtime::traits::CheckedConversion;
use xcm_runtime_apis::dry_run::{
	runtime_decl_for_dry_run_api::DryRunApi, CallDryRunEffects, Error as XcmDryRunApiError,
};

// ./target/release/scraper save-storage --pallet EVM AssetRegistry Timestamp Omnipool Tokens --uri wss://rpc.nice.hydration.cloud:443
pub const PATH_TO_SNAPSHOT: &str = "evm-snapshot/LIQUIDATION_SNAPSHOT";
const PAP_CONTRACT: hydradx_runtime::evm::EvmAddress = H160(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6"));

const DOT: AssetId = 5;
const DOT_UNIT: Balance = 10_000_000_000;
const WETH: AssetId = 20;
const WETH_UNIT: Balance = 1_000_000_000_000_000_000;
const ALICE_INITIAL_WETH_BALANCE: Balance = 20 * WETH_UNIT;
const ALICE_INITIAL_DOT_BALANCE: Balance = 10_000 * DOT_UNIT;

pub fn supply(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let context = CallContext::new_call(mm_pool, user);
	let data = EvmDataWriter::new_with_selector(Function::Supply)
		.write(asset)
		.write(amount)
		.write(user)
		.write(0u32)
		.build();

	let (res, value) = Executor::<Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
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

	let (res, value) = Executor::<Runtime>::call(context, data, U256::zero(), 50_000_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
}

#[allow(dead_code)]
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

	let (res, value) = Executor::<Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	let total_collateral_base = U256::checked_from(&value[0..32])?;
	let total_debt_base = U256::checked_from(&value[32..64])?;
	let available_borrows_base = U256::checked_from(&value[64..96])?;
	let current_liquidation_threshold = U256::checked_from(&value[96..128])?;
	let ltv = U256::checked_from(&value[128..160])?;
	let health_factor = U256::checked_from(&value[160..192])?;

	Some(UserAccountData {
		total_collateral_base,
		total_debt_base,
		available_borrows_base,
		current_liquidation_threshold,
		ltv,
		health_factor,
	})
}

pub fn update_oracle_price(oracle_data: Vec<(&str, U256)>) {
	// mainnet
	// let caller = EvmAddress::from_slice(&hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e"));
	// let oracle_address = EvmAddress::from_slice(&hex!("dee629af973ebf5bf261ace12ffd1900ac715f5e"));
	// testnet
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let oracle_address = EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));
	let context = CallContext::new_call(oracle_address, caller);

	let mut data = Into::<u32>::into(Function::SetMultipleValues).to_be_bytes().to_vec();

	let mut token_string_array = Vec::new();
	let mut token_uint_array = Vec::new();

	for data in oracle_data.iter() {
		token_string_array.push(Token::String(data.0.to_string()));
		token_uint_array.push(Token::Uint(data.1));
	}

	let encoded_values = encode(&[Token::Array(token_string_array), Token::Array(token_uint_array)]);

	data.extend_from_slice(&encoded_values);

	let (res, value) = Executor::<Runtime>::call(context, data, U256::zero(), 5_000_000);
	assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

pub fn get_oracle_price(asset_pair: &str) -> Option<(U256, U256)> {
	// contains addresses from mainnet and testnet to support different snapshots
	let oracle_addresses = [
		EvmAddress::from_slice(&hex!("dee629af973ebf5bf261ace12ffd1900ac715f5e")),
		EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917")),
		EvmAddress::from_slice(&hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52")),
	];

	for oracle_address in oracle_addresses.iter() {
		let context = CallContext::new_view(*oracle_address);
		let mut data = Into::<u32>::into(Function::GetValue).to_be_bytes().to_vec();
		let encoded_value = encode(&[Token::String(asset_pair.to_string())]);
		data.extend_from_slice(&encoded_value);

		let (res, value) = Executor::<Runtime>::call(context, data, U256::zero(), 5_000_000);
		if res == Succeed(Returned) {
			let price = U256::checked_from(&value[0..32]).unwrap();
			let timestamp = U256::checked_from(&value[32..64]).unwrap();

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

#[test]
fn liquidation_should_work() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		let treasury_dot_initial_balance = Currencies::free_balance(DOT, &Treasury::account_id());

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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

		assert_eq!(
			Currencies::free_balance(DOT, &ALICE.into()),
			ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount
		);
		assert_eq!(
			Currencies::free_balance(WETH, &ALICE.into()),
			ALICE_INITIAL_WETH_BALANCE - collateral_weth_amount
		);

		let borrow_dot_amount: Balance = 5_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);
		assert_eq!(
			Currencies::free_balance(DOT, &ALICE.into()),
			ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount + borrow_dot_amount
		);

		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);

		let (price, timestamp) = get_oracle_price("WETH/USD").unwrap();
		let price = price.as_u128() / 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("WETH/USD", U256::checked_from(&data[0..32]).unwrap())]);

		// ensure that the health_factor < 1
		let user_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(user_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		let route = Router::get_route(AssetPair {
			asset_in: WETH,
			asset_out: DOT,
		});

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			route
		));

		// Assert
		assert_eq!(Currencies::free_balance(DOT, &pallet_acc), 0);
		assert_eq!(Currencies::free_balance(WETH, &pallet_acc), 0);

		assert!(Currencies::free_balance(DOT, &Treasury::account_id()) > treasury_dot_initial_balance);

		assert_eq!(Currencies::free_balance(DOT, &BOB.into()), 0);
		assert_eq!(Currencies::free_balance(WETH, &BOB.into()), 0);
	});
}

#[test]
fn liquidation_should_revert_correctly_when_evm_call_fails() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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

		assert_eq!(
			Currencies::free_balance(DOT, &ALICE.into()),
			ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount
		);
		assert_eq!(
			Currencies::free_balance(WETH, &ALICE.into()),
			ALICE_INITIAL_WETH_BALANCE - collateral_weth_amount
		);

		let borrow_dot_amount: Balance = 5_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);
		assert_eq!(
			Currencies::free_balance(DOT, &ALICE.into()),
			ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount + borrow_dot_amount
		);

		// ensure that the health_factor > 1
		let user_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(user_data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		let route = Router::get_route(AssetPair {
			asset_in: WETH,
			asset_out: DOT,
		});

		// Act
		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::signed(BOB.into()),
				WETH,
				DOT,
				alice_evm_address,
				borrow_dot_amount,
				route
			),
			pallet_liquidation::Error::<Runtime>::LiquidationCallFailed
		);

		// Assert
		assert_eq!(Currencies::free_balance(DOT, &pallet_acc), 0);
		assert_eq!(Currencies::free_balance(WETH, &pallet_acc), 0);

		assert_eq!(Currencies::free_balance(DOT, &BOB.into()), 0);
		assert_eq!(Currencies::free_balance(WETH, &BOB.into()), 0);
	});
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

#[test]
fn calculate_debt_to_liquidate_with_same_collateral_and_debt_asset() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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
		let mut money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
		let current_evm_timestamp = ApiProvider::<Runtime>(Runtime).current_timestamp(hash).unwrap();

		// HF > 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		// update MM and UserData structs based on future price
		let dot_address = money_market_data.get_asset_address("DOT").unwrap();
		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 6 / 2;
		money_market_data.update_reserve_price(dot_address, new_price.into());

		let mut user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);

		let debt_asset = money_market_data.get_asset_address("DOT").unwrap();
		let collateral_asset = money_market_data.get_asset_address("DOT").unwrap();
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency,
			collateral_in_base_currency,
		} = money_market_data
			.calculate_debt_to_liquidate::<ApiProvider<Runtime>>(
				&user_data,
				target_health_factor,
				collateral_asset,
				debt_asset,
			)
			.unwrap();

		let mut user_reserve = user_data.reserves()[4].clone();
		user_reserve.collateral = user_reserve.collateral.saturating_sub(collateral_in_base_currency);
		user_reserve.debt = user_reserve.debt.saturating_sub(debt_in_base_currency);
		user_data.update_reserves(vec![(4, user_reserve)]);
		let target_hf_diff = target_health_factor.abs_diff(
			user_data
				.health_factor::<Block, ApiProvider<Runtime>, OriginCaller, RuntimeCall, RuntimeEvent>(
					&money_market_data,
				)
				.unwrap(),
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
		update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);

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
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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

		let mut money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();

		let current_evm_timestamp = ApiProvider::<Runtime>(Runtime).current_timestamp(hash).unwrap()
			+ primitives::constants::time::SECS_PER_BLOCK; // our calculations "happen" in the next block

		let dot_address = money_market_data.get_asset_address("DOT").unwrap();
		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 5 / 2;
		money_market_data.update_reserve_price(dot_address, new_price.into());

		let user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);

		let debt_asset = money_market_data.get_asset_address("DOT").unwrap();
		let collateral_asset = money_market_data.get_asset_address("WETH").unwrap();
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = money_market_data
			.calculate_debt_to_liquidate::<ApiProvider<Runtime>>(
				&user_data,
				target_health_factor,
				collateral_asset,
				debt_asset,
			)
			.unwrap();

		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 5 / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);

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
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, target_health_factor);
	});
}

#[test]
fn calculate_debt_to_liquidate_collateral_amount_is_not_sufficient_to_reach_target_health_factor() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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

		let mut money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();

		let current_evm_timestamp = ApiProvider::<Runtime>(Runtime).current_timestamp(hash).unwrap()
			+ primitives::constants::time::SECS_PER_BLOCK; // our calculations "happen" in the next block

		let weth_address = money_market_data.get_asset_address("WETH").unwrap();
		let new_price = get_oracle_price("WETH/USD").unwrap().0.as_u128() / 3;
		let dot_address = money_market_data.get_asset_address("DOT").unwrap();
		money_market_data.update_reserve_price(weth_address, new_price.into());

		let user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = money_market_data
			.calculate_debt_to_liquidate::<ApiProvider<Runtime>>(
				&user_data,
				target_health_factor,
				weth_address,
				dot_address,
			)
			.unwrap();

		// update WETH price
		let (price, timestamp) = get_oracle_price("WETH/USD").unwrap();
		let price = price.as_u128() / 3;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("WETH/USD", U256::checked_from(&data[0..32]).unwrap())]);

		// ensure that the health_factor < 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		let weth_reserve = money_market_data
			.reserves()
			.iter()
			.find(|x| x.asset_address() == weth_address)
			.unwrap();
		let collateral_reserve = weth_reserve
			.get_user_collateral_in_base_currency::<Block, ApiProvider<Runtime>, OriginCaller, RuntimeCall, RuntimeEvent>(
				&ApiProvider::<Runtime>(Runtime),
				hash,
				user_data.address(),
				current_evm_timestamp,
				alice_evm_address,
			)
			.unwrap();

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH, // collateral
			DOT,  // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
		));

		let money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();

		let user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		// Assert
		let remaining_collateral_reserve = weth_reserve
			.get_user_collateral_in_base_currency::<Block, ApiProvider<Runtime>, OriginCaller, RuntimeCall, RuntimeEvent>(
				&ApiProvider::<Runtime>(Runtime),
				hash,
				user_data.address(),
				current_evm_timestamp,
				alice_evm_address,
			)
			.unwrap();

		assert!(remaining_collateral_reserve < collateral_reserve / 1_000);
	});
}

#[test]
fn calculate_debt_to_liquidate_with_weth_as_debt() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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

		let mut money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();

		let current_evm_timestamp = ApiProvider::<Runtime>(Runtime).current_timestamp(hash).unwrap()
			+ primitives::constants::time::SECS_PER_BLOCK; // our calculations "happen" in the next block

		let weth_address = money_market_data.get_asset_address("WETH").unwrap();
		let new_price = get_oracle_price("WETH/USD").unwrap().0.as_u128() * 5 / 2;
		money_market_data.update_reserve_price(weth_address, new_price.into());

		let user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);

		let debt_asset = money_market_data.get_asset_address("WETH").unwrap();
		let collateral_asset = money_market_data.get_asset_address("WETH").unwrap();
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = money_market_data
			.calculate_debt_to_liquidate::<ApiProvider<Runtime>>(
				&user_data,
				target_health_factor,
				collateral_asset,
				debt_asset,
			)
			.unwrap();

		let (price, timestamp) = get_oracle_price("WETH/USD").unwrap();
		let price = price.as_u128() * 5 / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("WETH/USD", U256::checked_from(&data[0..32]).unwrap())]);

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
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, target_health_factor);
	});
}

#[test]
fn calculate_debt_to_liquidate_with_two_different_assets() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// get Pool contract address
		let block_number = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(block_number);
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
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

		let mut money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();

		let current_evm_timestamp = ApiProvider::<Runtime>(Runtime).current_timestamp(hash).unwrap();

		// ensure that the health_factor > 1
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		let dot_address = money_market_data.get_asset_address("DOT").unwrap();
		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 7 / 5;
		money_market_data.update_reserve_price(dot_address, new_price.into());

		let user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);

		let debt_asset = money_market_data.get_asset_address("DOT").unwrap();
		let collateral_asset = money_market_data.get_asset_address("WETH").unwrap();
		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency: _,
			collateral_in_base_currency: _,
		} = money_market_data
			.calculate_debt_to_liquidate::<ApiProvider<Runtime>>(
				&user_data,
				target_health_factor,
				collateral_asset,
				debt_asset,
			)
			.unwrap();

		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 7 / 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);

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
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, target_health_factor);
	});
}

#[derive(sp_core::RuntimeDebug)]
pub struct ApiProvider<C>(pub C);
impl<Block, C> RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent> for ApiProvider<C>
where
	Block: BlockT,
	C: EthereumRuntimeRPCApi<Block>
		+ Erc20MappingApi<Block>
		+ DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller>,
{
	fn current_timestamp(&self, _hash: Block::Hash) -> Option<u64> {
		let block = C::current_block()?;
		// milliseconds to seconds
		block.header.timestamp.checked_div(1_000)
	}
	fn call(
		&self,
		_hash: Block::Hash,
		caller: EvmAddress,
		contract_address: EvmAddress,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<Result<fp_evm::ExecutionInfoV2<Vec<u8>>, frame_support::sp_runtime::DispatchError>, sp_api::ApiError> {
		Ok(C::call(
			caller,
			contract_address,
			data,
			U256::zero(),
			gas_limit,
			None,
			None,
			None,
			true,
			None,
		)
		.map_err(|_| sp_runtime::DispatchError::Other("Calling EthereumRuntimeRPCApi::Call failed.")))
	}
	fn address_to_asset(
		&self,
		_hash: Block::Hash,
		_address: EvmAddress,
	) -> Result<Option<liquidation_worker_support::AssetId>, ApiError> {
		unimplemented!()
	}
	fn dry_run_call(
		&self,
		_hash: Block::Hash,
		_origin: OriginCaller,
		_call: RuntimeCall,
	) -> Result<Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError>, sp_api::ApiError> {
		unimplemented!()
	}
}

#[test]
fn calculate_debt_to_liquidate_with_three_different_assets() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);
		let vdot_asset_address = HydraErc20Mapping::encode_evm_address(15);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));
		assert_ok!(Currencies::deposit(DOT, &BOB.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &BOB.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		let bob_evm_address = EVMAccounts::evm_address(&AccountId::from(BOB));

		let b = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(b);

		// get Pool contract address
		let pool_contract = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<
			ApiProvider<Runtime>,
		>(&ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// BOB borrows vDOT and sends it to ALICE
		supply(pool_contract, bob_evm_address, dot_asset_address, 2_000 * DOT_UNIT);

		borrow(pool_contract, bob_evm_address, vdot_asset_address, 1_000 * DOT_UNIT);

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			ALICE.into(),
			15,
			1_000 * DOT_UNIT,
		));

		let collateral_vdot_amount: Balance = 1_000 * DOT_UNIT;
		supply(
			pool_contract,
			alice_evm_address,
			vdot_asset_address,
			collateral_vdot_amount,
		);

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

		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert!(usr_data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		let b = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(b);

		let mut money_market_data = MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new::<
			ApiProvider<Runtime>,
		>(ApiProvider::<Runtime>(Runtime), hash, PAP_CONTRACT, alice_evm_address)
		.unwrap();

		let current_evm_timestamp = ApiProvider::<Runtime>(Runtime).current_timestamp(hash).unwrap();

		let dot_address = money_market_data.get_asset_address("DOT").unwrap();
		let new_price = get_oracle_price("DOT/USD").unwrap().0.as_u128() * 12 / 7;
		money_market_data.update_reserve_price(dot_address, new_price.into());

		let mut user_data = UserData::new(
			ApiProvider::<Runtime>(Runtime),
			hash,
			&money_market_data,
			alice_evm_address,
			current_evm_timestamp,
			alice_evm_address,
		)
		.unwrap();

		let target_health_factor = U256::from(1_001_000_000_000_000_000u128);

		let debt_asset = money_market_data.get_asset_address("DOT").unwrap();
		let collateral_asset = money_market_data.get_asset_address("WETH").unwrap();

		let LiquidationAmounts {
			debt_amount,
			collateral_amount: _,
			debt_in_base_currency,
			collateral_in_base_currency,
		} = money_market_data
			.calculate_debt_to_liquidate::<ApiProvider<Runtime>>(
				&user_data,
				target_health_factor,
				collateral_asset,
				debt_asset,
			)
			.unwrap();

		let mut c_user_reserve = user_data.reserves()[2].clone();
		let mut d_user_reserve = user_data.reserves()[4].clone();
		c_user_reserve.collateral = c_user_reserve.collateral.saturating_sub(collateral_in_base_currency);
		d_user_reserve.debt = d_user_reserve.debt.saturating_sub(debt_in_base_currency);
		user_data.update_reserves(vec![(2, c_user_reserve)]);
		user_data.update_reserves(vec![(4, d_user_reserve)]);

		let (price, timestamp) = get_oracle_price("DOT/USD").unwrap();
		let price = price.as_u128() * 12 / 7;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH, // collateral
			DOT,  // debt
			alice_evm_address,
			debt_amount.try_into().unwrap(),
			BoundedVec::new(),
		));

		// Assert
		let usr_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
		assert_health_factor_is_within_tolerance(usr_data.health_factor, target_health_factor);
	});
}
