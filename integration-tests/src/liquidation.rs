#![cfg(test)]

use std::cmp::{max, Ordering};
use std::ops::BitAnd;
use crate::polkadot_test_net::*;
use ethabi::{decode, encode, ethereum_types::BigEndianHash, ParamType, Token};
use fp_evm::{
	ExitReason::Succeed,
	ExitSucceed::{Returned, Stopped},
};
use fp_self_contained::SelfContainedCall;
use fp_rpc::ConvertTransaction;
use polkadot_primitives::EncodeAs;
use frame_support::{assert_noop, assert_ok, sp_runtime::RuntimeDebug};
use frame_support::pallet_prelude::{Decode, Encode};
use hex_literal::hex;
use hydradx_runtime::{evm::{
	precompiles::{erc20_mapping::HydraErc20Mapping, handle::EvmDataWriter},
	Executor,
}, AssetId, Balance, Currencies, EVMAccounts, Liquidation, Router, Runtime, RuntimeOrigin, System, Treasury};
use hydradx_traits::{
	evm::{CallContext, Erc20Mapping, EvmAddress, EVM},
	router::{AssetPair, RouteProvider},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::currency::MultiCurrency;
use sp_core::{H160, H256, U256, U512};
use sp_runtime::{traits::CheckedConversion, SaturatedConversion};
use pallet_liquidation::{BorrowerData, BorrowerDataDetails, MAX_LIQUIDATIONS};

// ./target/release/scraper save-storage --pallet EVM AssetRegistry Timestamp Omnipool Tokens --uri wss://rpc.nice.hydration.cloud:443
const PATH_TO_SNAPSHOT: &str = "evm-snapshot/SNAPSHOT_NEW";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	GetPool = "getPool()",
	GetPriceOracle = "getPriceOracle()",
	GetAssetPrice= "getAssetPrice(address)",
	Supply = "supply(address,uint256,address,uint16)",
	Withdraw = "withdraw(address,uint256,address)",
	Borrow = "borrow(address,uint256,uint256,uint16,address)",
	GetUserAccountData = "getUserAccountData(address)",
	GetReservesList = "getReservesList()",
	GetReserveData = "getReserveData(address)",
	GetConfiguration = "getConfiguration(address)",
	GetUserConfiguration = "getUserConfiguration(address)",
	ScaledBalanceOf = "scaledBalanceOf(address)",
	BalanceOf = "balanceOf(address)",
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

pub fn get_price_oracle(pap_contract: EvmAddress) -> EvmAddress {
	let data = Into::<u32>::into(Function::GetPriceOracle).to_be_bytes().to_vec();
	let context = CallContext::new_view(pap_contract);

	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	EvmAddress::from(H256::from_slice(&value))
}

pub fn get_asset_price(oracle_address: EvmAddress, asset: EvmAddress, user: EvmAddress) -> U256 {
	let context = CallContext::new_call(oracle_address, user);
	let mut data = Into::<u32>::into(Function::GetAssetPrice).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(asset).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	U256::checked_from(&value[0..32]).unwrap()
}

pub fn supply(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let context = CallContext::new_call(mm_pool, user);
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

	// uint256 totalCollateralBase,
	// uint256 totalDebtBase,
	// uint256 availableBorrowsBase,
	// uint256 currentLiquidationThreshold,
	// uint256 ltv,
	// uint256 healthFactor
	(
		total_collateral_base,
		total_debt_base,
		available_borrows_base,
		current_liquidation_threshold,
		ltv,
		health_factor,
	)
}

pub fn get_reserves_list(mm_pool: EvmAddress) -> Vec<H160> {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(mm_pool, caller);
	let mut data = Into::<u32>::into(Function::GetReservesList).to_be_bytes().to_vec();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	let decoded = ethabi::decode(
		&[
			ethabi::ParamType::Array(Box::new(ethabi::ParamType::Address)),
		],
		&value,
	).ok().unwrap();

	let decoded = decoded[0].clone().into_array().unwrap();
	let mut address_arr = Vec::new();
	for i in decoded.iter() {
		address_arr.push(i.clone().into_address().unwrap());
	}

	address_arr
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct UserData {
	address: H160,
	configuration: U256,
}
impl UserData {
	pub fn new(money_market: &MoneyMarketData, address: H160) -> Self {
		let configuration = get_user_configuration(money_market.pool_contract, address);
		Self { address, configuration }
	}

	pub fn is_collateral(&self, asset_index: usize) -> bool {
		let mut bit_mask = U256::from(2 << 2 * asset_index);
		!(self.configuration & bit_mask).is_zero()
	}

	pub fn is_debt(&self, asset_index: usize) -> bool {
		let mut bit_mask = U256::from(1 << 2 * asset_index);
		!(self.configuration & bit_mask).is_zero()
	}
	pub fn health_factor(&self, money_market: &MoneyMarketData, current_timestamp: u64, caller: EvmAddress) -> Option<U256> {
		let mut avg_liquidation_threshold = U256::zero();
		let mut total_collateral = U256::zero();
		let mut total_debt = U256::zero();

		for reserve in money_market.reserves.iter() {
			let price = get_asset_price(money_market.oracle_contract, reserve.asset_address, caller);
			let partial_collateral = reserve.get_user_collateral_in_base_currency(self.address, current_timestamp, price)?;
			avg_liquidation_threshold = avg_liquidation_threshold
				.checked_add(partial_collateral.checked_mul(U256::from(reserve.reserve_data.liquidation_threshold()))?)?;
			total_collateral = total_collateral.checked_add(partial_collateral)?;
			total_debt = total_debt.checked_add(reserve.get_user_debt_in_base_currency(self.address, current_timestamp, price)?)?;
		}

		avg_liquidation_threshold = avg_liquidation_threshold.checked_div(total_collateral)?;

		let nominator = percent_mul(total_collateral, avg_liquidation_threshold)?;
		wad_div(nominator, total_debt)
	}
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct MoneyMarketData {
	pap_contract: EvmAddress, // PoolAddressesProvider
	pool_contract: EvmAddress,
	oracle_contract: EvmAddress,
	reserves: Vec<Reserve>,
}
impl MoneyMarketData {
	pub fn new(pap_contract: EvmAddress) -> Self {
		let pool_contract = get_pool(pap_contract);
		let oracle_contract = get_price_oracle(pap_contract);

		let mut reserves = Vec::new();
		for asset_address in get_reserves_list(pool_contract).into_iter() {
			let reserve_data = get_reserve_data(pool_contract, asset_address);
			let symbol = get_asset_symbol(&asset_address);
			reserves.push(Reserve { reserve_data, asset_address, symbol });
		};

		Self {
			pap_contract,
			pool_contract,
			oracle_contract,
			reserves,
		}
	}
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Reserve {
	reserve_data: ReserveData,
	asset_address: EvmAddress,
	symbol: Vec<u8>,
}
impl Reserve {
	pub fn get_collateral_and_debt_addresses(&self) -> (H160, (H160, H160)) {
		(self.reserve_data.a_token_address.clone(), (self.reserve_data.stable_debt_token_address.clone(), self.reserve_data.variable_debt_token_address.clone()))
	}

	fn get_normalized_income(&self, current_timestamp: u64) -> Option<U256> {
		if current_timestamp == self.reserve_data.last_update_timestamp {
			Some(U256::from(self.reserve_data.liquidity_index))
		} else {
			let current_liquidity_rate = U256::from(self.reserve_data.current_liquidity_rate);
			let timestamp_diff = U256::from(current_timestamp.checked_sub(self.reserve_data.last_update_timestamp)?);
			let nominator = current_liquidity_rate.checked_mul(timestamp_diff)?;
			let seconds_per_year = U256::from(365 * 24 * 60 * 60);
			let ray = U256::from(10u128.pow(27));
			let linear_interest = nominator.checked_div(seconds_per_year)?.checked_add(ray)?;
			ray_mul(linear_interest, self.reserve_data.liquidity_index.into())
		}
	}
	pub fn get_decimals(&self) -> u8 {
		let config = self.reserve_data.configuration;
		let res = config >> (48);// & U256::from(0b1111_1111); // bits 48-55
		res.byte(0)
	}
	fn get_user_collateral_in_base_currency(&self, user: EvmAddress, current_timestamp: u64, price: U256) -> Option<U256> {
		let (collateral_address, _) = self.get_collateral_and_debt_addresses();
		let scaled_balance = scaled_balance_of(collateral_address, user);
		let normalized_income = self.get_normalized_income(current_timestamp)?;

		ray_mul(scaled_balance, normalized_income)?
			.checked_mul(price)?.
			checked_div(U256::from(10u128.pow(self.get_decimals() as u32)))
	}

	fn get_normalized_debt(&self, current_timestamp: u64) -> Option<U256> {
		let variable_borrow_index = U256::from(self.reserve_data.variable_borrow_index);
		if current_timestamp == self.reserve_data.last_update_timestamp {
			Some(variable_borrow_index)
		} else {
			let exp = U256::from(current_timestamp.checked_sub(self.reserve_data.last_update_timestamp)?);
			let ray = U256::from(10u128.pow(27));
			if exp.is_zero() {
				return Some(ray);
			}

			let exp_minus_one = exp.checked_sub(U256::from(1))?;
			let exp_minus_two = if exp > U256::from(2) {
				exp.checked_sub(U256::from(2))?
			} else {
				U256::zero()
			};

			let seconds_per_year = U256::from(365 * 24 * 60 * 60);
			let rate = U256::from(self.reserve_data.current_variable_borrow_rate);
			let base_power_two = ray_mul(rate, rate)?
				.checked_div(seconds_per_year * seconds_per_year)?;
			let base_power_three = ray_mul(base_power_two, rate)?;

			let second_term = exp.checked_mul(exp_minus_one)?.checked_mul(base_power_two)? / 2;
			let third_term = exp.checked_mul(exp_minus_one)?.checked_mul(exp_minus_two)?.checked_mul(base_power_three)? / 6;

			let compound_interest = rate.checked_mul(exp)?.checked_add(ray)?.checked_add(seconds_per_year)?.checked_add(third_term)?;

			ray_mul(compound_interest, variable_borrow_index)
		}
	}
	fn get_user_debt_in_base_currency(&self, user: EvmAddress, current_timestamp: u64, price: U256) -> Option<U256> {
		let (_, (stable_debt_address, variable_debt_address)) = self.get_collateral_and_debt_addresses();
		let mut total_debt = scaled_balance_of(variable_debt_address, user);
		if !total_debt.is_zero() {
			let normalized_debt = self.get_normalized_debt(current_timestamp)?;
			total_debt = ray_mul(total_debt, normalized_debt)?;
		}

		total_debt = total_debt + balance_of(stable_debt_address, user);

		total_debt.checked_mul(price)?
			.checked_div(U256::from(10u128.pow(self.get_decimals() as u32)))
	}
}

pub fn ray_mul(a: U256, b: U256) -> Option<U256> {
	if a.is_zero() || b.is_zero() {
		return Some(U256::zero());
	}

	let ray = U512::from(10u128.pow(27));

	let res512 = a.full_mul(b).checked_add(ray / 2)?.checked_div(ray)?;
	res512.try_into().ok()
}

pub fn wad_div(a: U256, b: U256) -> Option<U256> {
	if a.is_zero() {
		return Some(U256::zero());
	}
	if b.is_zero() {
		return None;
	}

	let wad = U256::from(10u128.pow(18));
	let nominator = a.full_mul(wad).checked_add(U512::from(b / 2))?;
	let res = nominator.checked_div(U512::from(b))?;
	res.try_into().ok()
}

pub fn percent_mul(value: U256, percentage: U256) -> Option<U256> {
	if percentage.is_zero() {
		return Some(U256::zero());
	}

	let percentage_factor = U512::from(10u128.pow(4));
	let half_percentage_factor = U512::from(percentage_factor / 2);
	let nominator = value.full_mul(percentage).checked_add(half_percentage_factor)?;
	let res = nominator.checked_div(percentage_factor)?;
	res.try_into().ok()
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct ReserveData {
	configuration: U256,
	liquidity_index: u128,
	current_liquidity_rate: u128,
	variable_borrow_index: u128,
	current_variable_borrow_rate: u128,
	current_stable_borrow_rate: u128,
	last_update_timestamp: u64,
	id: u16,
	a_token_address: H160,
	stable_debt_token_address: H160,
	variable_debt_token_address: H160,
	interest_rate_strategy_address: H160,
	accrued_to_treasury: u128,
	unbacked: u128,
	isolation_mode_total_debt: u128,
}

impl ReserveData {
	pub fn new(data: &Vec<ethabi::Token>) -> Option<Self> {
		Some(Self {
			configuration: data[0].clone().into_uint()?,
			liquidity_index: data[1].clone().into_uint()?.try_into().ok()?,
			current_liquidity_rate: data[2].clone().into_uint()?.try_into().ok()?,
			variable_borrow_index: data[3].clone().into_uint()?.try_into().ok()?,
			current_variable_borrow_rate: data[4].clone().into_uint()?.try_into().ok()?,
			current_stable_borrow_rate: data[5].clone().into_uint()?.try_into().ok()?,
			last_update_timestamp: data[6].clone().into_uint()?.try_into().ok()?,
			id: data[7].clone().into_uint()?.try_into().ok()?,
			a_token_address: data[8].clone().into_address()?,
			stable_debt_token_address: data[9].clone().into_address()?,
			variable_debt_token_address: data[10].clone().into_address()?,
			interest_rate_strategy_address: data[11].clone().into_address()?,
			accrued_to_treasury: data[12].clone().into_uint()?.try_into().ok()?,
			unbacked: data[13].clone().into_uint()?.try_into().ok()?,
			isolation_mode_total_debt: data[14].clone().into_uint()?.try_into().ok()?,
		})
	}

	pub fn liquidation_threshold(&self) -> u128 {
		let first_byte = self.configuration.low_u32() >> 16;
		let bit_mask: u32 = 0b0000_0000_0000_0000_1111_1111_1111_1111;
		first_byte.bitand(bit_mask) as u128
	}
}

pub fn get_reserve_data(mm_pool: EvmAddress, asset_address: H160) -> ReserveData {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(mm_pool, caller);
	let mut data = Into::<u32>::into(Function::GetReserveData).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(asset_address).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	let decoded = ethabi::decode(
		&[
			ethabi::ParamType::Uint(32), 	// ReserveConfigurationMap
			ethabi::ParamType::Uint(16), 	// liquidityIndex
			ethabi::ParamType::Uint(16), 	// currentLiquidityRate
			ethabi::ParamType::Uint(16), 	// variableBorrowIndex
			ethabi::ParamType::Uint(16),	// currentVariableBorrowRate
			ethabi::ParamType::Uint(16),	// currentStableBorrowRate
			ethabi::ParamType::Uint(5),		// lastUpdateTimestamp
			ethabi::ParamType::Uint(2),		// id
			ethabi::ParamType::Address,		// aTokenAddress
			ethabi::ParamType::Address,		// stableDebtTokenAddress
			ethabi::ParamType::Address,		// variableDebtTokenAddress
			ethabi::ParamType::Address,		// interestRateStrategyAddress
			ethabi::ParamType::Uint(16),	// accruedToTreasury
			ethabi::ParamType::Uint(16),	// unbacked
			ethabi::ParamType::Uint(16),	// isolationModeTotalDebt
		],
		&value,
	).ok().unwrap();

	ReserveData::new(&decoded).unwrap()
}

pub fn get_asset_configuration(mm_pool: EvmAddress, asset: EvmAddress) -> U256 {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(mm_pool, caller);
	let mut data = Into::<u32>::into(Function::GetConfiguration).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(asset).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	U256::checked_from(&value[0..32]).unwrap()
}

// Returns Bitmap of the users collaterals and borrows.
// It is divided into pairs of bits, one pair per asset.
// The first bit indicates if an asset is used as collateral by the user, the second whether an asset is borrowed by the user.
// The corresponding assets are in the same position as getReservesList().
pub fn get_user_configuration(mm_pool: EvmAddress, user: EvmAddress) -> U256 {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(mm_pool, caller);
	let mut data = Into::<u32>::into(Function::GetUserConfiguration).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(user).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	U256::checked_from(&value[0..32]).unwrap()
}

pub fn get_asset_symbol(asset_address: &EvmAddress) -> Vec<u8> {
	let dispatch_address = hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(*asset_address, caller);
	let mut data = Into::<u32>::into(hydradx_runtime::evm::Function::Symbol).to_be_bytes().to_vec();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	let decoded = ethabi::decode(
		&[
			ethabi::ParamType::String,
		],
		&value
	).ok().unwrap();

	let symbol = decoded[0].clone().into_string().unwrap();
	symbol.into_bytes()
}

pub fn scaled_balance_of(asset_address: EvmAddress, user: EvmAddress) -> U256 {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(asset_address, caller);
	let mut data = Into::<u32>::into(Function::ScaledBalanceOf).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(user).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	U256::checked_from(&value[0..32]).unwrap()
}

pub fn balance_of(asset_address: EvmAddress, user: EvmAddress) -> U256 {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let context = CallContext::new_call(asset_address, caller);
	let mut data = Into::<u32>::into(Function::BalanceOf).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(user).as_bytes());

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 500_000);
	assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	U256::checked_from(&value[0..32]).unwrap()
}

pub fn current_evm_block_timestamp() -> u64 {
	use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApiV5;
	hydradx_runtime::Runtime::current_block().unwrap().header.timestamp / 1_000
}

pub fn update_oracle_price(oracle_data: Vec<(&str, U256)>) {
	let caller = EvmAddress::from_slice(&hex!("6c345254C4da3B16559e60570fe35311b9597f07"));
	let oracle_address = EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));
	let context = CallContext::new_call(oracle_address, caller);

	let mut data = Into::<u32>::into(Function::SetMultipleValues).to_be_bytes().to_vec();

	let mut token_string_array = Vec::new();
	let mut token_uint_array = Vec::new();

	for data in oracle_data.iter() {
		token_string_array.push(Token::String(data.0.to_string()));
		token_uint_array.push(Token::Uint(data.1.clone()));
	}

	let encoded_values = encode(&[Token::Array(token_string_array), Token::Array(token_uint_array)]);

	data.extend_from_slice(&encoded_values);

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

pub fn get_oracle_price(asset_pair: &str) -> (U256, U256) {
	let oracle_address = EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));
	let context = CallContext::new_view(oracle_address);
	let mut data = Into::<u32>::into(Function::GetValue).to_be_bytes().to_vec();
	let encoded_value = encode(&[Token::String(asset_pair.to_string())]);
	data.extend_from_slice(&encoded_value);

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
		// Arrange
		// PoolAddressesProvider contract
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		// get Pool contract address
		let pool_contract = get_pool(pap_contract);
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
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

		let (price, timestamp) = get_oracle_price("DOT/USD");
		let price = price.as_u128() * 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);

		let (price, timestamp) = get_oracle_price("WETH/USD");
		let price = price.as_u128() / 5;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		update_oracle_price(vec![("WETH/USD", U256::checked_from(&data[0..32]).unwrap())]);

		// ensure that the health_factor < 1
		let user_data = get_user_account_data(pool_contract, alice_evm_address);
		assert!(user_data.5 < U256::from(1_000_000_000_000_000_000u128));

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
		// PoolAddressesProvider contract
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		// get Pool contract address
		let pool_contract = get_pool(pap_contract);
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

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
		let user_data = get_user_account_data(pool_contract, alice_evm_address);
		assert!(user_data.5 > U256::from(1_000_000_000_000_000_000u128));

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
			pallet_liquidation::Error::<hydradx_runtime::Runtime>::LiquidationCallFailed
		);

		// Assert
		assert_eq!(Currencies::free_balance(DOT, &pallet_acc), 0);
		assert_eq!(Currencies::free_balance(WETH, &pallet_acc), 0);

		assert_eq!(Currencies::free_balance(DOT, &BOB.into()), 0);
		assert_eq!(Currencies::free_balance(WETH, &BOB.into()), 0);
	});
}

