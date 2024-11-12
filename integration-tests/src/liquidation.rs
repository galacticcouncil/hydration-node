#![cfg(test)]

use crate::polkadot_test_net::*;
use ethabi::ethereum_types::BigEndianHash;
use fp_evm::{
	ExitReason::Succeed,
	ExitSucceed::{Returned, Stopped},
};
use frame_support::{assert_ok, sp_runtime::RuntimeDebug};
use hex_literal::hex;
use hydradx_runtime::{
	evm::{
		precompiles::{erc20_mapping::HydraErc20Mapping, handle::EvmDataWriter},
		Executor,
	},
	AssetId, Balance, Currencies, EVMAccounts, Liquidation, Router, RuntimeOrigin, Treasury,
};
use hydradx_traits::{
	evm::{CallContext, Erc20Mapping, EvmAddress, EVM},
	router::{AssetPair, RouteProvider},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::currency::MultiCurrency;
use sp_core::{H256, U256};
use sp_runtime::{traits::CheckedConversion, SaturatedConversion};

const PATH_TO_SNAPSHOT: &str = "evm-snapshot/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	GetPool = "getPool()",
	Supply = "supply(address,uint256,address,uint16)",
	Withdraw = "withdraw(address,uint256,address)",
	Borrow = "borrow(address,uint256,uint256,uint16,address)",
	GetUserAccountData = "getUserAccountData(address)",
	GetPriceOracle = "getPriceOracle()",
	SetMultipleValues = "setMultipleValues(string[],uint256[])",
	GetValue = "getValue(string)",
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
}

const DOT: AssetId = 5;
const DOT_UNIT: Balance = 10_000_000_000;
const WETH: AssetId = 20;
const WETH_UNIT: Balance = 1_000_000_000_000_000_000;
const ALICE_INITIAL_WETH_BALANCE: Balance = 20 * WETH_UNIT;
const ALICE_INITIAL_DOT_BALANCE: Balance = 10_000 * DOT_UNIT;

pub fn get_pool(pap_contract: EvmAddress) -> EvmAddress {
	let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();
	let context = CallContext::new_view(pap_contract);

	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	EvmAddress::from(H256::from_slice(&value))
}

pub fn supply(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let context = CallContext::new_call(mm_pool, user);
	// let mut data = Into::<u32>::into(Function::Supply).to_be_bytes().to_vec();
	// data.extend_from_slice(H256::from(asset).as_bytes());
	// data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());
	// data.extend_from_slice(H256::from(user).as_bytes());
	// data.extend_from_slice(H256::zero().as_bytes());
	let data = EvmDataWriter::new_with_selector(Function::Supply)
		.write(asset)
		.write(amount)
		.write(user)
		.write(0u32)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
}

pub fn borrow(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let context = CallContext::new_call(mm_pool, user);
	// let mut data = Into::<u32>::into(Function::Borrow).to_be_bytes().to_vec();
	// data.extend_from_slice(H256::from(asset).as_bytes());
	// data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());
	// data.extend_from_slice(H256::from_uint(&U256::from(2u32)).as_bytes());
	// data.extend_from_slice(H256::zero().as_bytes());
	// data.extend_from_slice(H256::from(user).as_bytes());
	let data = EvmDataWriter::new_with_selector(Function::Borrow)
		.write(asset)
		.write(amount)
		.write(2u32)
		.write(0u32)
		.write(user)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 50_000_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
}

pub fn get_user_account_data(mm_pool: EvmAddress, user: EvmAddress) -> (U256, U256, U256, U256, U256, U256) {
	let context = CallContext::new_call(mm_pool, user);
	let mut data = Into::<u32>::into(Function::GetUserAccountData).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(user).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	let total_collateral_base = U256::checked_from(&value[0..32]).unwrap();
	let total_debt_base = U256::checked_from(&value[32..64]).unwrap();
	let available_borrows_base = U256::checked_from(&value[64..96]).unwrap();
	let current_liquidation_threshold = U256::checked_from(&value[96..128]).unwrap();
	let ltv = U256::checked_from(&value[128..160]).unwrap();
	let health_factor = U256::checked_from(&value[160..192]).unwrap();

	(
		total_collateral_base,
		total_debt_base,
		available_borrows_base,
		current_liquidation_threshold,
		ltv,
		health_factor,
	)
}

pub fn update_oracle_price(asset_pair: &str, price: U256) {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let oracle_address = EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));
	let context = CallContext::new_call(oracle_address, caller);

	let mut data = Into::<u32>::into(Function::SetMultipleValues).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from_uint(&U256::from(64u32)).as_bytes());
	data.extend_from_slice(H256::from_uint(&U256::from(192u32)).as_bytes());
	data.extend_from_slice(H256::from_uint(&U256::from(1u32)).as_bytes());
	data.extend_from_slice(H256::from_uint(&U256::from(32u32)).as_bytes());
	data.extend_from_slice(H256::from_uint(&U256::from(asset_pair.len().saturated_into::<u128>())).as_bytes());
	let mut arr = [0; 32];
	arr[0..asset_pair.len()].copy_from_slice(asset_pair.as_bytes());
	data.extend_from_slice(arr.as_slice());
	data.extend_from_slice(H256::from_uint(&U256::from(1u32)).as_bytes());
	data.extend_from_slice(H256::from_uint(&price).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

pub fn get_oracle_price(asset_pair: &str) -> (U256, U256) {
	let oracle_address = EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));
	let context = CallContext::new_view(oracle_address);
	let mut data = Into::<u32>::into(Function::GetValue).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from_uint(&U256::from(32u32)).as_bytes());
	data.extend_from_slice(H256::from_uint(&U256::from(asset_pair.len().saturated_into::<u128>())).as_bytes());
	let mut arr = [0; 32];
	arr[0..asset_pair.len()].copy_from_slice(asset_pair.as_bytes());
	data.extend_from_slice(arr.as_slice());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	let price = U256::checked_from(&value[0..32]).unwrap();
	let timestamp = U256::checked_from(&value[32..64]).unwrap();

	(price, timestamp)
}

#[test]
fn liquidation_should_work() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// PoolAddressesProvider contract
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		// get Pool contract address
		let pool_contract = get_pool(pap_contract);
		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT).unwrap();
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH).unwrap();

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		let treasury_dot_initial_balance = Currencies::free_balance(DOT, &Treasury::account_id());

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pap_contract));

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

		let (price, timestamp) = get_oracle_price("DOT/USD");
		let price = price.as_u128() * 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price("DOT/USD", U256::checked_from(&data[0..32]).unwrap());

		let (price, timestamp) = get_oracle_price("WETH/USD");
		let price = price.as_u128() / 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price("WETH/USD", U256::checked_from(&data[0..32]).unwrap());

		// ensure that the health_factor < 1
		let user_data = get_user_account_data(pool_contract, alice_evm_address);
		assert!(user_data.5 < U256::from(1_000_000_000_000_000_000u128));

		let route = Router::get_route(AssetPair {
			asset_in: WETH,
			asset_out: DOT,
		});

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			route
		));

		assert_eq!(Currencies::free_balance(DOT, &pallet_acc), 0);
		assert_eq!(Currencies::free_balance(WETH, &pallet_acc), 0);

		assert!(Currencies::free_balance(DOT, &Treasury::account_id()) > treasury_dot_initial_balance);

		assert_eq!(Currencies::free_balance(DOT, &BOB.into()), 0);
		assert_eq!(Currencies::free_balance(WETH, &BOB.into()), 0);
	});
}