#[test]
fn rrr() {
	env_logger::init();
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		let pool_contract = get_pool(pap_contract);
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		let evm_acc = H160::from_slice(hex!("81d58b3083589b6053e7bd8caeb06757068592fb").as_slice());
		let acc = hydradx_runtime::EVMAccounts::account_id(evm_acc);
		assert_ok!(Currencies::deposit(
			WETH,
			&acc.clone().into(),
			ALICE_INITIAL_WETH_BALANCE
		));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Act
		// let _result = hydradx_runtime::Executive::apply_extrinsic(unchecked_eth_tx(VALID_ETH_TX));

		let mut tx = ethereum_transaction(VALID_ETH_TX);
		if let pallet_ethereum::Transaction::EIP1559(ref mut inner_tx) = tx {
			inner_tx.chain_id = 0u64;
		}
		let tx = pallet_ethereum::Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: U256::from(9264),
			gas_price: U256::from(5143629),
			gas_limit: U256::from(80674),
			action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
				hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
			)),
			value: U256::from(0), // 0x40	= 64	/ 120 = 288 / 80 = 128
			input: hex!(
				"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
			)
			.encode_as(),
			signature: ethereum::TransactionSignature::new(
				444480,
				H256::from_slice(hex!("6fd26272de1d95aea3df6d0a5eb554bb6a16bf2bff563e2216661f1a49ed3f8a").as_slice()),
				H256::from_slice(hex!("4bf0c9b80cc75a3860f0ae2fcddc9154366ddb010e6d70b236312299862e525c").as_slice()),
			)
			.unwrap(),
		});

		let call: hydradx_runtime::RuntimeCall = pallet_ethereum::Call::<Runtime>::transact {
			transaction: tx.clone(),
		}
		.into();
		let signer = call.check_self_contained().unwrap().unwrap();
		println!("RRR: {:?}\n", tx);
		let unchecked_tx = hydradx_runtime::TransactionConverter.convert_transaction(tx);

		let result = hydradx_runtime::Executive::validate_transaction(
			sp_runtime::transaction_validity::TransactionSource::External,
			unchecked_tx,
			Default::default(),
		);
		println!("Result: {:?}\n", result);

		// Liquidation::parse_oracle_data();
	});
}


// A valid signed Alice transfer.
pub const VALID_ETH_TX: &str = "02f869820501808085e8d4a51000825208943cd0a705a2dc65e5b1e1205896baa2be8a07c6e00180c\
	001a061087911e877a5802142a89a40d231d50913db399eb50839bb2d04e612b22ec8a01aa313efdf2\
	793bea76da6813bda611444af16a6207a8cfef2d9c8aa8f8012f7";

pub fn unchecked_eth_tx(raw_hex_tx: &str) -> hydradx_runtime::UncheckedExtrinsic {
	hydradx_runtime::TransactionConverter.convert_transaction(ethereum_transaction(raw_hex_tx))
}

pub fn ethereum_transaction(raw_hex_tx: &str) -> pallet_ethereum::Transaction {
	let bytes = hex::decode(raw_hex_tx).expect("Transaction bytes.");
	let transaction = ethereum::EnvelopedDecodable::decode(&bytes[..]);
	assert!(transaction.is_ok());
	transaction.unwrap()
}

#[test]
fn decode_dia_set_multiple_values() {
	let encoded = hex!(
		"\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
	);

	let decoded = decode(
		&[
			ParamType::Array(Box::new(ParamType::String)),
			ParamType::Array(Box::new(ParamType::Uint(32))),
		],
	    &encoded,
	).unwrap();

	let mut dai_oracle_data = Vec::new();
	if decoded.len() == 2 {
		for (asset_str, price) in sp_std::iter::zip(decoded[0].clone().into_array().unwrap().iter(), decoded[1].clone().into_array().unwrap().iter()) {
			dai_oracle_data.push((asset_str.clone().into_string().unwrap(), price.clone().into_uint().unwrap()));

		}
	};

	let decoded = ethabi::encode(&[
		ethabi::Token::Array(vec![ethabi::Token::String("WETH/USD".to_owned())]),
		ethabi::Token::Array(vec![ethabi::Token::Uint(11111111.into())]),
	]);
}

#[test]
fn parse_dia_oracle_transaction_should_work() {
	env_logger::init();
	TestNet::reset();
	use xcm_emulator::TestExt;
	// hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
	Hydra::execute_with(|| {
		let tx = pallet_ethereum::Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: U256::from(9264),
			gas_price: U256::from(5143629),
			gas_limit: U256::from(80674),
			action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
				hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
			)),
			value: U256::from(0), // 0x40	= 64	/ 120 = 288 / 80 = 128
			input: hex!(
				"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			444f542f45544800000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			4441492f45544800000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
			)
				.encode_as(),
			signature: ethereum::TransactionSignature::new(
				444480,
				H256::from_slice(hex!("6fd26272de1d95aea3df6d0a5eb554bb6a16bf2bff563e2216661f1a49ed3f8a").as_slice()),
				H256::from_slice(hex!("4bf0c9b80cc75a3860f0ae2fcddc9154366ddb010e6d70b236312299862e525c").as_slice()),
			)
				.unwrap(),
		});

		let parsed_data = Liquidation::parse_oracle_transaction(tx);
		println!("{:#?}", parsed_data);
	})
}

#[test]
fn call_methods_for_liquidation_worker() {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Arrange
		hydradx_run_to_next_block();

		// PoolAddressesProvider contract
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		// get Pool contract address
		let pool_contract = get_pool(pap_contract);
		let oracle_contract = get_price_oracle(pap_contract);
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
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

		// assert_eq!(
		// 	Currencies::free_balance(DOT, &ALICE.into()),
		// 	ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount
		// );
		// assert_eq!(
		// 	Currencies::free_balance(WETH, &ALICE.into()),
		// 	ALICE_INITIAL_WETH_BALANCE - collateral_weth_amount
		// );
		//
		let borrow_dot_amount: Balance = 5_000 * DOT_UNIT;
		borrow(pool_contract, alice_evm_address, dot_asset_address, borrow_dot_amount);
		// assert_eq!(
		// 	Currencies::free_balance(DOT, &ALICE.into()),
		// 	ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount + borrow_dot_amount
		// );

		// let (price, timestamp) = get_oracle_price("DOT/USD");
		// let price = price.as_u128() * 5;
		// let timestamp = timestamp.as_u128() + 6;
		// let mut data = price.to_be_bytes().to_vec();
		// data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		// update_oracle_price(vec![("DOT/USD", U256::checked_from(&data[0..32]).unwrap())]);
		//
		// let (price, timestamp) = get_oracle_price("WETH/USD");
		// let price = price.as_u128() / 5;
		// let timestamp = timestamp.as_u128() + 6;
		// let mut data = price.to_be_bytes().to_vec();
		// data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		// update_oracle_price(vec![("WETH/USD", U256::checked_from(&data[0..32]).unwrap())]);

		// ensure that the health_factor < 1
		let user_data = get_user_account_data(pool_contract, alice_evm_address);
		// assert!(user_data.5 < U256::from(1_000_000_000_000_000_000u128));
		println!(" - - - - USER DATA: \ntotal coll {:?} \ntotal debt {:?}\n {:?}\n avg liq threshold {:?}\n LTV {:?}\n HF {:?}", user_data.0, user_data.1, user_data.2, user_data.3, user_data.4, user_data.5);

		// let route = Router::get_route(AssetPair {
		// 	asset_in: WETH,
		// 	asset_out: DOT,
		// });




		// 1603597735704195190
		// 1396895269127220621

		////////////////////////////////////////////////
		let oracle_address = EvmAddress::from_slice(&hex!("C756bD338A97c1d2FAAB4F13B5444a08a1566917"));
		let context = CallContext::new_view(oracle_address);
		let mut data = Into::<u32>::into(Function::GetValue).to_be_bytes().to_vec();
		let encoded_value = encode(&[Token::String("DOT/USD".to_string())]);
		data.extend_from_slice(&encoded_value);

		let from = alice_evm_address;
		let to = oracle_address;
		let value = U256::default();
		let gas_limit = U256::from(100_000);
		let max_fee_per_gas = None;
		use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApiV5;
		let res = hydradx_runtime::Runtime::call(from, to, data, value, gas_limit, max_fee_per_gas, None, None, true, None);


		let money_market_data = MoneyMarketData::new(pap_contract);
		let user_data = UserData::new(&money_market_data, alice_evm_address);

		for (i, a) in money_market_data.reserves.iter().enumerate() {
			let is_collateral = user_data.is_collateral(i);
			let is_debt = user_data.is_debt(i);
			// println!(" - - - - {:?} {:?} {:?}", String::from_utf8(symbols[i].clone()).unwrap(), is_collateral, is_debt);
		}

		hydradx_run_to_next_block();
		let current_evm_timestamp = current_evm_block_timestamp();
		for reserve in money_market_data.reserves.iter() {
			// let normalized_income = reserve.get_normalized_income(current_evm_timestamp);
			// println!("normalized_income {:?}", normalized_income);
			let (a, b) = reserve.get_collateral_and_debt_addresses();


			// println!("\n\nSYMBOL  {:?}", String::from_utf8(reserve.symbol.clone()));
			// println!("DECIMALS  {:?}", reserve.get_decimals());
			// println!("LIQ RATE  {:?}", reserve.reserve_data.current_liquidity_rate.clone());
			let price = get_asset_price(oracle_contract, reserve.asset_address, alice_evm_address);
			// println!("PRICE  {:?}", price);
			println!("\n\nCOLL  {:?}", reserve.get_user_collateral_in_base_currency(user_data.address, current_evm_timestamp, price));
			println!("LIQ THRESHOLD  {:?} {:?}",String::from_utf8(reserve.symbol.clone()).unwrap(), reserve.reserve_data.liquidation_threshold());
			println!("DEBT  {:?}", reserve.get_user_debt_in_base_currency(user_data.address, current_evm_timestamp, price));


			// for loop
			// 			vars.avgLiquidationThreshold += vars.userBalanceInBaseCurrency * vars.liquidationThreshold);

			// vars.avgLiquidationThreshold = vars.avgLiquidationThreshold / vars.totalCollateralInBaseCurrency

			// vars.healthFactor = (vars.totalCollateralInBaseCurrency.percentMul(vars.avgLiquidationThreshold)).wadDiv(
			//         vars.totalDebtInBaseCurrency




			// let scaled_balance = scaled_balance_of(a, alice_evm_address);
			// println!("coll {:?}", scaled_balance);
			// let scaled_balance = balance_of(b.0, alice_evm_address);
			// println!("debt {:?}", scaled_balance);
			// let scaled_balance = scaled_balance_of(b.1, alice_evm_address);
			// println!("debt {:?}", scaled_balance);

		}


		let hf = user_data.health_factor(&money_market_data, current_evm_timestamp, alice_evm_address);
		println!("\n\n\n\nHF {:?} ", hf.unwrap());

		let dummy_data = fetch_dummy_data();
		println!("\n\n\n {:?} ", dummy_data);

		let user_data = UserData::new(&money_market_data, dummy_data[0].0);
		let hf = user_data.health_factor(&money_market_data, current_evm_timestamp, alice_evm_address);
		println!("\n\n\n\nHF {:?} ", hf);
		println!("DATA {:?} ", user_data);


		// let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
		// assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
		// let price = U256::checked_from(&value[0..32]).unwrap();
		// let timestamp = U256::checked_from(&value[32..64]).unwrap();
		//
		// (price, timestamp)
	});
}

fn fetch_dummy_data() -> Vec<(EvmAddress, pallet_liquidation::BorrowerDataDetails<hydradx_runtime::AccountId>)> {
	pub const TEST_DATA: &'static str = r#"{"lastGlobalUpdate":7245144,"lastUpdate":7245151,"borrowers":[["0x1acc506f91c6b8dfd37f1a9361d205363d9cc9cf",{"totalCollateralBase":0,"totalDebtBase":0.89320925,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1743426784668,"account":"7JChDxuBxpopLkj2rQTJ7DZi9kFKg3YBEW12HYa29FJAHoAR","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x56b2ea0f360d095f0fe04d23f19784e1b9f7f92d",{"totalCollateralBase":0,"totalDebtBase":0.0003283,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1743426784673,"account":"7KZEYyPwwbXk6f69hJcWrCLx3WoDyLNXN9TGn5TfNXiToVky","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x4cc7b36a1f27af1ca1b4ff66e259da96d300ca3d",{"totalCollateralBase":0,"totalDebtBase":0.00239625,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1743426784676,"account":"7KLEFLtuS4iQV57YjZJSWUKi3iPrSjzwMaU9oAtacCfJ8TA6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5c82f3a21b49dec00ccb3505adba2af107ca9414",{"totalCollateralBase":0.7374992,"totalDebtBase":0.68324754,"availableBorrowsBase":0,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":0.8635221138154409,"updated":1743426781232,"account":"7KgrbG52RBUgk6PPyfkiECSSJ3rJVCoDGSSrjNdcjBespseq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x56417b9cadf44d16d1bc6adabc16693f642e9139",{"totalCollateralBase":0.00012498,"totalDebtBase":0.00011216,"availableBorrowsBase":0,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":0.8914051355206848,"updated":1743426781201,"account":"7KATdGawRT3erLU1kxR57V5uF7gWoMUF6RH1MDLCyB96E8WN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x75362b10afd0e72dc9b6a103ef2fe3bb6739d186",{"totalCollateralBase":2.01308833,"totalDebtBase":1.79852636,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.0073688883825977,"updated":1743426781206,"account":"7KATdGb3dCxMY7yfiSKprsrvQddYnN3bzum8M54JC6ATRcHC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7e878d91757ee4e599109fa861909f177e7785b0",{"totalCollateralBase":9.87877665,"totalDebtBase":8.80377064,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.0098967083040682,"updated":1743426781226,"account":"7LTTay6WKX4Aq1oJHrQhwb3FjndEk4jtKHm5u8CNFBii53T8","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x04ceb6a33f688b53a0c6684efb161ce1e4b3925d",{"totalCollateralBase":12.96945162,"totalDebtBase":8.87050707,"availableBorrowsBase":0,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.023460785089031,"updated":1743426781216,"account":"7HhrtCdrGEL4yVGkJkVud4ANNhB1xKS8VvZwgqGa6bN8R2zE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x562d5086ab334d537769c9ff0c730b18a95d38bc",{"totalCollateralBase":3578.86287808,"totalDebtBase":2653.09544241,"availableBorrowsBase":31.05171615,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.0791508879376184,"updated":1743426781240,"account":"7KYYs6wX1Ny5tn3NsdKcQvwWjhxJZ91epbjbiro7wKv8ieLi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x74072c1b55ff8be8d496606a0ec366ccce9b5aaa",{"totalCollateralBase":34.38189036,"totalDebtBase":25.37575259,"availableBorrowsBase":0.41066518,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.0839289275242734,"updated":1743426781262,"account":"7LDgyFGFDi69GNed1tEMFgkpGwpT8FvLgkNLsYzhBYWhQfsG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0cc8969938f6ce86ae3fe79c808dbd198397d501",{"totalCollateralBase":66414.57308484,"totalDebtBase":54681.40585589,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.0931159292774757,"updated":1743426781254,"account":"7HtKStLktsuUsWuNYJeWxz66Jou87bcdhRo7ML9xhTrT7CHN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe8881d8b8548331bc5df86b2e729774f876f836e",{"totalCollateralBase":24.37675555,"totalDebtBase":17.75859246,"availableBorrowsBase":0.5239742,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.0981390830340616,"updated":1743426781235,"account":"7NrSq43HzxthQTRWs4JGFYBtGE4QGrPy6GvdLjj4raUVjBJr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x77410188f811d7caa863dcc2e9e263721bc49fd7",{"totalCollateralBase":13781.03591073,"totalDebtBase":10017.0596975,"availableBorrowsBase":318.71723555,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1006052735546252,"updated":1743426781193,"account":"7LHvJ2hp6dSRqQfcqYLG85KNMBqPeLhA6g63JEypiX4Y5YAi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xae262a4f42c640e0875ace3c112955363657e76e",{"totalCollateralBase":593.90248582,"totalDebtBase":470.98040952,"availableBorrowsBase":26.41292235,"currentLiquidationThreshold":87.58,"ltv":83.75,"healthFactor":1.1043767141187482,"updated":1743426781229,"account":"7MXtxKwumQoBXaT4x9v8nQxWCMQn6URyARqa5pJAwNNCckLr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x34cd97eea8d7958109c6db5e35eb407390555d9d",{"totalCollateralBase":9889.31874133,"totalDebtBase":7130.03943072,"availableBorrowsBase":286.94962528,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1095948444511046,"updated":1743426781265,"account":"7JnnrDVoGrXA68TuQMVasG8TD8D2iagjmBA3bSEYyBHphbvy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x34CD97EEA8D7958109C6DB5E35Eb407390555D9D",{"totalCollateralBase":9889.31874133,"totalDebtBase":7130.03943072,"availableBorrowsBase":286.94962528,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1095948444511046,"updated":1743426781252,"account":"7JnnrDVoGrXA68TuQMVasG8TD8D2iagjmBA3bSEYyBHphbvy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x60e265013352b87debe7efa75798e7994185eb96",{"totalCollateralBase":408.66054919,"totalDebtBase":294.62471537,"availableBorrowsBase":11.87069652,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1096436323729049,"updated":1743426781212,"account":"7Knb8p3vnLFKnEoPr9BabjUisMUc9KuPsfLXVr6d6yWAQNWm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x60E265013352B87DeBe7eFA75798e7994185EB96",{"totalCollateralBase":408.66054919,"totalDebtBase":294.62471537,"availableBorrowsBase":11.87069652,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1096436323729049,"updated":1743426781223,"account":"7Knb8p3vnLFKnEoPr9BabjUisMUc9KuPsfLXVr6d6yWAQNWm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x58f1dbf9ce44114d9b99dc5ba0903528aae6478f",{"totalCollateralBase":150031.98298906,"totalDebtBase":121200.86272354,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1140909532810142,"updated":1743426781248,"account":"7KcBM3YRJAvhNQn5ir96h2JJf8sXQWViMGUyguZRU7qGM9Vi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x58f1DbF9ce44114d9b99DC5ba0903528AaE6478f",{"totalCollateralBase":150031.98298906,"totalDebtBase":121200.86272354,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1140909532810142,"updated":1743426781270,"account":"7KcBM3YRJAvhNQn5ir96h2JJf8sXQWViMGUyguZRU7qGM9Vi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb833625dcf03dc9282433e23e1129fa20791bf6d",{"totalCollateralBase":37623.23535293,"totalDebtBase":26952.06805797,"availableBorrowsBase":1265.35845673,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1167450385477764,"updated":1743426781267,"account":"7Mm5MrjELZ6NpZXJC3E63pnBAiLPxr5SqdhvFvRWeZMbomaP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xB833625dcF03DC9282433E23e1129fA20791Bf6D",{"totalCollateralBase":37623.23535293,"totalDebtBase":26952.06805797,"availableBorrowsBase":1265.35845673,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1167450385477764,"updated":1743426781311,"account":"7Mm5MrjELZ6NpZXJC3E63pnBAiLPxr5SqdhvFvRWeZMbomaP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x4425333101d35f55eccfe16b28f0e65007a0076c",{"totalCollateralBase":877.43336106,"totalDebtBase":628.14727247,"availableBorrowsBase":29.92774833,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1174874422200483,"updated":1743426781365,"account":"7K8ubJP81cZNSNsgBix1B4eF14YsQyKXUZAcqtGw9ZfsnFDz","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6c1297ad63c76e3dd7190cba99c673e5d9ba7a36",{"totalCollateralBase":317.47733727,"totalDebtBase":226.89406853,"availableBorrowsBase":11.21393442,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1193852332301866,"updated":1743426781350,"account":"7L3Fynz5DGbjj9nUFXyupNtaMedUPNzyDRPJpL4NNFW3EzvP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x582ad31f609b3c75d026958ca31ad064358a86bf",{"totalCollateralBase":2.82905866,"totalDebtBase":2.0102975,"availableBorrowsBase":0.1114965,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.125826863934318,"updated":1743426781341,"account":"7KbADkxDxasws9b3h4ooApqdyuddJaLSy4h2pfxxzUxXbY3B","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xeed4ffd9c654ee1b26957cf2a3e70a05e6078948",{"totalCollateralBase":216.5843103,"totalDebtBase":152.43409773,"availableBorrowsBase":10.004135,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1366711964071268,"updated":1743426781383,"account":"7NzhxQ26KH1GQHrusE1b9azLR2465zkKhBR2NogSJJnsfeQn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x26219557f2597e5d6194b4c8f9b23570220d6cac",{"totalCollateralBase":284602.68374693,"totalDebtBase":174717.77474824,"availableBorrowsBase":0,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1402496335013381,"updated":1743426781385,"account":"7JTZ5fjcmPLJpbe9UEHMNQbW2vy741RnxgYEQf7jU5b4ajXq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7802f0469d452910f6543615068013217b763ba8",{"totalCollateralBase":24728.13541438,"totalDebtBase":19490.63445644,"availableBorrowsBase":291.87387506,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1418469687417747,"updated":1743426781360,"account":"7LJuuQn6hrJTyV5CZsQmCBB8yXJWyW4Pyikm88mdCkBF5XPF","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10817a5e4400515e85d2e0e8c2b190116b653c4d",{"totalCollateralBase":25022.18664753,"totalDebtBase":19680.13083732,"availableBorrowsBase":337.6184807,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1442997086215878,"updated":1743426781368,"account":"7KATdGahSqi1bU1oNjgwsiNJGAXgkbTGMgM5mxVrFXRqB9Dt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf4e29ca3fd2936858e1cd3a5c7a049b07f002853",{"totalCollateralBase":21.70796532,"totalDebtBase":15.14318226,"availableBorrowsBase":1.13779173,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1468112819240412,"updated":1743426781334,"account":"7P8eHbgdhdzDhytPQ16APwkLLXUdwn5haRFXgpPnMMcZyQSj","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32b27ec11da5f81abe4ae44203c91fb9c80a291b",{"totalCollateralBase":31365.56685192,"totalDebtBase":21828.48043275,"availableBorrowsBase":1695.69470619,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1495281844673417,"updated":1743426781329,"account":"7Jk2hmVZPQ4BjRxSfqdzkhTyVHbj4fb6hspeyG7yKMYAbedq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32B27ec11Da5f81abe4aE44203c91fb9c80a291b",{"totalCollateralBase":31365.56685192,"totalDebtBase":21828.48043275,"availableBorrowsBase":1695.69470619,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1495281844673417,"updated":1743426781362,"account":"7Jk2hmVZPQ4BjRxSfqdzkhTyVHbj4fb6hspeyG7yKMYAbedq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8aa40cdeb25d51c3f0958576ec3d92820c801553",{"totalCollateralBase":7252.92077561,"totalDebtBase":5039.97224485,"availableBorrowsBase":399.71833686,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1512636059492207,"updated":1743426781376,"account":"7LjLdHuY5qiMMCSkNH6Nxt6NZfPJ4FBBMpFUGTtb7fKzRRxq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc04455453c91e51576ad6ebf2802c3ce71df0748",{"totalCollateralBase":6930.59080733,"totalDebtBase":5412.9904894,"availableBorrowsBase":131.48215646,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1523263783327644,"updated":1743426781326,"account":"7Mwen6fYUmsmWDRQMKLKrb5jtNCYAbaWJmGmXAowz1haUccv","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x185cc996005dffb9804257be76c7bea356c8f77f",{"totalCollateralBase":57116.73736141,"totalDebtBase":39641.83122142,"availableBorrowsBase":3195.72179964,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.152655880953353,"updated":1743426781370,"account":"7J9VzrBEr3UJA7oHdBriJjHhsAteuoudZX6Xm4pYi5nTUzLD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x185CC996005dFfB9804257be76C7beA356C8F77F",{"totalCollateralBase":57116.73736141,"totalDebtBase":39641.83122142,"availableBorrowsBase":3195.72179964,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.152655880953353,"updated":1743426781373,"account":"7J9VzrBEr3UJA7oHdBriJjHhsAteuoudZX6Xm4pYi5nTUzLD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x84a8306ae1be19f92d81fffe9da6ab787109d5ce",{"totalCollateralBase":14964.30307936,"totalDebtBase":10382.79047789,"availableBorrowsBase":840.43683163,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1530081907154932,"updated":1743426781344,"account":"7LbVZwRz9ik7oohoRwL3mVK5PfRSQiSfBR8TV68D1S7juZnD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7cb0517b58ec990eddcc664c2907f57311e3b41e",{"totalCollateralBase":166.23755284,"totalDebtBase":100.79155339,"availableBorrowsBase":0,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1545241945000646,"updated":1743426781380,"account":"7LR3bnMvXAoJjnF5iUCivgRB8wm8K5cq84cxyHR59mTqzqMK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf42fc37ca29f3ba9ceb2e0f28ea6b450a2f3cabd",{"totalCollateralBase":4588.64419107,"totalDebtBase":3314.01455764,"availableBorrowsBase":135.72814521,"currentLiquidationThreshold":83.4,"ltv":75.18,"healthFactor":1.15477140754483,"updated":1743426781347,"account":"7P7jA7dPQ8rMdhnU4oUkL3PFTkXrhvsFCT4oE8vzeZCK8KoH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf42fC37ca29f3BA9Ceb2E0f28Ea6B450a2F3caBd",{"totalCollateralBase":4588.64419107,"totalDebtBase":3314.01455764,"availableBorrowsBase":135.72814521,"currentLiquidationThreshold":83.4,"ltv":75.18,"healthFactor":1.15477140754483,"updated":1743426781353,"account":"7P7jA7dPQ8rMdhnU4oUkL3PFTkXrhvsFCT4oE8vzeZCK8KoH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc85e5b06f48428c0e2070b4a4f55ca6493a15666",{"totalCollateralBase":4831.73780522,"totalDebtBase":3342.6500082,"availableBorrowsBase":281.15334572,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1563849744058288,"updated":1743426781337,"account":"7N8Gtg3b3nb62vvoG28RFRY7ZJyE3RsmC9heTb7M1R7DkzrG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc85e5B06F48428c0e2070B4A4f55CA6493a15666",{"totalCollateralBase":4831.73780522,"totalDebtBase":3342.6500082,"availableBorrowsBase":281.15334572,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1563849744058288,"updated":1743426781388,"account":"7N8Gtg3b3nb62vvoG28RFRY7ZJyE3RsmC9heTb7M1R7DkzrG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8eb7ae465ffc861172e0a865b22b48779f43378c",{"totalCollateralBase":322.0748579,"totalDebtBase":220.92082415,"availableBorrowsBase":20.63531928,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1662996791332585,"updated":1743426781468,"account":"7LpgedKS6NaGXvmHLipBs3Yhbbh72RjeuiQMM7LukVpvs9T1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xdcb8a44a43fd35358fc40e9671dce2432e572a69",{"totalCollateralBase":26.20944462,"totalDebtBase":17.95596634,"availableBorrowsBase":1.70111713,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1677208178593634,"updated":1743426781432,"account":"7NaxfpDqzbtrNCVMZs2fMisdPbkPsSYhvcvsEqiGMvccNE8S","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1c5e8115239b5527cf9f5fbd6fdd032b20ae252a",{"totalCollateralBase":17.79423927,"totalDebtBase":10.64485701,"availableBorrowsBase":0.03168655,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1701394841000312,"updated":1743426781481,"account":"7JEkhY8BWNWCf2tUpytLYSrFfbQYtqEV54TAhZTceMBi1moj","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x58358640a707b5a6998f846a227d5630a45a82bc",{"totalCollateralBase":769.90628212,"totalDebtBase":526.12970006,"availableBorrowsBase":51.30001153,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1706714630057184,"updated":1743426781438,"account":"7KbDQ7NLW97WYxrEyXdTJdrfanRNWnynqVWMfMGh61xdxNS9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x54c9b04ccd87caea676fe0959fea909e295e0935",{"totalCollateralBase":4350.3071582,"totalDebtBase":2782.11617549,"availableBorrowsBase":155.21121773,"currentLiquidationThreshold":75.01,"ltv":67.52,"healthFactor":1.1729076693913671,"updated":1743426781484,"account":"7KWjDp3Y72usf7btscKvVidH2yjxF4LcNE9zrC4TzVsSPgM2","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x864159e69c32e12b73db380366c2ccc8c1611fe8",{"totalCollateralBase":873.64045432,"totalDebtBase":594.60429712,"availableBorrowsBase":60.62604362,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1754243399269433,"updated":1743426781451,"account":"7Ldb7duiNdnPrAsqqPkuCHfaUPid3nxoS1Sguso7B9Nk3sq9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc02dc42dd88fce69f499dab19e663954b23eae8a",{"totalCollateralBase":834.45867358,"totalDebtBase":567.58523214,"availableBorrowsBase":58.25877305,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.17615276271906,"updated":1743426781474,"account":"7MwY5HKazABMVvB11zR6GAgVEPUseAyTd5CuPmsk21r88447","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9c51f33019ae97643c1fac54d8bdc0a3eff9f0ec",{"totalCollateralBase":2022.10315024,"totalDebtBase":1372.11473716,"availableBorrowsBase":144.46262552,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1789702977305496,"updated":1743426781443,"account":"7KATdGbBTiF8WmRRZd5JLo6LBNFVyShMmx6YKiT1Q7XaUc1t","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9c51F33019aE97643C1faC54d8Bdc0a3eFf9F0ec",{"totalCollateralBase":2022.10315024,"totalDebtBase":1372.11473716,"availableBorrowsBase":144.46262552,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1789702977305496,"updated":1743426781491,"account":"7KATdGbBTiF8WmRRZd5JLo6LBNFVyShMmx6YKiT1Q7XaUc1t","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5857d73e49e89496cc67707c44fde97fa30264e6",{"totalCollateralBase":51211.76082893,"totalDebtBase":30215.71540016,"availableBorrowsBase":511.3410972,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1864101877283426,"updated":1743426781447,"account":"7KbPbNHsVaJ7GrEAWb3yCAf7v8Pyy54a4gbT8RfKzCgh5dJU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5857D73e49E89496CC67707C44fde97fa30264E6",{"totalCollateralBase":51211.76082893,"totalDebtBase":30215.71540016,"availableBorrowsBase":511.3410972,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1864101877283426,"updated":1743426781477,"account":"7KbPbNHsVaJ7GrEAWb3yCAf7v8Pyy54a4gbT8RfKzCgh5dJU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc0f05d300585d81d5b589ecf07a2197fc48df3f8",{"totalCollateralBase":326003.83050999,"totalDebtBase":219581.27999962,"availableBorrowsBase":24921.59288287,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1877290468861523,"updated":1743426781496,"account":"7MxXt83nnmMJ8gCqhmwmvM5nQvjHijfw6asMb7qqmZ4NUtbd","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xC0f05d300585d81D5b589eCF07A2197fc48Df3f8",{"totalCollateralBase":326003.83050999,"totalDebtBase":219581.27999962,"availableBorrowsBase":24921.59288287,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1877290468861523,"updated":1743426781500,"account":"7MxXt83nnmMJ8gCqhmwmvM5nQvjHijfw6asMb7qqmZ4NUtbd","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6ef312d98279ba2a17916edf701175ac80555b6f",{"totalCollateralBase":1354.54694897,"totalDebtBase":910.89571311,"availableBorrowsBase":105.01449862,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1896395422481691,"updated":1743426781488,"account":"7L72m4v2Cc4fDyUSaXK7uaQon8d78MbtvuGeMenu8GSUdGnx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8ae6e65078a5263f4f335e160b8da0ed59518f69",{"totalCollateralBase":4048.9998015,"totalDebtBase":2717.90373133,"availableBorrowsBase":318.8461198,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1918007999550835,"updated":1743426781462,"account":"7LjgV5d6FPWKGzjBC4Ewn8YMPtTaubFRdgTnAnuuiNnVh5tx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8ae6E65078a5263F4F335E160B8da0eD59518f69",{"totalCollateralBase":4048.9998015,"totalDebtBase":2717.90373133,"availableBorrowsBase":318.8461198,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1918007999550835,"updated":1743426781493,"account":"7LjgV5d6FPWKGzjBC4Ewn8YMPtTaubFRdgTnAnuuiNnVh5tx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x60a0d45f10f7f0d64e71cd152b82ed529f4d9a51",{"totalCollateralBase":279.02089475,"totalDebtBase":210.17747291,"availableBorrowsBase":13.03924289,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1947941033030285,"updated":1743426781458,"account":"7KnFf9sjg2DrDrTSGvUfPWaJn9fXbSQJBthj32NevT9bCMkh","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7525ccef62ae236bffc555f76458a9b3c46961ff",{"totalCollateralBase":691.77142471,"totalDebtBase":403.8823334,"availableBorrowsBase":11.18052143,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1989630574418189,"updated":1743426781503,"account":"7KATdGb3cTrke5R8VQYoUzfdC39BBpwPbpcFnMvfkM8cv3u9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7525cCef62ae236bFfc555f76458A9b3C46961FF",{"totalCollateralBase":691.77142471,"totalDebtBase":403.8823334,"availableBorrowsBase":11.18052143,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1989630574418189,"updated":1743426781465,"account":"7KATdGb3cTrke5R8VQYoUzfdC39BBpwPbpcFnMvfkM8cv3u9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x06bd31f6c1ddf6fe8f91e210e1381c5af2447a9e",{"totalCollateralBase":151.4276975,"totalDebtBase":100.51829157,"availableBorrowsBase":13.05248156,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2051752582328534,"updated":1743426781517,"account":"7HkPmvWweUi1i2uKADrGodyd9iJABrx6Dpu5rZBkK9X8yFPy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x24a1b9d4bd6be959c5d55e66f3be0f6591c38b98",{"totalCollateralBase":546.47176485,"totalDebtBase":362.65901932,"availableBorrowsBase":47.19480432,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2054778416919698,"updated":1743426781546,"account":"7JRb3xYLmXcpwVBy1UfuhvQUAw8XxWErRpJ9gvnKbo8Vx23f","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x502c5f63385e3334f63f48c8d79c71e4a4f33c20",{"totalCollateralBase":6728.13033383,"totalDebtBase":4541.13443345,"availableBorrowsBase":560.13398566,"currentLiquidationThreshold":81.64,"ltv":75.82,"healthFactor":1.2095756434955316,"updated":1743426781557,"account":"7KQgJD8dSzdbGYQUDfJaE4rF1oC8yoLVLZnR4wueYW356dXT","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xba8e05dbe5f2831bda3bc369f19c4b90bd58a19e",{"totalCollateralBase":76.843137,"totalDebtBase":50.7896007,"availableBorrowsBase":6.84275205,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.210375918549011,"updated":1743426781610,"account":"7MpAP5cKM8m61QKdPWwD46PN6ePhXA8NP8UazedB9vZYhRPs","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8820ad235ac91d3036a790624b2be637c130b6f9",{"totalCollateralBase":9508.70860013,"totalDebtBase":6279.58266723,"availableBorrowsBase":851.94878287,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2113809600432452,"updated":1743426781571,"account":"7Lg3WDYohqb9NA3RKWqyHTWcywTq46hdkALL2ZDkXHzYPKHU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x521c02a4787b05d9e563e64ea7d5266688ba6498",{"totalCollateralBase":4807.32673563,"totalDebtBase":2777.55967983,"availableBorrowsBase":106.83636155,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.2115414618727336,"updated":1743426781561,"account":"7KTDXrCFaJ8YbvNg7LXZ2121wWatxksDwtdBucqd4EuQJuiK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x521c02A4787B05d9e563e64eA7d5266688ba6498",{"totalCollateralBase":4807.32673563,"totalDebtBase":2777.55967983,"availableBorrowsBase":106.83636155,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.2115414618727336,"updated":1743426781588,"account":"7KTDXrCFaJ8YbvNg7LXZ2121wWatxksDwtdBucqd4EuQJuiK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf09dfdc7dcd63b7d20cbcbe25d3de8eb5431e82f",{"totalCollateralBase":38.02513421,"totalDebtBase":25.05387459,"availableBorrowsBase":3.46497607,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.214187740132693,"updated":1743426781613,"account":"7P33iBgp3pzy6Xnj5PnwyHfpFLMTCKJiG3vzp5MmWRgBEYBf","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xF09dfdc7dCD63b7d20CbCBE25D3de8eB5431E82f",{"totalCollateralBase":38.02513421,"totalDebtBase":25.05387459,"availableBorrowsBase":3.46497607,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.214187740132693,"updated":1743426781603,"account":"7P33iBgp3pzy6Xnj5PnwyHfpFLMTCKJiG3vzp5MmWRgBEYBf","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfc4b86f4ffa38e7f92bd9a08c946b973933a9568",{"totalCollateralBase":67328.84844276,"totalDebtBase":44349.67786676,"availableBorrowsBase":6146.95846531,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.214508906153302,"updated":1743426781137,"account":"7PJMngtPWAgLa3P8PckTfDrwwnwa3BD2oe5AAA5j1MdUvdqg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xFc4b86f4fFA38E7F92bD9a08C946b973933a9568",{"totalCollateralBase":67328.84844276,"totalDebtBase":44349.67786676,"availableBorrowsBase":6146.95846531,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.214508906153302,"updated":1743426781605,"account":"7PJMngtPWAgLa3P8PckTfDrwwnwa3BD2oe5AAA5j1MdUvdqg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2cC5cb836C36d352CFF07d580a9fA01fd594B82c",{"totalCollateralBase":69.90420427,"totalDebtBase":45.93638015,"availableBorrowsBase":6.49177305,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2174090173711696,"updated":1743426781591,"account":"7JcG9de9cMewXXprkxem8ufqZNMZe4wxk2HYomkwqjXGoNwH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd247c203b7c5598692ee05611aa4fcea9b67c777",{"totalCollateralBase":164.09552833,"totalDebtBase":107.81354436,"availableBorrowsBase":15.25810189,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.217624589185707,"updated":1743426781551,"account":"7NMGf6czqfcxtSstPpjQiBmeGg3H39AvzjAvAXxYTLjNp8N7","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa072610f1e1ffade38a6d64df55a89e6b07f65ed",{"totalCollateralBase":12351.98560329,"totalDebtBase":8109.27640323,"availableBorrowsBase":1154.71279924,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2185536651203641,"updated":1743426781594,"account":"7MDvvaZSqfCo5g9cj6c1ecp1HHpoJqUv5VcXyUws4SAdAfTb","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xA072610F1E1FfadE38A6d64df55A89E6b07f65eD",{"totalCollateralBase":12351.98560329,"totalDebtBase":8109.27640323,"availableBorrowsBase":1154.71279924,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2185536651203641,"updated":1743426781597,"account":"7MDvvaZSqfCo5g9cj6c1ecp1HHpoJqUv5VcXyUws4SAdAfTb","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfada8b71c5d2906a12a63adb35dac00d22349528",{"totalCollateralBase":1254.07833813,"totalDebtBase":719.84477559,"availableBorrowsBase":32.60222729,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.2195057413183163,"updated":1743426781564,"account":"7PGUBGXcHV2KamovhBgu74ERmfXTxDS7NizUygnqiFDXsHUG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8eeda641ca05e0ea26570cb33827c27ba8284ee6",{"totalCollateralBase":298.72514188,"totalDebtBase":195.5911857,"availableBorrowsBase":28.45267071,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2218347807684464,"updated":1743426781153,"account":"7LpxgV88VdbhNLTNxNLLt5mCDcMAzRdNgELzA26fecAjNqSt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc07da84351adbf69063b5ffdfece90862d82d764",{"totalCollateralBase":146.55267596,"totalDebtBase":95.93507898,"availableBorrowsBase":13.97942799,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2220987569566912,"updated":1743426781583,"account":"7Mwwomgp4tMUXRcXiHB2aCXyzWBtxG5xPtTnW3N8mC6R3yBB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xddbcf39e88154fdce1fef51b596ce2abc8f38cd9",{"totalCollateralBase":9015.85813247,"totalDebtBase":5892.49970263,"availableBorrowsBase":869.39389672,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2240452897707845,"updated":1743426781580,"account":"7NcHzrEwaqP1zpyVaoME78Zede3aK3MA2crxpvEx77NhVmUK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xddbCF39E88154fDce1FEF51B596Ce2aBC8F38CD9",{"totalCollateralBase":9015.85813247,"totalDebtBase":5892.49970263,"availableBorrowsBase":869.39389672,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2240452897707845,"updated":1743426781567,"account":"7NcHzrEwaqP1zpyVaoME78Zede3aK3MA2crxpvEx77NhVmUK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xdc8b1e22756cf161f243387abb4547bc8cb07862",{"totalCollateralBase":2851.4518831,"totalDebtBase":1630.10500652,"availableBorrowsBase":80.76612334,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.224471006583287,"updated":1743426781574,"account":"7Naj9THL7j3v91HRDxscmvJ3LY2w6SYvPLZBM43Fgeu8U8Db","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80562ba15cbedd1d663909b5967325c21629c31c",{"totalCollateralBase":8885.22542935,"totalDebtBase":5798.89456638,"availableBorrowsBase":865.02450563,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2257819593221766,"updated":1743426781586,"account":"7LVq1gSDYnj9muw7fq4WivYLZAK67cfBkVA3bXFtjRGt5nx4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6a30ef1cf090b1ce0eef2469bae6bbc92b05536f",{"totalCollateralBase":204.90999726,"totalDebtBase":133.52017785,"availableBorrowsBase":20.1623201,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2277395106091076,"updated":1743426781600,"account":"7Kznu1dZDhfBYFcHhA5FaXardkhdZkV4piA53XpjaG6SimCn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe0b98dc813918a7e3e2cc86f58b99f7c92d2ea31",{"totalCollateralBase":1394.14846068,"totalDebtBase":904.64019675,"availableBorrowsBase":140.97114876,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2328865913176106,"updated":1743426781694,"account":"7NgD8e1dRVssnXZCfQKq9MUpPaQhgbV5CzNgQxKybyK676CC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x66508d2d43a34ffc714785c9bb4a4be22fcb5c38",{"totalCollateralBase":3053.9129971,"totalDebtBase":1979.23565627,"availableBorrowsBase":311.19909156,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2343807519536305,"updated":1743426781645,"account":"7Kui6f8NrK5zbGBUfZhriBd727zymtgWefaVNPQX6YG2Spz9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x404b2963a43b0f6aca3bfe2268f7b5ae52d5ec91",{"totalCollateralBase":1497.94765462,"totalDebtBase":970.70801546,"availableBorrowsBase":152.75272551,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2345196543289292,"updated":1743426781220,"account":"7K3rgFpPTE6ewGzunBNH9HH3L5vyog1qpk2caTcZo7a4txRB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x404b2963a43B0F6aCa3Bfe2268F7b5ae52D5EC91",{"totalCollateralBase":1497.94765462,"totalDebtBase":970.70801546,"availableBorrowsBase":152.75272551,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2345196543289292,"updated":1743426781714,"account":"7K3rgFpPTE6ewGzunBNH9HH3L5vyog1qpk2caTcZo7a4txRB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xab0211d3c1ee22c9e0c727964318b366517b4658",{"totalCollateralBase":888.96173252,"totalDebtBase":503.93281745,"availableBorrowsBase":29.44422206,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.2348336746727984,"updated":1743426781665,"account":"7KATdGbEQQjtFPHk87hUaVH7N4tb8RcVxt3uWUVhFLpySmk3","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6EF312D98279BA2a17916edf701175AC80555B6F",{"totalCollateralBase":1408.05016538,"totalDebtBase":910.89579719,"availableBorrowsBase":145.14182685,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.236628970926123,"updated":1743426877852,"account":"7L72m4v2Cc4fDyUSaXK7uaQon8d78MbtvuGeMenu8GSUdGnx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb3ff54b7f24950945ba8f1778f52835a7ad30a7f",{"totalCollateralBase":4086.08520383,"totalDebtBase":2504.08064736,"availableBorrowsBase":305.92014731,"currentLiquidationThreshold":75.85,"ltv":68.77,"healthFactor":1.237698007201774,"updated":1743426781669,"account":"7KATdGbGCt2GqD6VovBYYYzruU9skQx9adn8uRHEGqqtLPJU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb3ff54b7F24950945bA8f1778F52835a7aD30A7F",{"totalCollateralBase":4086.08520383,"totalDebtBase":2504.08064736,"availableBorrowsBase":305.92014731,"currentLiquidationThreshold":75.85,"ltv":68.77,"healthFactor":1.237698007201774,"updated":1743426781688,"account":"7KATdGbGCt2GqD6VovBYYYzruU9skQx9adn8uRHEGqqtLPJU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb2c882ed0aaf258ab3a0b2bc31bca319856e4d58",{"totalCollateralBase":2811.27661836,"totalDebtBase":1811.49295729,"availableBorrowsBase":296.96450648,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2415291407229891,"updated":1743426781704,"account":"7MdyNbWtnau72BWDiShxadUcztcEBSzWs9STUoyHQCgVBLWD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x65bb5eb1e8f42fec9e867ed7a2e87f2a5e31e3f5",{"totalCollateralBase":46098.21710907,"totalDebtBase":33301.86670009,"availableBorrowsBase":3576.70698717,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2458279222542163,"updated":1743426781698,"account":"7KtwnKVJe3WAXUGCvcNMTd9x6krq2Xk7DNAFTKe91buErt5s","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xbcbd7706146d4290c5235c7080add8ace95113ca",{"totalCollateralBase":865.98648137,"totalDebtBase":486.41275691,"availableBorrowsBase":33.17913191,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.246247201267713,"updated":1743426781100,"account":"7Ms2a3Qsb3SsnSjxsCy22Rinothf8rPatoVXYoB14xkQhGi1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xbCBD7706146d4290c5235C7080aDD8ACe95113cA",{"totalCollateralBase":865.98648137,"totalDebtBase":486.41275691,"availableBorrowsBase":33.17913191,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.246247201267713,"updated":1743426781679,"account":"7Ms2a3Qsb3SsnSjxsCy22Rinothf8rPatoVXYoB14xkQhGi1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa0160030ba577ad80e739b18a83382ad253f3d88",{"totalCollateralBase":3103.23315025,"totalDebtBase":1988.15337909,"availableBorrowsBase":339.2714836,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2486896364788052,"updated":1743426781673,"account":"7MDTUwHXX7jbwd8ia9oeP9aGKL2ENEbmdQJ4FXfPcsUjCJyS","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xA0160030bA577AD80e739b18a83382AD253f3d88",{"totalCollateralBase":3103.23315025,"totalDebtBase":1988.15337909,"availableBorrowsBase":339.2714836,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2486896364788052,"updated":1743426781711,"account":"7MDTUwHXX7jbwd8ia9oeP9aGKL2ENEbmdQJ4FXfPcsUjCJyS","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8eb853e3e58105d04a16fef7a29364155b6f81aa",{"totalCollateralBase":1586.85095101,"totalDebtBase":1012.26782895,"availableBorrowsBase":177.87038431,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2540957289206756,"updated":1743426781682,"account":"7Lpgqmp1uZ3V1kCv2nL8J4qVmFPnnV7sdxxFCwzwDQ4QMtkN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfc8660f1761ccc8b5aa586839e4a6dabea2a1388",{"totalCollateralBase":7254.75015633,"totalDebtBase":4622.8854909,"availableBorrowsBase":818.17712635,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2554496832085917,"updated":1743426781677,"account":"7PJfGgBdnBzoUeYoy5hj1VsaXKdjspLKL8V7vt7soAEu2zgF","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8aee4e164d5d70ac67308f303c7e063e9156903e",{"totalCollateralBase":8010.31222643,"totalDebtBase":5741.01829702,"availableBorrowsBase":667.23148412,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.255749525747398,"updated":1743426781660,"account":"7Ljigfve9PdRqvSjiRGUjVf37rbX3n89ZmaitD2hQQhtLBMN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xece792e16f847add756d2c421801ff82999d2333",{"totalCollateralBase":102059.24064452,"totalDebtBase":66787.31641307,"availableBorrowsBase":7011.72049698,"currentLiquidationThreshold":82.31,"ltv":72.31,"healthFactor":1.2577981192557781,"updated":1743426781720,"account":"7KATdGbTcEbXAegK69QTWLNj1Y8EKkr6aTgHBcyHb7eum574","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xece792e16F847aDd756D2C421801Ff82999D2333",{"totalCollateralBase":102059.24064452,"totalDebtBase":66787.31641307,"availableBorrowsBase":7011.72049698,"currentLiquidationThreshold":82.31,"ltv":72.31,"healthFactor":1.2577981192557781,"updated":1743426781723,"account":"7KATdGbTcEbXAegK69QTWLNj1Y8EKkr6aTgHBcyHb7eum574","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x40449d64ace905e8f97ffda6c75d794f96ac17c2",{"totalCollateralBase":35015.24711321,"totalDebtBase":24977.13019745,"availableBorrowsBase":3035.06749312,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2617030920993215,"updated":1743426781701,"account":"7K3pjTS11AXxZKbkR9BJ9diBfY9TyfzMzHFTuRDRScruT6Dj","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa06a0dfa4149e2cd131e81cd47adcf358f9e4ff5",{"totalCollateralBase":4065.25582274,"totalDebtBase":2546.60784105,"availableBorrowsBase":445.42044449,"currentLiquidationThreshold":79.07,"ltv":73.6,"healthFactor":1.2622272370427718,"updated":1743426781691,"account":"7MDtT9iC3zM1LpGU2QwoUTKpeVoDw89sF4vxofT6fd8np8C6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xA06a0dFa4149E2CD131E81CD47ADcF358F9e4FF5",{"totalCollateralBase":4065.25582274,"totalDebtBase":2546.60784105,"availableBorrowsBase":445.42044449,"currentLiquidationThreshold":79.07,"ltv":73.6,"healthFactor":1.2622272370427718,"updated":1743426781708,"account":"7MDtT9iC3zM1LpGU2QwoUTKpeVoDw89sF4vxofT6fd8np8C6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x56a9d2ae4e9d5ce4b31ef54ff39bb41fe41576d5",{"totalCollateralBase":51651.87982409,"totalDebtBase":29072.53397855,"availableBorrowsBase":2745.02399309,"currentLiquidationThreshold":71.06,"ltv":61.6,"healthFactor":1.26249145774773,"updated":1743426781717,"account":"7KZBrLHXs8LuhwBKuuAcsHuas7n4Ju8SNGTATNJJS7acNiTx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x56A9d2aE4e9d5cE4b31EF54fF39bb41fE41576d5",{"totalCollateralBase":51651.87982409,"totalDebtBase":29072.53397855,"availableBorrowsBase":2745.02399309,"currentLiquidationThreshold":71.06,"ltv":61.6,"healthFactor":1.26249145774773,"updated":1743426781726,"account":"7KZBrLHXs8LuhwBKuuAcsHuas7n4Ju8SNGTATNJJS7acNiTx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x05973b27fe36d10a72be2a946023420c72e470d1",{"totalCollateralBase":6874.5234226,"totalDebtBase":4334.12563679,"availableBorrowsBase":821.76693016,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2689107790039063,"updated":1743426781856,"account":"7HitT3HC79XQ7FDspXh9GshxJYfFfKrXsmTrh9eSx9UHjydY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x375eb67c78c22746db431a8827e33bfd1e9ec062",{"totalCollateralBase":662.94127423,"totalDebtBase":469.86050011,"availableBorrowsBase":60.49251927,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2698389131887395,"updated":1743426781805,"account":"7JrA47rx6r4PqNoTNSYXtCdu3F4UQW5vhDT8C3rtKdvVAfbE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x66c596f1c28f95b4dd3e5a7be13233dfe59f9aa2",{"totalCollateralBase":1280.36501817,"totalDebtBase":805.38868979,"availableBorrowsBase":154.88507384,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2717983596306495,"updated":1743426781782,"account":"7KvJsBKC6HoYAfAWPjLV5yuj5nggXnKNvWogYfWF7AvLpV2M","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc68f1d7bf374b5c68d4655e24760e38bb4a0a077",{"totalCollateralBase":12010.65784554,"totalDebtBase":7536.69467328,"availableBorrowsBase":1471.29871088,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.274899235402929,"updated":1743426781818,"account":"7N5uHEABWkiutuqoVz5v35SAKBnGsXEhLRovBmHZKDvsX16y","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc890d07efe185262cf6ac3cf407a5c9e0e11f91f",{"totalCollateralBase":1815.5867607,"totalDebtBase":1135.55780123,"availableBorrowsBase":226.1322693,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.279080119908235,"updated":1743426781839,"account":"7N8Xt4Ryt8hvf8tS72GPj17arHiZMUyFJWxcRZHq5oKNTvqP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd8c2ebc049bc98fee89a13df6aad06c97b3febb7",{"totalCollateralBase":1330.35055825,"totalDebtBase":935.19952108,"availableBorrowsBase":129.08092552,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2802781389871754,"updated":1743426781828,"account":"7NVmXp1Mc4KPtGwratR2uLcU89ywudWeem31pbLqFcu695o1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd8C2EBc049BC98FeE89A13Df6aaD06C97b3Febb7",{"totalCollateralBase":1330.35055825,"totalDebtBase":935.19952108,"availableBorrowsBase":129.08092552,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2802781389871754,"updated":1743426781788,"account":"7NVmXp1Mc4KPtGwratR2uLcU89ywudWeem31pbLqFcu695o1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfaeb3f91b59c6fa4460a5bd32219f20f40411184",{"totalCollateralBase":8081.4095887,"totalDebtBase":5025.35018098,"availableBorrowsBase":1035.70701055,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2865029178323304,"updated":1743426781831,"account":"7PGZ94jAoHbmvp4pwELWimu7gsWLTZSZut345WRKJesSKrsq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfAEB3F91B59c6Fa4460A5bd32219F20F40411184",{"totalCollateralBase":8081.4095887,"totalDebtBase":5025.35018098,"availableBorrowsBase":1035.70701055,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2865029178323304,"updated":1743426781864,"account":"7PGZ94jAoHbmvp4pwELWimu7gsWLTZSZut345WRKJesSKrsq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x76e031a692cb70e5a263b8584ce49bc2fe2e354a",{"totalCollateralBase":21424.8557695,"totalDebtBase":14914.48851688,"availableBorrowsBase":2225.39609872,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2928616472986314,"updated":1743426781115,"account":"7LHRXzocZ4hQFLw9mpMtbsVkq4a4jZTeLn1wRGdyqpU27TgE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x00037ff736d90742c08089f482b996f5bb928447",{"totalCollateralBase":3839.16659933,"totalDebtBase":2294.39823154,"availableBorrowsBase":443.6953871,"currentLiquidationThreshold":77.55,"ltv":71.32,"healthFactor":1.297627263154598,"updated":1743426781835,"account":"7HbaKoavnGFvMMna72zsZSj7eSDd8rGuq3Hv89wi2d1LH9Dt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9334f5b92da1bed85fd33a60146b453d58bbcbaf",{"totalCollateralBase":8.41124938,"totalDebtBase":5.82611114,"availableBorrowsBase":0.90288836,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2993443238708968,"updated":1743426781842,"account":"7KATdGb9doRG1PLU3Y7TgGvTyAjGEnUqy2HzqrxbCRZLp73p","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x70130536c725955faa2c212d7786c5c03ea5f842",{"totalCollateralBase":4922.10620947,"totalDebtBase":3013.81292191,"availableBorrowsBase":677.76673519,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3065459169524356,"updated":1743426781814,"account":"7L8WJGuYLpXhb6t9aSj2E3NkEL7RSDzdCDoxSETjZL2WuXks","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1ad9d16ce64de2df5b556e1c0cf58b8428e6ce66",{"totalCollateralBase":30072.26739688,"totalDebtBase":20712.74165212,"availableBorrowsBase":3345.07226538,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.306685571217938,"updated":1743426781822,"account":"7JCmEdYEruZ6eQAoLRpU6btAyEcdjFpgDiusjqhyd4cKtB7D","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80ae7d7ac8d94958ee0f13724c0aa67c0a354654",{"totalCollateralBase":1034.65176781,"totalDebtBase":552.68556218,"availableBorrowsBase":68.10549851,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3104308978386565,"updated":1743426781095,"account":"7LWHFPp9NkErAj9T7pqGC4U4pvfH2i2uqVzGAyZFDi8vWbiH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6244f66e034b30ede22115174afde52882cf0ed7",{"totalCollateralBase":3672.15655683,"totalDebtBase":2238.14571551,"availableBorrowsBase":515.97170211,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3125710382045384,"updated":1743426781825,"account":"7KpQTsrUpKgD8TVWtCgVVxfZFbmFz2Fh2DaCNsLqg2WVah7i","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x504332ffb7a77eff28f5bcc937331faff076e671",{"totalCollateralBase":12716.54025039,"totalDebtBase":7733.40454655,"availableBorrowsBase":1804.00064124,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3154920499857268,"updated":1743426781852,"account":"7KQo5W9DNkmA1D667C5Ux8Xd3ikZ5KsWAwCAdsFCnfRFsaz9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x504332FFb7A77EFF28f5Bcc937331Faff076E671",{"totalCollateralBase":12716.54025039,"totalDebtBase":7733.40454655,"availableBorrowsBase":1804.00064124,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3154920499857268,"updated":1743426781810,"account":"7KQo5W9DNkmA1D667C5Ux8Xd3ikZ5KsWAwCAdsFCnfRFsaz9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe14e33cc20a77cafbfe2ab8591b8dde9ffb1087f",{"totalCollateralBase":549.7909315,"totalDebtBase":331.43235772,"availableBorrowsBase":80.91084091,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3270663981806465,"updated":1743426781860,"account":"7NgyHnwk16FFVxCGBG2eGQUo3YHuN6MPFd6Hy6MBJuS2yWet","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10a41d7838b131eca8e6e26ca54ebb8ccbe383d0",{"totalCollateralBase":3007.80479773,"totalDebtBase":1765.09034419,"availableBorrowsBase":399.62676874,"currentLiquidationThreshold":78.2,"ltv":71.97,"healthFactor":1.332568250436711,"updated":1743426781799,"account":"7HyNoaSRGa8z56b2ExsGKhJxCSaLcWYNavUJoRGNJWTd7JWQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa8db958cf2f81ad2bcad99a30c315066f500b95d",{"totalCollateralBase":313.30022836,"totalDebtBase":186.42360313,"availableBorrowsBase":48.55156814,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3444659285724643,"updated":1743426781960,"account":"7MQxZSPthgKaeZ48f23YGgQ9GGQ1Y9ZS9Xpr2Vqkhqrxgx5m","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0453c43ab2dfb1ec81c0e40dbaa3a875f19e2236",{"totalCollateralBase":2382.32697213,"totalDebtBase":1371.69251094,"availableBorrowsBase":323.33312973,"currentLiquidationThreshold":77.43,"ltv":71.15,"healthFactor":1.3447881065238878,"updated":1743426781968,"account":"7HhEMsjWNUfLwTcTER9cBMy3BsFyBwiRvAT5oQmawPJtB7Ps","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0453C43aB2dFB1eC81C0e40DBAa3a875F19e2236",{"totalCollateralBase":2382.32697213,"totalDebtBase":1371.69251094,"availableBorrowsBase":323.33312973,"currentLiquidationThreshold":77.43,"ltv":71.15,"healthFactor":1.3447881065238878,"updated":1743426781946,"account":"7HhEMsjWNUfLwTcTER9cBMy3BsFyBwiRvAT5oQmawPJtB7Ps","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x06763e31c9266acc2ef2d09d5cf2591c3294a3dd",{"totalCollateralBase":454.41446103,"totalDebtBase":236.18770987,"availableBorrowsBase":36.46096675,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3467683093886633,"updated":1743426781776,"account":"7Hk2hSmj4Efg7QtYHA3yVRic9ZbQ5J8NCUbpN9CpBvExYL6H","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x06763e31c9266acc2EF2D09d5cf2591C3294a3dd",{"totalCollateralBase":454.41446103,"totalDebtBase":236.18770987,"availableBorrowsBase":36.46096675,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3467683093886633,"updated":1743426781849,"account":"7Hk2hSmj4Efg7QtYHA3yVRic9ZbQ5J8NCUbpN9CpBvExYL6H","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5a6bb2cfc2079dae439cd2df5fb81e997df6e2bd",{"totalCollateralBase":5766.81385336,"totalDebtBase":3262.00787272,"availableBorrowsBase":734.97080904,"currentLiquidationThreshold":76.2,"ltv":69.31,"healthFactor":1.3471188077163765,"updated":1743426781925,"account":"7Ke7b4acZ1jrTge166Uvr2iDJWCdmp5bkZhWfAisfuCzGPEp","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5A6bB2CFC2079DAE439CD2DF5fb81e997Df6e2bd",{"totalCollateralBase":5766.81385336,"totalDebtBase":3262.00787272,"availableBorrowsBase":734.97080904,"currentLiquidationThreshold":76.2,"ltv":69.31,"healthFactor":1.3471188077163765,"updated":1743426781973,"account":"7Ke7b4acZ1jrTge166Uvr2iDJWCdmp5bkZhWfAisfuCzGPEp","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xaf72a664e1021cac17ee165195f273138ee3046e",{"totalCollateralBase":122796.15152925,"totalDebtBase":72596.42085848,"availableBorrowsBase":19500.69278846,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.353192348351495,"updated":1743426781962,"account":"7MZbiuUddZVqBwkCzmfcAYVGkmDBZaAnDfa3s7ib3QXuNmSZ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9117900a3794ad6d167dd97853f82a1aa07f9bbc",{"totalCollateralBase":48584.50368253,"totalDebtBase":32298.72658374,"availableBorrowsBase":6568.87636228,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.353801153767245,"updated":1743426781954,"account":"7KATdGb9DDuVCfHCbxyJFUNxXNj4DZvEVe5kfGMKBgLn6Uyg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9117900a3794AD6D167Dd97853f82A1aA07F9BBc",{"totalCollateralBase":48584.50368253,"totalDebtBase":32298.72658374,"availableBorrowsBase":6568.87636228,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.353801153767245,"updated":1743426781957,"account":"7KATdGb9DDuVCfHCbxyJFUNxXNj4DZvEVe5kfGMKBgLn6Uyg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9845856b900dfd4e97369f6659a14f0a479bb784",{"totalCollateralBase":596.98151637,"totalDebtBase":358.60392323,"availableBorrowsBase":93.72897172,"currentLiquidationThreshold":81.54,"ltv":75.77,"healthFactor":1.357427225183456,"updated":1743426781943,"account":"7M3DDUaAFHi4i3AQosHE6kPmRRERfJdoeLVWk9Brht8BTPMk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9845856B900Dfd4e97369F6659a14F0a479BB784",{"totalCollateralBase":596.98151637,"totalDebtBase":358.60392323,"availableBorrowsBase":93.72897172,"currentLiquidationThreshold":81.54,"ltv":75.77,"healthFactor":1.357427225183456,"updated":1743426781914,"account":"7M3DDUaAFHi4i3AQosHE6kPmRRERfJdoeLVWk9Brht8BTPMk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x247c8ef2fa79b07a5e9c2e83dad14e7e6e8d2be3",{"totalCollateralBase":51468.71155035,"totalDebtBase":30204.30514097,"availableBorrowsBase":8397.22852179,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3632152452475748,"updated":1743426781934,"account":"7JRQ1aRqhkV4JEd68rHCkp2AgzfyFiotSBzZZbfainc1PehJ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x78bc8df38324d1e072bff0987d181d19e3bdf31a",{"totalCollateralBase":41.20460435,"totalDebtBase":24.16521036,"availableBorrowsBase":6.7382429,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3640966906112213,"updated":1743426781952,"account":"7LKs3VsautxTfZ2AE6oYVyQENJfGHyzwsKfgTDiHogqWM4yB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0a385a4bb1bf8ff092734e41c9386cf3288f401f",{"totalCollateralBase":74526.03256599,"totalDebtBase":38205.95368525,"availableBorrowsBase":6509.66585434,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3654474699405383,"updated":1743426781939,"account":"7HpxWDAbeb9boQRhkqEasYXfYfYG1sXYYU6EhDbu51uu5g4g","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0A385a4bb1bf8FF092734E41c9386cf3288f401F",{"totalCollateralBase":74526.03256599,"totalDebtBase":38205.95368525,"availableBorrowsBase":6509.66585434,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3654474699405383,"updated":1743426781949,"account":"7HpxWDAbeb9boQRhkqEasYXfYfYG1sXYYU6EhDbu51uu5g4g","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xbba25830fdcf5075a765f6c4583571b4f53b68e4",{"totalCollateralBase":42056.8302127,"totalDebtBase":27485.73779409,"availableBorrowsBase":6159.72637607,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3771195619703822,"updated":1743426781966,"account":"7MqaTzRJFZwDr7Hr25xhGZAzvsPQBiQnEvarj7EhEaYyq9dN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x3684116871a3890d447d79d9470e2a97eb6ea6f1",{"totalCollateralBase":5907.2011009,"totalDebtBase":3858.22087227,"availableBorrowsBase":867.54000845,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3779618033329508,"updated":1743426781971,"account":"7KATdGaq4acSFUaNgq259utpMkh4PB22jmhnfkbpigRDKGi1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6edaa7dc187523280a0c835d584db3d5e894a895",{"totalCollateralBase":2263.55383392,"totalDebtBase":1171.64335656,"availableBorrowsBase":232.89179739,"currentLiquidationThreshold":71.36,"ltv":62.05,"healthFactor":1.378637967642743,"updated":1743426781989,"account":"7L6uWMfnkzcMVSKY5rEdzbYYR2Z1p76GEXyKLraYu64vykB6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32e3589c27c9f931b938766a820f48827bc254a4",{"totalCollateralBase":1395.909189,"totalDebtBase":809.39730719,"availableBorrowsBase":237.53458456,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3797023307094554,"updated":1743426781920,"account":"7JkHDT7YEx7gpxELgmei6K55MYNGR9xA5rBep5pJppVUhh4G","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb4ff5d2aaeb2ca31c915e34afbd743dee5938b7b",{"totalCollateralBase":6070.15012367,"totalDebtBase":3506.72653929,"availableBorrowsBase":1045.88605346,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3848014792517038,"updated":1743426781981,"account":"7MgsmEi4YbhPCfSdSAqFyVMnNR8GhKS6cZgHeGWU2iK9SXDF","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb4Ff5d2AAeB2CA31C915E34Afbd743dEE5938B7b",{"totalCollateralBase":6070.15012367,"totalDebtBase":3506.72653929,"availableBorrowsBase":1045.88605346,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3848014792517038,"updated":1743426782016,"account":"7MgsmEi4YbhPCfSdSAqFyVMnNR8GhKS6cZgHeGWU2iK9SXDF","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5009e192ec169788c9c1f0202fe7c2bc79405ff8",{"totalCollateralBase":61898.42315423,"totalDebtBase":32248.60689496,"availableBorrowsBase":6895.95590778,"currentLiquidationThreshold":72.16,"ltv":63.24,"healthFactor":1.38504904393469,"updated":1743426781930,"account":"7KQW3wEr1vwkWeaYNhMEwQqsyQnoH7JZqT9UBKLsAS6bY5cy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf2de6aee2c7a3a4371b5f32bdd86192f7f16bd28",{"totalCollateralBase":1928.9776374,"totalDebtBase":1107.3995179,"availableBorrowsBase":339.33371015,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3935188565427494,"updated":1743426782022,"account":"7P5zwnCcbzozXZxSaTEvbsTpbUzQqjm7WFfykPvnh4p6Eixt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x329b4791f71da7fd9454ed3c8b63ec472c3bbb66",{"totalCollateralBase":2755.55055021,"totalDebtBase":1571.29521913,"availableBorrowsBase":495.36769353,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4029447893251799,"updated":1743426782056,"account":"7JjuomnuogtYAqYk47zRTqTkyYDYEgvxzBmvqeHgepo1F7pT","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2c4270dad627d74ff6e0fe99092d3b9302eb44b3",{"totalCollateralBase":20534.60362735,"totalDebtBase":11693.78762861,"availableBorrowsBase":3707.1650919,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4048213823969284,"updated":1743426782061,"account":"7Jbb8TFohE9xbBmbk2KuQijj1zJaHr1DBdMM2YBytACQfaGX","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x99d281d96f086bcd179cde61da873b0b1e480fa2",{"totalCollateralBase":8306.73672826,"totalDebtBase":4135.83251112,"availableBorrowsBase":848.20952584,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.405935973989757,"updated":1743426782031,"account":"7KATdGbAxgaHFKKJXZLG28fQpPP7VzDJtWmAErZdP7aEhGBc","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x99D281D96F086bcd179cdE61da873b0B1e480fa2",{"totalCollateralBase":8306.73672826,"totalDebtBase":4135.83251112,"availableBorrowsBase":848.20952584,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.405935973989757,"updated":1743426782082,"account":"7KATdGbAxgaHFKKJXZLG28fQpPP7VzDJtWmAErZdP7aEhGBc","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x765d9f39bc1e25da32df40a28ec1f5aed693632b",{"totalCollateralBase":526.06463666,"totalDebtBase":261.37608986,"availableBorrowsBase":54.26269214,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.40887120110046,"updated":1743426782035,"account":"7LGkkJ3eiLVHRsbAMs9cWGuVFoN57hxiFVW2xPdAWGh4Li4q","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x765d9f39BC1e25DA32dF40a28Ec1F5aed693632B",{"totalCollateralBase":526.06463666,"totalDebtBase":261.37608986,"availableBorrowsBase":54.26269214,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.40887120110046,"updated":1743426782122,"account":"7LGkkJ3eiLVHRsbAMs9cWGuVFoN57hxiFVW2xPdAWGh4Li4q","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x76a497415fc75a15a2014b49e2d53bf748c30a8f",{"totalCollateralBase":4482.20067397,"totalDebtBase":2537.45116258,"availableBorrowsBase":824.1993429,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4131347992266823,"updated":1743426782110,"account":"7KATdGb3uqkncqvKgMPuKDdizM1HpFCa9gDppUUWekawS6CN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x76A497415fC75A15a2014B49e2d53bf748c30A8F",{"totalCollateralBase":4482.20067397,"totalDebtBase":2537.45116258,"availableBorrowsBase":824.1993429,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4131347992266823,"updated":1743426782086,"account":"7KATdGb3uqkncqvKgMPuKDdizM1HpFCa9gDppUUWekawS6CN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6680ae2f9f36ee543fa21bba92613238aea28f86",{"totalCollateralBase":775.09967443,"totalDebtBase":436.95505176,"availableBorrowsBase":144.36970406,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4190927351506677,"updated":1743426782043,"account":"7KuxPuBhx4sajeC54xuPayv2bGnrdxui97RFaUr3MaYkETNn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe64afe6914886cdcfea8da5f13e1e21aa11876cf",{"totalCollateralBase":3372.41260304,"totalDebtBase":1646.54292866,"availableBorrowsBase":376.90463316,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.4337244301618002,"updated":1743426782071,"account":"7NoWaQ1pua9UXtRLyus4F3MpnH4UsCBN9sP9FeD4Ntv9FfKw","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfc89997e9e97b2c5466a6095a9b44b18ce255e4c",{"totalCollateralBase":376.82785703,"totalDebtBase":209.42832944,"availableBorrowsBase":73.19256333,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4394532316907356,"updated":1743426781070,"account":"7PJgEAtRFHarM74kH19mwfhrGrBrMQV6dCg3NxcAMtc419DZ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9e289d693888d1bf3bcfa508f827240c56ba2e28",{"totalCollateralBase":129715.85292789,"totalDebtBase":72062.92620766,"availableBorrowsBase":25223.96348826,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4400287055132015,"updated":1743426782215,"account":"7MAvv6YQeXULbpNAKWceqA6voTLoioDzm71ggvWzstyPDepm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9E289D693888d1BF3bcFa508f827240c56bA2E28",{"totalCollateralBase":129715.85292789,"totalDebtBase":72062.92620766,"availableBorrowsBase":25223.96348826,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4400287055132015,"updated":1743426782066,"account":"7MAvv6YQeXULbpNAKWceqA6voTLoioDzm71ggvWzstyPDepm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe806cb404a70f85dd540d2c7df5f7da8318e757c",{"totalCollateralBase":21001.64303089,"totalDebtBase":11630.61752748,"availableBorrowsBase":4120.61474569,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4445762991529076,"updated":1743426782115,"account":"7NqnQtwX1NPjdDxrnCdJyyEjQWe73gA1Q826d9mzzhEVGCYn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe806Cb404A70F85dD540d2C7DF5F7da8318e757C",{"totalCollateralBase":21001.64303089,"totalDebtBase":11630.61752748,"availableBorrowsBase":4120.61474569,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4445762991529076,"updated":1743426782050,"account":"7NqnQtwX1NPjdDxrnCdJyyEjQWe73gA1Q826d9mzzhEVGCYn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb8d7fb4ddfc6f4d9721c0bb74beb504340f117c5",{"totalCollateralBase":715.24160615,"totalDebtBase":395.4992866,"availableBorrowsBase":140.93191801,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4467618635648887,"updated":1743426782127,"account":"7MmvFogJHHHhtaio9nFivwXZQx7HajwGTZGeQbjM6vrJC4Hr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x38d4950cdea6241265a1c863a96bb7d9effbcbc7",{"totalCollateralBase":1459.21014574,"totalDebtBase":804.90850214,"availableBorrowsBase":289.49910717,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4503115739072618,"updated":1743426782077,"account":"7Jt57joQj8QMu5rqLwo58YmZcREpEKgr1BgktnzhJezdvcAt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xae70debed84304554c2909745a07d44b8ba4b6de",{"totalCollateralBase":434994.88393035,"totalDebtBase":229090.6938799,"availableBorrowsBase":74187.73919634,"currentLiquidationThreshold":76.48,"ltv":69.72,"healthFactor":1.4521938084675692,"updated":1743426782091,"account":"7MYH9TjQccMqw5VJnhpUrjMjd8TMBCKet1jftikHMMJsj5M4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xAe70debEd84304554c2909745A07d44B8Ba4b6de",{"totalCollateralBase":434994.88393035,"totalDebtBase":229090.6938799,"availableBorrowsBase":74187.73919634,"currentLiquidationThreshold":76.48,"ltv":69.72,"healthFactor":1.4521938084675692,"updated":1743426782097,"account":"7MYH9TjQccMqw5VJnhpUrjMjd8TMBCKet1jftikHMMJsj5M4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x52e3ce919fd6e1cff805c0afa2eef21f72232cff",{"totalCollateralBase":4049.68473724,"totalDebtBase":2229.43139935,"availableBorrowsBase":807.83215358,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4531722262163178,"updated":1743426782102,"account":"7KUEtGGWW5xNYa8mvLLK1ayoezVukAjMgG28neWtV5EPuCCe","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x52e3ce919Fd6E1CFf805c0AfA2eEf21f72232cff",{"totalCollateralBase":4049.68473724,"totalDebtBase":2229.43139935,"availableBorrowsBase":807.83215358,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4531722262163178,"updated":1743426782133,"account":"7KUEtGGWW5xNYa8mvLLK1ayoezVukAjMgG28neWtV5EPuCCe","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x40516ae940349430d92f5b15e392cdc22c53758a",{"totalCollateralBase":21895.71452948,"totalDebtBase":12006.23206159,"availableBorrowsBase":4415.55383552,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4589566096776125,"updated":1743426782130,"account":"7K3tY3VH1GqRMmVM3GHos91fPVBfFur2eWUMf6F2kNGUmsmQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x40516AE940349430D92f5B15E392cdC22C53758a",{"totalCollateralBase":21895.71452948,"totalDebtBase":12006.23206159,"availableBorrowsBase":4415.55383552,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4589566096776125,"updated":1743426782175,"account":"7K3tY3VH1GqRMmVM3GHos91fPVBfFur2eWUMf6F2kNGUmsmQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10efd89d53f43e9afb0445d600c0091d9113b0fe",{"totalCollateralBase":188.06227995,"totalDebtBase":102.6739185,"availableBorrowsBase":38.37279146,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4653168609708804,"updated":1743426782185,"account":"7KATdGahXrJwhsBUj9QSGzYe8GYMmkoKRGdDu2A2ayq6jiZC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10efd89D53F43e9afb0445d600c0091d9113B0Fe",{"totalCollateralBase":188.06227995,"totalDebtBase":102.6739185,"availableBorrowsBase":38.37279146,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4653168609708804,"updated":1743426782218,"account":"7KATdGahXrJwhsBUj9QSGzYe8GYMmkoKRGdDu2A2ayq6jiZC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xba796be5c16cde3238ec2c7dcae2ed2d82143d1e",{"totalCollateralBase":942.20817401,"totalDebtBase":512.41249936,"availableBorrowsBase":194.24363115,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4710151297079008,"updated":1743426782237,"account":"7Mp4G8GqW2YKXYRELYs8wW9rZSE5siiwDhjpdX4e4NpEMwMs","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x17B55f38f703Ef6c97CB3782dbcDDfa3F2F28b00",{"totalCollateralBase":36981.94368892,"totalDebtBase":20028.97126757,"availableBorrowsBase":7707.48649912,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4771380195169377,"updated":1743426782246,"account":"7J8eGM5r5EVRUurMAWN6ZvMLsGi5GrwY4V2iZKfW26m3WzP8","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x3261ec20e922ad196f633c259d26ec394c8a9d29",{"totalCollateralBase":89172.5263339,"totalDebtBase":45760.70961841,"availableBorrowsBase":15732.66454145,"currentLiquidationThreshold":75.97,"ltv":68.96,"healthFactor":1.4804046707485006,"updated":1743426782193,"account":"7JjcmXnhVWjAFyD2MVz7CCyGinoVThhLCKsgZYtXmoy9ATs6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc8762ce2ce36743d05c3bca15193913e8cfd5716",{"totalCollateralBase":355.79597879,"totalDebtBase":190.93391813,"availableBorrowsBase":75.91306596,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4907607083001413,"updated":1743426782208,"account":"7N8Py5Xkuo4SxZNYNYn8HHYA46bbUVYtYAVmvfFVgagH1UfQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xcafcf9b276c0927c9f0098e90be3f7f3c9768949",{"totalCollateralBase":4967.36830025,"totalDebtBase":2887.33356038,"availableBorrowsBase":922.14118908,"currentLiquidationThreshold":86.69,"ltv":76.69,"healthFactor":1.49141465280626,"updated":1743426782202,"account":"7NBi7Be1NpFU3Li3A5t2Hbsxdz2AEAB4scrQkgsHY5f8yXEk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xCAFCf9B276c0927C9F0098e90BE3f7f3c9768949",{"totalCollateralBase":4967.36830025,"totalDebtBase":2887.33356038,"availableBorrowsBase":922.14118908,"currentLiquidationThreshold":86.69,"ltv":76.69,"healthFactor":1.49141465280626,"updated":1743426782189,"account":"7NBi7Be1NpFU3Li3A5t2Hbsxdz2AEAB4scrQkgsHY5f8yXEk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80d8ec3368f9c2ad259b0051967ef2d85c245007",{"totalCollateralBase":8469.36449658,"totalDebtBase":4533.99350062,"availableBorrowsBase":1818.02987182,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4943761159634408,"updated":1743426782435,"account":"7LWVrVKqnock474KHKxEKg43FR7MTBJUrtTXadUq1eQUo851","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80d8EC3368F9c2ad259B0051967Ef2D85C245007",{"totalCollateralBase":8469.36449658,"totalDebtBase":4533.99350062,"availableBorrowsBase":1818.02987182,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4943761159634408,"updated":1743426782241,"account":"7LWVrVKqnock474KHKxEKg43FR7MTBJUrtTXadUq1eQUo851","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x689af88ce2eaa5c73c29d248d3bcb0d9163679a9",{"totalCollateralBase":1608.0212692,"totalDebtBase":801.90365258,"availableBorrowsBase":274.9881914,"currentLiquidationThreshold":74.65,"ltv":66.97,"healthFactor":1.4969228205881582,"updated":1743426782211,"account":"7KxiJRsksomESPYdYoMQWQ9r2cuVcDoLsp8SV4YAy61zFfWP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf420d6be126b4516eb729191d38527a15e2024c9",{"totalCollateralBase":8443.88717177,"totalDebtBase":4510.5081677,"availableBorrowsBase":1822.40721113,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.497638289582029,"updated":1743426782180,"account":"7P7eiy3SqzQe51PDDx4WeQkHhnH1orNoXsJ8RVi6WQEK8Fm4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6e17e05d80b1d365f3789e5e1bdee6979df35f89",{"totalCollateralBase":245531.5209444,"totalDebtBase":116817.55260848,"availableBorrowsBase":36664.20113386,"currentLiquidationThreshold":71.67,"ltv":62.51,"healthFactor":1.5063869866425865,"updated":1743426782221,"account":"7L5uePUoBCZW9Fb63cne34vDVwgcBSuNeuyhwFDgPKs1usRr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x52b9deeb5e8bbca3cd6887b5dc3dc9aa56a5670e",{"totalCollateralBase":98026.09020809,"totalDebtBase":49412.81013737,"availableBorrowsBase":15715.72419688,"currentLiquidationThreshold":76.44,"ltv":66.44,"healthFactor":1.5164315315552344,"updated":1743426782227,"account":"7KU2RinqWYLbdMYXoN9aHCf5fd44M8RCLe36nxnNB1PGvtiG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x52b9DEeB5e8bbcA3cD6887B5DC3Dc9Aa56A5670e",{"totalCollateralBase":98026.09020809,"totalDebtBase":49412.81013737,"availableBorrowsBase":15715.72419688,"currentLiquidationThreshold":76.44,"ltv":66.44,"healthFactor":1.5164315315552344,"updated":1743426782196,"account":"7KU2RinqWYLbdMYXoN9aHCf5fd44M8RCLe36nxnNB1PGvtiG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xcad3acc5fdc054877afde6ccebb1b176cd4acc92",{"totalCollateralBase":61585.86878119,"totalDebtBase":28175.03713324,"availableBorrowsBase":8776.48413547,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5300816798558923,"updated":1743426782205,"account":"7NBVqbNgut84x2KDwi7A1nqJJLkxMY9B6FLhfDnkoxSQALcq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfd315d09bf4e3fb044abad86b3e433116f812b52",{"totalCollateralBase":1213.79881429,"totalDebtBase":554.12738287,"availableBorrowsBase":174.1519057,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5333282495431788,"updated":1743426782224,"account":"7KATdGbWsXmFhGYQUqvyHZXRt25z24iHMk1oFW69FkH6ZcMr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd43c7e59fc0417deebdc50ec4afb4a80b37b5dab",{"totalCollateralBase":1550.14383947,"totalDebtBase":805.76362281,"availableBorrowsBase":356.84425679,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5390556690251833,"updated":1743426782235,"account":"7NPqQaBFeES16emvahBN7VGwmdmwRgs93uaqobzvX3dYKD2u","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x72aae1fa2cc19a1f367740bb49b93c96135d4e01",{"totalCollateralBase":8.28008207,"totalDebtBase":3.75994243,"availableBorrowsBase":1.20810681,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5415282435587718,"updated":1743426782199,"account":"7LBuWMfmdg4d824RRxvJe3U9GsYyKfATWivKC9bjAaC2MoLn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfcacbd0797bc102db06b206788f7c96909dbb214",{"totalCollateralBase":661.59945113,"totalDebtBase":343.19851504,"availableBorrowsBase":153.00107331,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5421965355482734,"updated":1743426781245,"account":"7PJrfbgoMXPveNVxB6SJxhErKCusQizEeQitpgC57Ugm9nPH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf8bd2aa8ee55c107cc630aa30121921892cf1eff",{"totalCollateralBase":2849.84810838,"totalDebtBase":1465.24427302,"availableBorrowsBase":672.14180827,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5559716073832288,"updated":1743426782285,"account":"7PDhMYNLR8tEmq5EwG3TNEn2zd9DmRxCf6rgsGNU3fnuxa8z","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x582c50704848d821b1d54d9551429cc0033e24ac",{"totalCollateralBase":1580.90770771,"totalDebtBase":809.92215336,"availableBorrowsBase":375.75862742,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5615404035106637,"updated":1743426782302,"account":"7KbAfRT7KrU3dnoyrnSsNymHhFaXARXyTU5NxPmS9SG2ka52","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xcc8800567968f958e319dcef16a123aae0039da3",{"totalCollateralBase":4.17490472,"totalDebtBase":2.4009285,"availableBorrowsBase":0.93899528,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.5649838177188533,"updated":1743426782328,"account":"7NDjTKfuJzvPdi25rJvNGvcG8nuJVavYznRfpAW4aUQzUnM3","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x51bc3dd10a9ee4946f961038cf5d50057e49eb77",{"totalCollateralBase":25952.55585205,"totalDebtBase":11593.40906734,"availableBorrowsBase":3978.12444389,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5669928483432873,"updated":1743426782312,"account":"7KATdGavWuztuT7tK9JsYU7iCboszXysXHcTH2ZLTfmFtnEY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x764fb00d7e2353592c6eac16f748a61e838b42b8",{"totalCollateralBase":5860.96361384,"totalDebtBase":2762.48575551,"availableBorrowsBase":1139.15772222,"currentLiquidationThreshold":74.44,"ltv":66.57,"healthFactor":1.579338936114998,"updated":1743426782331,"account":"7LGgcDJGLJgsZZ4u9UEwLiD384DKC8YJKzoPKmMgTpVD5mt1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x764fB00D7E2353592C6EAc16f748a61e838B42B8",{"totalCollateralBase":5860.96361384,"totalDebtBase":2762.48575551,"availableBorrowsBase":1139.15772222,"currentLiquidationThreshold":74.44,"ltv":66.57,"healthFactor":1.579338936114998,"updated":1743426782293,"account":"7LGgcDJGLJgsZZ4u9UEwLiD384DKC8YJKzoPKmMgTpVD5mt1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2e337289b8bf3b198026bf941b4faecb7c5f4a3c",{"totalCollateralBase":339.78886959,"totalDebtBase":150.40334683,"availableBorrowsBase":53.46997492,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5814289623411302,"updated":1743426782305,"account":"7Je8mgF5rFnaZZXRyy8iyz3vTRRbNeZy8LQc7Aa8czsjaf5j","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfa6bec1c59118bb967eacc2d3f52b46779d20a63",{"totalCollateralBase":101.10939149,"totalDebtBase":50.79246988,"availableBorrowsBase":25.03957374,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5925099405699543,"updated":1743426782298,"account":"7PFuKHiTzHFsuwq6GPPTc5eUMG9BZgzL5LPRAhi1iCYwxgSt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd8037d45361fdd690134b4d70e8e40c83177f81e",{"totalCollateralBase":149.4997829,"totalDebtBase":74.84168893,"availableBorrowsBase":37.28314825,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.598037511310877,"updated":1743426782353,"account":"7NUnfXEsQw3fd6GETmkGEsaGzzqXv8GTBSKh5FkkUFqS2jrg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb27b9088b217981687f9be39a8406ff5df1cdc03",{"totalCollateralBase":2.0221569,"totalDebtBase":1.00910535,"availableBorrowsBase":0.50751233,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6031284741479173,"updated":1743426782322,"account":"7MdaWqFKKL7hYuADd7hpJyUJjYh3FUrPo6r5i25mDVwfLPpQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6a8d03f86ea4e4edf130f06f6ffb95b298d73490",{"totalCollateralBase":3228.34120807,"totalDebtBase":1606.15465794,"availableBorrowsBase":815.10124811,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6079852296244308,"updated":1743426782309,"account":"7L1GFYBvnnbttnzQ2paGip7fXBCtZkvZiNHcXYztn2hZLxrk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32c32ce4d9e9fe317d47f9c351c56d43d388c97f",{"totalCollateralBase":404114.18416185,"totalDebtBase":200955.01017743,"availableBorrowsBase":102130.62794396,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.608774755324762,"updated":1743426782339,"account":"7Jk7fAL2t1DuKK34HSbfDHX5UmAGxUxrLDSbZQYfaxaW2nXt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd214457d2b23e06f78961fa7cb4d5ec3b7f38dc4",{"totalCollateralBase":505.06774088,"totalDebtBase":250.84460592,"availableBorrowsBase":127.95619974,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6107748907658872,"updated":1743426782336,"account":"7NM1N1NYb5rTH6XuGjiFDqAuFijVrUk4jS9yV4T2Y54ej3Yj","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb7280Cf3579cF33d66060426846b95499c9547a3",{"totalCollateralBase":807.9245504,"totalDebtBase":400.65040922,"availableBorrowsBase":205.29300358,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.613225958207097,"updated":1743426782334,"account":"7MjhwojTcVmgv4CtUrkKJ8MrDCnUADDCyBWNF4MEMcFYxC7J","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6268e9dc4432c3df9cc1171c3acf1b468fa6f4a2",{"totalCollateralBase":0.9971066,"totalDebtBase":0.55512727,"availableBorrowsBase":0.24255801,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.6165589199031782,"updated":1743426782345,"account":"7Kpb9JC2nqAa26kUcU1F6DLn65nVHxYHwxLhLzJDmeLwpigL","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6268e9dc4432C3dF9cC1171C3Acf1b468fa6f4a2",{"totalCollateralBase":0.9971066,"totalDebtBase":0.55512727,"availableBorrowsBase":0.24255801,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.6165589199031782,"updated":1743426782316,"account":"7Kpb9JC2nqAa26kUcU1F6DLn65nVHxYHwxLhLzJDmeLwpigL","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6ec2723c5983e60bbfabe3364f2a38812410fc17",{"totalCollateralBase":2323.37231944,"totalDebtBase":1004.66582864,"availableBorrowsBase":389.35756302,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.6188075450038728,"updated":1743426782319,"account":"7L6nKEk1LhqkPuXPich1AEH5JEdsiGTniSvAgDdhw1WKbaxC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5226e4dc206a573da19cba5f94592151e00d74c2",{"totalCollateralBase":95139.8738383,"totalDebtBase":52796.98805068,"availableBorrowsBase":23314.91101996,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.621794909442134,"updated":1743426782356,"account":"7KTGmNRKYRE831TrCLxtchusmaU5bVmLraWzyYZZVpdmGcP7","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1f8a1052999d8ab419c2e99d6d9633e16bb7ab30",{"totalCollateralBase":1569.86631547,"totalDebtBase":673.14551393,"availableBorrowsBase":268.77427535,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.6324946064251342,"updated":1743426782325,"account":"7KATdGakTZ8umRY358X8UozzSBocsEPAGo1V2K1J31RvCHSB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xbec33a3ae810bb2466593ce1b15edf83f45a17d1",{"totalCollateralBase":519.20987729,"totalDebtBase":254.02190095,"availableBorrowsBase":135.38550702,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6351657092423628,"updated":1743426782350,"account":"7MugNtAi3EkfkuBbcDrHGjzma64fm5FfeNreCafBJPjVCTTx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x288e0dbd476cbfc7dfc1268c00b9e5081e9d9b1a",{"totalCollateralBase":2251.84979639,"totalDebtBase":1106.45095276,"availableBorrowsBase":601.80230278,"currentLiquidationThreshold":80.75,"ltv":75.86,"healthFactor":1.6434245964939955,"updated":1743426782376,"account":"7JWjQ85wyc4qPG5GJ4yEzKj3NrTsvnHrePxEgRcoAkggTuYq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x70e255c2d405b147d179abc79e0c2633dbd52417",{"totalCollateralBase":4112.86515515,"totalDebtBase":1998.10652355,"availableBorrowsBase":1086.54234281,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6467050606862526,"updated":1743426782428,"account":"7L9ZtDtjkmnwRF6ivpNRiVTutzUkRxoymP1YAJ3d1e6yDsaD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x765750bc1f316505a55e190eeb9efaa4cc5f9855",{"totalCollateralBase":7961.39221482,"totalDebtBase":3864.71651846,"availableBorrowsBase":2106.32764266,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6480157707396206,"updated":1743426782475,"account":"7LGisdkXvvD9zFdjeBDnrC5h3MsbCFEqwYFVqt1n5w1Fhrze","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd736b3431f5e5195fd039f1a0a6ad900045b7718",{"totalCollateralBase":469.10106665,"totalDebtBase":219.99023976,"availableBorrowsBase":114.71337129,"currentLiquidationThreshold":77.56,"ltv":71.35,"healthFactor":1.65386786107842,"updated":1743426782401,"account":"7KATdGbPG9jNnyMYzVbekSsJR2LkuyqTb1gNgtEN1vBq8AAY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xD736b3431f5e5195FD039f1A0A6aD900045B7718",{"totalCollateralBase":469.10106665,"totalDebtBase":219.99023976,"availableBorrowsBase":114.71337129,"currentLiquidationThreshold":77.56,"ltv":71.35,"healthFactor":1.65386786107842,"updated":1743426782406,"account":"7KATdGbPG9jNnyMYzVbekSsJR2LkuyqTb1gNgtEN1vBq8AAY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2e6a0c8fd2d9212f742b416484fbbea0c8bc2824",{"totalCollateralBase":6268.80296109,"totalDebtBase":2974.17598522,"availableBorrowsBase":1727.4262356,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.686195569391983,"updated":1743426782417,"account":"7JeQzSbHDHzQeNR9ZoB9byStp1vxnQJV88SdmZCEfC1BUN42","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2E6a0c8Fd2D9212f742b416484fBBeA0C8bC2824",{"totalCollateralBase":6268.80296109,"totalDebtBase":2974.17598522,"availableBorrowsBase":1727.4262356,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.686195569391983,"updated":1743426782411,"account":"7JeQzSbHDHzQeNR9ZoB9byStp1vxnQJV88SdmZCEfC1BUN42","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1237936b529a53060201af397ca61e820f3398fa",{"totalCollateralBase":4938.28816198,"totalDebtBase":2291.98972912,"availableBorrowsBase":1327.77549361,"currentLiquidationThreshold":78.86,"ltv":73.3,"healthFactor":1.6991062372845858,"updated":1743426782422,"account":"7J1Sf3WUwEN7R3QCdKYwmnABJj6Ypg128khg9pFymYQrKLwk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1237936b529A53060201af397ca61E820F3398fA",{"totalCollateralBase":4938.28816198,"totalDebtBase":2291.98972912,"availableBorrowsBase":1327.77549361,"currentLiquidationThreshold":78.86,"ltv":73.3,"healthFactor":1.6991062372845858,"updated":1743426782446,"account":"7J1Sf3WUwEN7R3QCdKYwmnABJj6Ypg128khg9pFymYQrKLwk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0ed8d20a8e7583feb61e79b1046dfdb6a4331804",{"totalCollateralBase":539.63656751,"totalDebtBase":250.95576257,"availableBorrowsBase":153.77166306,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7202603741349902,"updated":1743426782451,"account":"7Hw2N7cjdredGhb2jgfYZUyANHDoxm8RyvCXHj7sddSxVHKa","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xca442cec2bb03517d991b36056e45368990f6b67",{"totalCollateralBase":197.60320323,"totalDebtBase":90.34438233,"availableBorrowsBase":57.85802009,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7497774460682396,"updated":1743426782511,"account":"7NAmDA1GcpQxxdGAJf6dBzDRbey3rMPtjuLEARWareap4Acg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd089bd98d28ff9df3ac58d7a05f0d50e0eecf43b",{"totalCollateralBase":6308.7682122,"totalDebtBase":2876.17320015,"availableBorrowsBase":1855.402959,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7547672614072007,"updated":1743426782441,"account":"7KATdGbMvZzfQUmyRNtc33SzoqwXS5W5K5L6GEDjNv3z7CiD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xD089Bd98D28fF9df3Ac58D7A05F0D50e0EECF43B",{"totalCollateralBase":6308.7682122,"totalDebtBase":2876.17320015,"availableBorrowsBase":1855.402959,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7547672614072007,"updated":1743426782485,"account":"7KATdGbMvZzfQUmyRNtc33SzoqwXS5W5K5L6GEDjNv3z7CiD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7043b5b5e169031a473222dbdaae8abf06d3b181",{"totalCollateralBase":11.15703643,"totalDebtBase":5.06125266,"availableBorrowsBase":3.30652466,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7635217483886687,"updated":1743426782497,"account":"7L8kmB5Vjgc5DVWMVsNqHwwoeZbP1HE4Y3FGeCH5Z4jpQp1R","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x908fdface285549efcc6bc7cd221f8a27ec1dc9b",{"totalCollateralBase":89.75296398,"totalDebtBase":35.593021,"availableBorrowsBase":18.25875739,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.7651515107413895,"updated":1743426782470,"account":"7Ls6vKf1M8ZXUMNnsKuGVqFgo111qhxdsgahiFgxjAXGY8aa","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x589a2b25775ea0a4db41fb9975b288efbe4da7e8",{"totalCollateralBase":2228.28593967,"totalDebtBase":1008.93372005,"availableBorrowsBase":662.2807347,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7668442597514313,"updated":1743426782480,"account":"7KbjJAidH76pvZ5ZKRRdKCM7hjkaMPEPg57hxJZYPjfYNHCm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6230d0ffbfdcbb99b63db96d2bac5cf395f5e63b",{"totalCollateralBase":15282.69755812,"totalDebtBase":6896.32138322,"availableBorrowsBase":3798.51036795,"currentLiquidationThreshold":79.98,"ltv":69.98,"healthFactor":1.7724089159651146,"updated":1743426782490,"account":"7KpJUmQTxJfw8Hg5autn2oHvaj8v5QP2KyCFE8S2SZU84H5H","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6230D0ffBfdcbb99b63Db96d2BaC5Cf395F5E63B",{"totalCollateralBase":15282.69755812,"totalDebtBase":6896.32138322,"availableBorrowsBase":3798.51036795,"currentLiquidationThreshold":79.98,"ltv":69.98,"healthFactor":1.7724089159651146,"updated":1743426782466,"account":"7KpJUmQTxJfw8Hg5autn2oHvaj8v5QP2KyCFE8S2SZU84H5H","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xba99240fa3a03fdf8ee4bc08a43d33e009e37a02",{"totalCollateralBase":5742.0619537,"totalDebtBase":2588.72989642,"availableBorrowsBase":1717.81656886,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7744800526747262,"updated":1743426782461,"account":"7KATdGbHXbLXjMThV8W5nxWB9Ep8YHudAsyDLQHtZ3ZbMfj4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xBA99240Fa3A03Fdf8eE4Bc08A43D33e009E37A02",{"totalCollateralBase":5742.0619537,"totalDebtBase":2588.72989642,"availableBorrowsBase":1717.81656886,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7744800526747262,"updated":1743426782505,"account":"7KATdGbHXbLXjMThV8W5nxWB9Ep8YHudAsyDLQHtZ3ZbMfj4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x22f239b2cb78cb0e64526e8ac9e5c71e68336a60",{"totalCollateralBase":1998.83021117,"totalDebtBase":900.56411739,"availableBorrowsBase":598.55854099,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7756250088826337,"updated":1743426782602,"account":"7JPNsNrPrj1czSoT9bqgGHwzVb7juW2PQYN2BrjXRje6P1wc","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x22f239b2CB78cB0e64526e8ac9E5C71E68336A60",{"totalCollateralBase":1998.83021117,"totalDebtBase":900.56411739,"availableBorrowsBase":598.55854099,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7756250088826337,"updated":1743426782592,"account":"7JPNsNrPrj1czSoT9bqgGHwzVb7juW2PQYN2BrjXRje6P1wc","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfab6735cfadd7b3fbb4a1383571629d576801d9c",{"totalCollateralBase":11666.09500996,"totalDebtBase":4566.28770201,"availableBorrowsBase":2433.36930397,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.7883819504792378,"updated":1743426782568,"account":"7KATdGbWNi2CM8RmucyRuyrzNt3QyihMQeV2X3i15KbsZkdx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5a05ce8780158e99b9ffdb16eaee342464383066",{"totalCollateralBase":4892.09386478,"totalDebtBase":2162.19905794,"availableBorrowsBase":1506.87134065,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8100438428405803,"updated":1743426782585,"account":"7KdbKWUKY3T7joiGaebggUML5P5KZBLxuUt17yag4AZ1wyNc","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5a05CE8780158e99B9fFdB16EaEe342464383066",{"totalCollateralBase":4892.09386478,"totalDebtBase":2162.19905794,"availableBorrowsBase":1506.87134065,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8100438428405803,"updated":1743426782624,"account":"7KdbKWUKY3T7joiGaebggUML5P5KZBLxuUt17yag4AZ1wyNc","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2e125d12f06a9264d4481505bba2bf0381d08ddd",{"totalCollateralBase":17218.79241838,"totalDebtBase":7557.61028573,"availableBorrowsBase":5356.48402806,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.822670581560617,"updated":1743426782577,"account":"7Jdxwf1EtHvFNafmQpLbd4Fgqi4y8LycFw3qU7oNV5vAx6zU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2e125D12f06a9264d4481505bbA2Bf0381D08DdD",{"totalCollateralBase":17218.79241838,"totalDebtBase":7557.61028573,"availableBorrowsBase":5356.48402806,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.822670581560617,"updated":1743426782634,"account":"7Jdxwf1EtHvFNafmQpLbd4Fgqi4y8LycFw3qU7oNV5vAx6zU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x028056ad2ddee5730946613e0450f1f5041fbd5f",{"totalCollateralBase":1151.3592721,"totalDebtBase":500.80922606,"availableBorrowsBase":362.71022802,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.839198181164594,"updated":1743426782581,"account":"7HeqWHApnrDGH6MpnE4qADygBwQFYAE6JPJBRusJu9BRJD1z","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9e5a1e58511c1c937cb741638e211f8b63864dba",{"totalCollateralBase":281.86201979,"totalDebtBase":136.31085732,"availableBorrowsBase":89.17875851,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.8610096275344885,"updated":1743426782597,"account":"7MBBd2NDGKLV7jzgRrQis6o8ShwAQH4gWYzjNcizgaLdQcef","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x39b812e2751f3a5a32e08b51f73c95c09d22dd26",{"totalCollateralBase":7098.28412297,"totalDebtBase":3048.90232057,"availableBorrowsBase":2274.81077166,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8625153256199976,"updated":1743426782641,"account":"7JuEhKuQJNikoS8UdushXb4tE4f8fBV8SQvyYfmzMjC7LBCD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xab1878639b6f8fffe801e005e5dbfe683ecdbe1a",{"totalCollateralBase":14479.22920832,"totalDebtBase":6185.10196994,"availableBorrowsBase":4674.3199363,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.872787776653643,"updated":1743426782562,"account":"7MTtk33siM1LNFE1wHjMRhhMeZytoDb6R3PWvNsUnb3B7tDA","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xAb1878639b6F8FffE801E005E5dbFe683EcdbE1A",{"totalCollateralBase":14479.22920832,"totalDebtBase":6185.10196994,"availableBorrowsBase":4674.3199363,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.872787776653643,"updated":1743426782572,"account":"7MTtk33siM1LNFE1wHjMRhhMeZytoDb6R3PWvNsUnb3B7tDA","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32406ea80e9b35a6da619b3c93ded9d6065735f5",{"totalCollateralBase":1215.07750061,"totalDebtBase":517.52076512,"availableBorrowsBase":393.78736034,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8783053087050592,"updated":1743426782627,"account":"7JjSpWYd4GYpbusx1JapctncaRNxzuN1Utesj5oWJdT5PiYE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa416e3003795ae8cf97b8b40e5900c33eb4a82ab",{"totalCollateralBase":6059.42602081,"totalDebtBase":2576.45918696,"availableBorrowsBase":1968.11032865,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8814739395773936,"updated":1743426782613,"account":"7MJhwJzhA2nrkZWAQkyz2aVF9dGtis5rT4s8kqbNPYoCcBMk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8214586aeeb0be6c48ca46d07aa2568df8f394b5",{"totalCollateralBase":3586.57386079,"totalDebtBase":1523.1732671,"availableBorrowsBase":1179.66879439,"currentLiquidationThreshold":80.73,"ltv":75.36,"healthFactor":1.9009269269363478,"updated":1743426782609,"account":"7LY7Z6Xj1PB1BtF8QpdkBvK7dYDz1UMJwX1tCHPXh5qpAszG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2eeb7ff95ebdd23ebcfbcfeea301df0583b7e4b7",{"totalCollateralBase":970.48507632,"totalDebtBase":404.17553949,"availableBorrowsBase":323.68826775,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9209179804390641,"updated":1743426782616,"account":"7Jf5SpyKeMadVAKhULpZJkFGrmEXi6JExvJdMdBZsZ2qZJ2e","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x26ddba9021ef148aeed472f171e431b9fec2a4bf",{"totalCollateralBase":19416.09235367,"totalDebtBase":7094.21778386,"availableBorrowsBase":4669.99257323,"currentLiquidationThreshold":70.39,"ltv":60.59,"healthFactor":1.9264967363764414,"updated":1743426782605,"account":"7JUWyLa55nJ29e6ypn15TDG62k1Jt41G638dLRiHTquM9JNE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf03dcbda983b1f95e91802b525eeb51eae975e12",{"totalCollateralBase":26026.16708594,"totalDebtBase":10037.35299306,"availableBorrowsBase":7163.34083404,"currentLiquidationThreshold":74.54,"ltv":66.09,"healthFactor":1.932771016349971,"updated":1743426782630,"account":"7P2Z8nPQwfDSWJGsTE1RhdRNgNm3vGGt1Pi3YGub8LNVSBJr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xF03dCbdA983b1f95e91802b525EeB51eae975e12",{"totalCollateralBase":26026.16708594,"totalDebtBase":10037.35299306,"availableBorrowsBase":7163.34083404,"currentLiquidationThreshold":74.54,"ltv":66.09,"healthFactor":1.932771016349971,"updated":1743426782620,"account":"7P2Z8nPQwfDSWJGsTE1RhdRNgNm3vGGt1Pi3YGub8LNVSBJr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x72c7df8b90146d5ecbb62ac78c68a35fa47c85e2",{"totalCollateralBase":43190.8978817,"totalDebtBase":19241.65672565,"availableBorrowsBase":14563.85904636,"currentLiquidationThreshold":86.54,"ltv":78.27,"healthFactor":1.9425251972713051,"updated":1743426782637,"account":"7LC47rMsqUGmbdT5xBNMAwEu77kGmKBdezwEmAiGmuqkcuyK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xde0419f94107c4dcf4b8fc79b0a97bb39c755022",{"totalCollateralBase":80814.24153429,"totalDebtBase":32744.35478117,"availableBorrowsBase":27866.32636955,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9744286812030418,"updated":1743426782696,"account":"7Ncf8jSz3XarDx8t2SXmnaURW81jHrEoXgH971sCanAb2V9n","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xde0419f94107c4DCF4b8Fc79b0A97bB39C755022",{"totalCollateralBase":80814.24153429,"totalDebtBase":32744.35478117,"availableBorrowsBase":27866.32636955,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9744286812030418,"updated":1743426782722,"account":"7Ncf8jSz3XarDx8t2SXmnaURW81jHrEoXgH971sCanAb2V9n","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7fe0e0bc5bd476181d53ce0bffbc8dc3a35d2ed9",{"totalCollateralBase":5725.63546133,"totalDebtBase":2309.49861103,"availableBorrowsBase":1984.72798497,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9833345416116814,"updated":1743426782740,"account":"7LVEAn29nTJMKChtr9HV2cdB6QqjZ1k4shv95YjDKh4cmMwU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x4e276ab655f6e4444c44e2953ba31ebb011e53e5",{"totalCollateralBase":5.02558501,"totalDebtBase":2.02018357,"availableBorrowsBase":1.74900519,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9901498406899725,"updated":1743426782687,"account":"7KN2jGb1Q65iKWPboApK3GPcUV1ycaHseiYtKv1QEsfmNhuh","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xceba54427482d5deaf3d4e2560d37d90c8b5ffc0",{"totalCollateralBase":84474.72919074,"totalDebtBase":29644.53265295,"availableBorrowsBase":21099.43717193,"currentLiquidationThreshold":70.05,"ltv":60.07,"healthFactor":1.996136977123888,"updated":1743426782725,"account":"7NGcW1GjgDKZ2DH47Ue1fSYX2qmD13gHQG5KmqtErLCXoG5E","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xceBa54427482d5DeaF3d4e2560d37d90c8B5fFc0",{"totalCollateralBase":84474.72919074,"totalDebtBase":29644.53265295,"availableBorrowsBase":21099.43717193,"currentLiquidationThreshold":70.05,"ltv":60.07,"healthFactor":1.996136977123888,"updated":1743426782701,"account":"7NGcW1GjgDKZ2DH47Ue1fSYX2qmD13gHQG5KmqtErLCXoG5E","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}]]}"#;

	let bytes = TEST_DATA.as_bytes();
	let data = String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");
	let data = data.as_str();
	let data = serde_json::from_str::<pallet_liquidation::BorrowerData<hydradx_runtime::AccountId>>(&data);
	let data = data.unwrap();
	pub fn process_borrowers_data(oracle_data: BorrowerData<hydradx_runtime::AccountId>) -> Vec<(H160, BorrowerDataDetails<hydradx_runtime::AccountId>)> {
		let mut borrowers = oracle_data.borrowers.clone();
		// remove elements with HF == 1
		borrowers.retain(|b| b.1.health_factor > 0.0 && b.1.health_factor < 1.0);
		borrowers.sort_by(|a, b| a.1.health_factor.partial_cmp(&b.1.health_factor).unwrap_or(Ordering::Equal));
		borrowers.truncate(borrowers.len().min(MAX_LIQUIDATIONS as usize));
		borrowers
	}
	process_borrowers_data(data)
}

pub fn calculate_liquidation_amount(
	money_market: &MoneyMarketData,
	user_address: EvmAddress,
	debt_asset_address: EvmAddress,
	collateral_asset_address: EvmAddress,
	current_timestamp: u64,
	caller: EvmAddress,
) -> Option<U256> {
	let user_data = UserData::new(money_market, user_address);

	// Find the indices of debt and collateral assets
	let mut debt_asset_index = None;
	let mut collateral_asset_index = None;

	for (i, reserve) in money_market.reserves.iter().enumerate() {
		if reserve.asset_address == debt_asset_address {
			debt_asset_index = Some(i);
		}
		if reserve.asset_address == collateral_asset_address {
			collateral_asset_index = Some(i);
		}
	}

	let debt_asset_index = debt_asset_index?;
	let collateral_asset_index = collateral_asset_index?;

	// Ensure the user has debt in the debt asset and collateral in the collateral asset
	if !user_data.is_debt(debt_asset_index) || !user_data.is_collateral(collateral_asset_index) {
		return None;
	}

	// Get the debt and collateral reserves
	let debt_reserve = &money_market.reserves[debt_asset_index];
	let collateral_reserve = &money_market.reserves[collateral_asset_index];

	// Get asset prices
	let debt_price = get_asset_price(money_market.oracle_contract, debt_asset_address, caller);
	let collateral_price = get_asset_price(money_market.oracle_contract, collateral_asset_address, caller);

	// Calculate total collateral and debt in base currency
	let mut total_collateral = U256::zero();
	let mut total_debt = U256::zero();
	let mut weighted_liquidation_threshold = U256::zero();

	for reserve in money_market.reserves.iter() {
		let price = get_asset_price(money_market.oracle_contract, reserve.asset_address, caller);
		let partial_collateral = reserve.get_user_collateral_in_base_currency(user_address, current_timestamp, price)?;

		if !partial_collateral.is_zero() {
			weighted_liquidation_threshold = weighted_liquidation_threshold
				.checked_add(partial_collateral.checked_mul(U256::from(reserve.reserve_data.liquidation_threshold()))?)?;
			total_collateral = total_collateral.checked_add(partial_collateral)?;
		}

		let partial_debt = reserve.get_user_debt_in_base_currency(user_address, current_timestamp, price)?;
		total_debt = total_debt.checked_add(partial_debt)?;
	}

	if total_collateral.is_zero() || total_debt.is_zero() {
		return None;
	}

	let avg_liquidation_threshold = weighted_liquidation_threshold.checked_div(total_collateral)?;

	// Calculate current health factor
	let current_hf = user_data.health_factor(money_market, current_timestamp, caller)?;

	// If health factor is already >= 1, no liquidation needed
	if current_hf >= U256::from(1_000_000_000_000_000_000u128) {
		return Some(U256::zero());
	}

	// Calculate the user's debt in the specific debt asset
	let user_debt_in_debt_asset = debt_reserve.get_user_debt_in_base_currency(user_address, current_timestamp, debt_price)?;

	// Calculate the user's collateral in the specific collateral asset
	let user_collateral_in_collateral_asset = collateral_reserve.get_user_collateral_in_base_currency(
		user_address,
		current_timestamp,
		collateral_price
	)?;

	// Calculate the liquidation bonus (close factor)
	let close_factor = U256::from(5000); // 50% in basis points (typical Aave value)

	// Calculate the maximum debt amount that can be liquidated (based on close factor)
	let max_debt_to_liquidate = user_debt_in_debt_asset
		.checked_mul(close_factor)?
		.checked_div(U256::from(10000))?; // Convert from basis points

	// Calculate the amount of debt to liquidate to restore HF to 1
	// Formula: debt_to_liquidate = (total_debt - (total_collateral * avg_liquidation_threshold / 10^18)) / (1 - (collateral_price * liquidation_threshold / debt_price))

	let collateral_liquidation_threshold = U256::from(collateral_reserve.reserve_data.liquidation_threshold());
	let numerator = total_debt.checked_sub(
		percent_mul(total_collateral, avg_liquidation_threshold)?
			.checked_div(U256::from(1_000_000_000_000_000_000u128))?
	)?;

	let liquidation_bonus = U256::from(10500); // 105% in basis points (typical Aave value)
	let price_ratio = collateral_price
		.checked_mul(liquidation_bonus)?
		.checked_div(U256::from(10000))?
		.checked_mul(collateral_liquidation_threshold)?
		.checked_div(debt_price)?
		.checked_div(U256::from(10000))?; // Convert from basis points

	let denominator = U256::from(1_000_000_000_000_000_000u128)
		.checked_sub(price_ratio)?;

	let debt_to_liquidate = numerator
		.checked_mul(U256::from(1_000_000_000_000_000_000u128))?
		.checked_div(denominator)?;

	// Ensure we don't liquidate more than the max allowed by close factor
	let debt_to_liquidate = std::cmp::min(debt_to_liquidate, max_debt_to_liquidate);

	// Convert from base currency to debt asset units
	let debt_decimals = U256::from(10u128.pow(debt_reserve.get_decimals() as u32));
	let debt_amount = debt_to_liquidate
		.checked_mul(debt_decimals)?
		.checked_div(debt_price)?;

	// Ensure we don't liquidate more than the user's total debt in this asset
	let total_user_debt = balance_of(debt_reserve.get_collateral_and_debt_addresses().1.1, user_address);
	let debt_amount = std::cmp::min(debt_amount, total_user_debt);

	Some(debt_amount)
}

