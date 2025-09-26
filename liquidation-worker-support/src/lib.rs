// This file is part of Hydration node.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use codec::{Decode, Encode};
use ethabi::ethereum_types::U512;
use evm::ExitReason;
use fp_evm::{ExitReason::Succeed, ExitSucceed::Returned};
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::{Block as BlockT, CheckedConversion},
	Deserialize,
};
use hydradx_traits::evm::EvmAddress;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_arithmetic::ArithmeticError;
use sp_core::{RuntimeDebug, H160, H256, U256};
use sp_std::{boxed::Box, ops::BitAnd, vec::Vec};
use std::marker::PhantomData;
use xcm_runtime_apis::dry_run::{CallDryRunEffects, Error as XcmDryRunApiError};

pub type Balance = u128;
pub type AssetId = u32;
pub type CallResult = (ExitReason, Vec<u8>);

#[derive(RuntimeDebug)]
pub enum LiquidationError {
	DispatchError(DispatchError),
	EvmError(ExitReason),
	EthAbiError(ethabi::Error),
	ReserveNotFound,
	CurrentBlockNotAvailable,
	ApiError(sp_api::ApiError),
}

impl From<ArithmeticError> for LiquidationError {
	fn from(e: ArithmeticError) -> Self {
		Self::DispatchError(DispatchError::Arithmetic(e))
	}
}

impl From<primitive_types::Error> for LiquidationError {
	fn from(_e: primitive_types::Error) -> Self {
		Self::DispatchError(DispatchError::Arithmetic(ArithmeticError::Overflow))
	}
}

impl From<DispatchError> for LiquidationError {
	fn from(e: DispatchError) -> Self {
		Self::DispatchError(e)
	}
}

impl From<ExitReason> for LiquidationError {
	fn from(e: ExitReason) -> Self {
		Self::EvmError(e)
	}
}

impl From<ethabi::Error> for LiquidationError {
	fn from(e: ethabi::Error) -> Self {
		Self::EthAbiError(e)
	}
}

/// Default percentage of borrower's debt to be repaid in a liquidation.
/// Percentage applied when the users health factor is above `CLOSE_FACTOR_HF_THRESHOLD`
/// Expressed in bps, a value of 0.5e4 results in 50.00%
const DEFAULT_LIQUIDATION_CLOSE_FACTOR: u128 = 5_000u128;

/// Maximum percentage of borrower's debt to be repaid in a liquidation
/// Percentage applied when the users health factor is below `CLOSE_FACTOR_HF_THRESHOLD`
/// Expressed in bps, a value of 1e4 results in 100.00%
const MAX_LIQUIDATION_CLOSE_FACTOR: u128 = 10_000u128;

/// This constant represents below which health factor value it is possible to liquidate
/// an amount of debt corresponding to `MAX_LIQUIDATION_CLOSE_FACTOR`.
/// A value of 0.95e18 results in 0.95
const CLOSE_FACTOR_HF_THRESHOLD: u128 = 950_000_000_000_000_000u128;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	GetPool = "getPool()",
	GetPriceOracle = "getPriceOracle()",
	GetAssetPrice = "getAssetPrice(address)",
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
	SetValue = "setValue(string,uint128,uint128)",
	SetMultipleValues = "setMultipleValues(string[],uint256[])",
	GetValue = "getValue(string)",
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
	Symbol = "symbol()",
}

#[derive(Clone, Encode, Decode, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BorrowerDataDetails<AccountId> {
	pub total_collateral_base: f32,
	pub total_debt_base: f32,
	pub available_borrows_base: f32,
	pub current_liquidation_threshold: f32,
	pub ltv: f32,
	pub health_factor: f32,
	pub updated: u64,
	pub account: AccountId,
	pub pool: H160,
}

#[derive(Clone, Encode, Decode, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BorrowersData<AccountId> {
	pub last_global_update: u32,
	pub last_update: u32,
	pub borrowers: Vec<(H160, BorrowerDataDetails<AccountId>)>,
}

#[derive(Clone, Debug)]
pub struct Borrower {
	pub user_address: EvmAddress,
	pub health_factor: U256,
}

/// Multiplies two ray, rounding half up to the nearest ray.
pub fn ray_mul(a: U256, b: U256) -> Result<U256, LiquidationError> {
	if a.is_zero() || b.is_zero() {
		return Ok(U256::zero());
	}

	let ray = U512::from(10u128.pow(27));

	let res512 = a
		.full_mul(b)
		.checked_add(ray / 2)
		.and_then(|r| r.checked_div(ray))
		.ok_or(ArithmeticError::Overflow)?;
	res512.try_into().map_err(|_| ArithmeticError::Overflow.into())
}

/// Divides two wad, rounding half up to the nearest wad.
pub fn wad_div(a: U256, b: U256) -> Result<U256, LiquidationError> {
	if a.is_zero() {
		return Ok(U256::zero());
	}
	if b.is_zero() {
		return Err(ArithmeticError::DivisionByZero.into());
	}

	let wad = U256::from(10u128.pow(18));
	let nominator = a.full_mul(wad).checked_add(U512::from(b / 2));
	let res = nominator
		.and_then(|n| n.checked_div(U512::from(b)))
		.ok_or(ArithmeticError::DivisionByZero)?;
	res.try_into().map_err(|_| ArithmeticError::Overflow.into())
}

pub trait RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>
where
	Block: BlockT,
{
	fn current_timestamp(&self, hash: Block::Hash) -> Option<u64>;
	fn call(
		&self,
		hash: Block::Hash,
		caller: EvmAddress,
		mm_pool: EvmAddress,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<Result<fp_evm::ExecutionInfoV2<Vec<u8>>, DispatchError>, sp_api::ApiError>;
	fn address_to_asset(&self, hash: Block::Hash, address: EvmAddress) -> Result<Option<AssetId>, sp_api::ApiError>;
	fn dry_run_call(
		&self,
		hash: Block::Hash,
		origin: OriginCaller,
		call: RuntimeCall,
	) -> Result<Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError>, sp_api::ApiError>;
}

/// Executes a percentage multiplication.
/// Params:
///     value: The value of which the percentage needs to be calculated
///     percentage: The percentage of the value to be calculated, in basis points.
pub fn percent_mul(value: U256, percentage: U256) -> Result<U256, LiquidationError> {
	if percentage.is_zero() {
		return Ok(U256::zero());
	}

	let percentage_factor = U512::from(10u128.pow(4));
	let half_percentage_factor = percentage_factor / 2;
	let nominator = value.full_mul(percentage).checked_add(half_percentage_factor);
	let res: U512 = nominator
		.and_then(|n| n.checked_div(percentage_factor))
		.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;
	res.try_into().map_err(|_| ArithmeticError::Overflow.into())
}

/// Collateral and debt amounts of a reserve in the base currency.
#[derive(Default, Eq, PartialEq, Clone, RuntimeDebug)]
pub struct UserReserve {
	pub collateral: U256,
	pub debt: U256,
}

/// The configuration of the user across all the reserves.
/// Bitmap of the users collaterals and borrows. It is divided into pairs of bits, one pair per asset.
/// The first bit indicates if the user uses an asset as collateral, the second whether the user borrows an asset.
/// The corresponding assets are in the same position as `fetch_reserves_list()`.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
struct UserConfiguration(U256);
impl UserConfiguration {
	/// Returns `true` if the user uses the asset as collateral.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_collateral(&self, asset_index: usize) -> bool {
		let bit_mask = U256::from(2 << (2 * asset_index));
		!(self.0 & bit_mask).is_zero()
	}

	/// Returns `true` if the user uses the asset as debt.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_debt(&self, asset_index: usize) -> bool {
		let bit_mask = U256::from(1 << (2 * asset_index));
		!(self.0 & bit_mask).is_zero()
	}
}

/// User's data. The state is not automatically updated. Any change in the chain can invalidate the data stored in the struct.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct UserData {
	address: EvmAddress,
	configuration: UserConfiguration,
	reserves: Vec<UserReserve>, // the order of reserves is given by fetch_reserves_list()
}
impl UserData {
	/// Calls Runtime API.
	pub fn new<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
		api_provider: ApiProvider,
		hash: Block::Hash,
		money_market: &MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
		address: H160,
		current_evm_timestamp: u64,
		caller: EvmAddress,
	) -> Result<Self, LiquidationError>
	where
		Block: BlockT,
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	{
		let configuration = UserConfiguration(Self::fetch_user_configuration::<
			Block,
			ApiProvider,
			OriginCaller,
			RuntimeCall,
			RuntimeEvent,
		>(&api_provider, hash, money_market.pool_contract, address, caller)?);

		let mut reserves = Vec::new();
		for (index, reserve) in money_market.reserves.iter().enumerate() {
			// skip the computation if the reserve is not used as user's collateral or debt
			let (collateral, debt) = if configuration.is_collateral(index) || configuration.is_debt(index) {
				let c = reserve
					.get_user_collateral_in_base_currency::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
						&api_provider,
						hash,
						address,
						current_evm_timestamp,
						caller,
					)
					.unwrap_or_default();
				let d = reserve
					.get_user_debt_in_base_currency::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
						&api_provider,
						hash,
						address,
						current_evm_timestamp,
						caller,
					)
					.unwrap_or_default();
				(c, d)
			} else {
				(U256::zero(), U256::zero())
			};
			reserves.push(UserReserve { collateral, debt });
		}

		Ok(Self {
			address,
			configuration,
			reserves,
		})
	}

	/// Get the user's address.
	pub fn address(&self) -> EvmAddress {
		self.address
	}

	/// Get user's reserves.
	pub fn reserves(&self) -> &Vec<UserReserve> {
		&self.reserves
	}

	/// Returns `true` if the user uses the asset as collateral.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_collateral(&self, asset_index: usize) -> bool {
		self.configuration.is_collateral(asset_index)
	}

	/// Returns `true` if the user uses the asset as debt.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_debt(&self, asset_index: usize) -> bool {
		self.configuration.is_debt(asset_index)
	}

	/// Returns the amount of collateral.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn collateral(&self, asset_index: usize) -> U256 {
		if let Some(collateral) = self.reserves.get(asset_index) {
			collateral.collateral
		} else {
			U256::zero()
		}
	}

	/// Returns the amount of debt.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn debt(&self, asset_index: usize) -> U256 {
		if let Some(debt) = self.reserves.get(asset_index) {
			debt.debt
		} else {
			U256::zero()
		}
	}

	/// Returns non-zero collateral assets of the user.
	pub fn collateral_assets<Block, OriginCaller, RuntimeCall, RuntimeEvent>(
		&self,
		money_market: &MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	) -> Vec<(usize, EvmAddress)>
	where
		Block: BlockT,
	{
		let mut assets = Vec::new();
		for (index, reserve) in money_market.reserves.iter().enumerate() {
			if self.is_collateral(index) && !self.collateral(index).is_zero() {
				assets.push((index, reserve.asset_address()));
			}
		}

		assets
	}

	/// Returns non-zero debt assets of the user.
	pub fn debt_assets<Block, OriginCaller, RuntimeCall, RuntimeEvent>(
		&self,
		money_market: &MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	) -> Vec<(usize, EvmAddress)>
	where
		Block: BlockT,
	{
		let mut assets = Vec::new();
		for (index, reserve) in money_market.reserves.iter().enumerate() {
			if self.is_debt(index) && !self.debt(index).is_zero() {
				assets.push((index, reserve.asset_address()));
			}
		}

		assets
	}

	/// Update the state of reserve.
	/// Calling `health_factor()` with updated reserved calculates the updated health factor.
	pub fn update_reserves(&mut self, reserves: Vec<(usize, UserReserve)>) {
		let len = self.reserves.len();

		for (i, reserve) in reserves {
			if i < len {
				self.reserves[i] = reserve;
			}
		}
	}

	/// Calculates user's health factor.
	pub fn health_factor<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
		&self,
		money_market: &MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	) -> Result<U256, LiquidationError>
	where
		Block: BlockT,
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	{
		let mut avg_liquidation_threshold = U256::zero();
		let mut total_collateral = U256::zero();
		let mut total_debt = U256::zero();

		for (i, reserve) in money_market.reserves.iter().enumerate() {
			let user_reserve = if let Some(maybe_reserve) = self.reserves.get(i) {
				maybe_reserve
			} else {
				&Default::default()
			};

			let partial_collateral = user_reserve.collateral;

			avg_liquidation_threshold = avg_liquidation_threshold
				.checked_add(
					partial_collateral
						.checked_mul(U256::from(reserve.liquidation_threshold()))
						.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?,
				)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			total_collateral = total_collateral
				.checked_add(partial_collateral)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			total_debt = total_debt
				.checked_add(user_reserve.debt)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;
		}

		avg_liquidation_threshold = avg_liquidation_threshold
			.checked_div(total_collateral)
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

		let nominator = percent_mul(total_collateral, avg_liquidation_threshold)?;
		wad_div(nominator, total_debt)
	}

	/// Returns Bitmap of the users collaterals and borrows.
	/// It is divided into pairs of bits, one pair per asset.
	/// The first bit indicates if an asset is used as collateral by the user, the second whether an asset is borrowed by the user.
	/// The corresponding assets are in the same position as `getReservesList()`.
	/// Calls Runtime API.
	pub fn fetch_user_configuration<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		mm_pool: EvmAddress,
		user: EvmAddress,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError>
	where
		Block: BlockT,
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	{
		let mut data = Into::<u32>::into(Function::GetUserConfiguration).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(user).as_bytes());

		let gas_limit = U256::from(500_000);
		let call_info = ApiProvider::call(api_provider, hash, caller, mm_pool, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			Ok(U256::checked_from(&call_info.value[0..32])
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?)
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}
}

trait BalanceOf<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>
where
	Block: BlockT,
	ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
{
	fn fetch_scaled_balance_of(
		self,
		api_provider: &ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError>;
	fn fetch_balance_of(
		self,
		api_provider: &ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError>;
}
impl<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>
	BalanceOf<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent> for EvmAddress
where
	Block: BlockT,
	ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
{
	/// Calls Runtime API.
	fn fetch_scaled_balance_of(
		self,
		api_provider: &ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError> {
		let mut data = Into::<u32>::into(Function::ScaledBalanceOf).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(user).as_bytes());

		let gas_limit = U256::from(500_000);
		let call_info = ApiProvider::call(api_provider, hash, caller, self, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			Ok(U256::checked_from(&call_info.value[0..32])
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?)
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	/// Calls Runtime API.
	fn fetch_balance_of(
		self,
		api_provider: &ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError> {
		let mut data = Into::<u32>::into(Function::BalanceOf).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(user).as_bytes());

		let gas_limit = U256::from(500_000);
		let call_info = ApiProvider::call(api_provider, hash, caller, self, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			Ok(U256::checked_from(&call_info.value[0..32])
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?)
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}
}

/// Configuration of the reserve.
/// https://github.com/aave/aave-v3-core/blob/782f51917056a53a2c228701058a6c3fb233684a/contracts/protocol/libraries/types/DataTypes.sol#L5
/// Not all data fields are used.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct ReserveData {
	configuration: U256, // https://github.com/aave-dao/aave-v3-origin/blob/3aad8ca184159732e4b3d8c82cd56a8707a106a2/src/core/contracts/protocol/libraries/types/DataTypes.sol#L79
	liquidity_index: u128,
	current_liquidity_rate: u128,
	variable_borrow_index: u128,
	current_variable_borrow_rate: u128,
	last_update_timestamp: u64,
	a_token_address: H160,
	stable_debt_token_address: H160,
	variable_debt_token_address: H160,
}

impl ReserveData {
	pub fn new(data: &[ethabi::Token]) -> Option<Self> {
		Some(Self {
			#[allow(clippy::get_first)]
			configuration: data.get(0)?.clone().into_uint()?,
			liquidity_index: data.get(1)?.clone().into_uint()?.try_into().ok()?,
			current_liquidity_rate: data.get(2)?.clone().into_uint()?.try_into().ok()?,
			variable_borrow_index: data.get(3)?.clone().into_uint()?.try_into().ok()?,
			current_variable_borrow_rate: data.get(4)?.clone().into_uint()?.try_into().ok()?,
			last_update_timestamp: data.get(6)?.clone().into_uint()?.try_into().ok()?,
			a_token_address: data.get(8)?.clone().into_address()?,
			stable_debt_token_address: data.get(9)?.clone().into_address()?,
			variable_debt_token_address: data.get(10)?.clone().into_address()?,
		})
	}
}

/// State of asset reserve.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Reserve {
	reserve_data: ReserveData,
	asset_address: EvmAddress,
	symbol: Vec<u8>,
	price: U256,
}
impl Reserve {
	/// Get the price of the reserve.
	pub fn price(&self) -> U256 {
		self.price
	}

	/// Get the asset symbol of the reserve.
	pub fn symbol(&self) -> &Vec<u8> {
		&self.symbol
	}

	/// Get the address of the reserve.
	pub fn asset_address(&self) -> EvmAddress {
		self.asset_address
	}

	/// Get addresses of collateral and debt assets.
	pub fn get_collateral_and_debt_addresses(&self) -> (EvmAddress, (EvmAddress, EvmAddress)) {
		(
			self.reserve_data.a_token_address,
			(
				self.reserve_data.stable_debt_token_address,
				self.reserve_data.variable_debt_token_address,
			),
		)
	}

	fn get_normalized_income(&self, current_timestamp: u64) -> Result<U256, LiquidationError> {
		if current_timestamp == self.reserve_data.last_update_timestamp {
			Ok(U256::from(self.reserve_data.liquidity_index))
		} else {
			let current_liquidity_rate = U256::from(self.reserve_data.current_liquidity_rate);

			let timestamp_diff = U256::from(
				current_timestamp
					.checked_sub(self.reserve_data.last_update_timestamp)
					.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?,
			);

			let nominator: U256 = current_liquidity_rate
				.checked_mul(timestamp_diff)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			let seconds_per_year = U256::from(365 * 24 * 60 * 60);
			let ray = U256::from(10u128.pow(27));

			let linear_interest = nominator
				.checked_div(seconds_per_year)
				.and_then(|r| r.checked_add(ray))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			ray_mul(linear_interest, self.reserve_data.liquidity_index.into())
		}
	}

	/// Get liquidation threshold of the reserve.
	pub fn liquidation_threshold(&self) -> u128 {
		// bits [16..31]
		let res = self.reserve_data.configuration.low_u32() >> 16;
		res.into()
	}

	/// Get the number of decimals of the reserve.
	pub fn decimals(&self) -> u8 {
		let config = self.reserve_data.configuration;
		// bits [48..55]
		let res = config >> 48;
		res.byte(0)
	}

	/// Get liquidation bonus of the reserve.
	pub fn liquidation_bonus(&self) -> U256 {
		// bits [32..47]
		let shifted_config = self.reserve_data.configuration >> 32;

		let bit_mask: u32 = 0b0000_0000_0000_0000_1111_1111_1111_1111;
		shifted_config.low_u32().bitand(bit_mask).into()
	}

	/// Get user's collateral in base currency.
	/// Calls Runtime API.
	pub fn get_user_collateral_in_base_currency<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
		&self,
		api_provider: &ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		current_timestamp: u64,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError>
	where
		Block: BlockT,
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	{
		let (collateral_address, _) = self.get_collateral_and_debt_addresses();

		let scaled_balance =
			BalanceOf::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_scaled_balance_of(
				collateral_address,
				api_provider,
				hash,
				user,
				caller,
			)?;

		let normalized_income = self.get_normalized_income(current_timestamp)?;

		ray_mul(scaled_balance, normalized_income)?
			.full_mul(self.price)
			.checked_div(U512::from(10u128.pow(self.decimals() as u32)))
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow.into())
	}

	fn get_normalized_debt(&self, current_timestamp: u64) -> Result<U256, LiquidationError> {
		let variable_borrow_index = U256::from(self.reserve_data.variable_borrow_index);
		if current_timestamp == self.reserve_data.last_update_timestamp {
			Ok(variable_borrow_index)
		} else {
			let exp = U256::from(
				current_timestamp
					.checked_sub(self.reserve_data.last_update_timestamp)
					.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?,
			);

			let ray = U256::from(10u128.pow(27));

			if exp.is_zero() {
				return Ok(ray);
			}

			let exp_minus_one = exp
				.checked_sub(U256::from(1))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			let exp_minus_two = if exp > U256::from(2) {
				exp.checked_sub(U256::from(2))
					.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			} else {
				U256::zero()
			};

			let seconds_per_year = U256::from(365 * 24 * 60 * 60);

			let rate = U256::from(self.reserve_data.current_variable_borrow_rate);

			let base_power_two = ray_mul(rate, rate)?
				.checked_div(seconds_per_year * seconds_per_year)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			let base_power_three = ray_mul(base_power_two, rate)?
				.checked_div(seconds_per_year)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			let second_term = exp
				.full_mul(exp_minus_one)
				.checked_mul(base_power_two.into())
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
				/ 2;

			let third_term = exp
				.full_mul(exp_minus_one)
				.checked_mul(exp_minus_two.into())
				.and_then(|r| r.checked_mul(base_power_three.into()))
				.ok_or(ArithmeticError::Overflow)?
				/ 6;

			let compound_interest = rate
				.full_mul(exp)
				.checked_div(seconds_per_year.into())
				.and_then(|r| r.checked_add(ray.into()))
				.and_then(|r| r.checked_add(second_term))
				.and_then(|r| r.checked_add(third_term))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
				.try_into()
				.map_err(|_| ArithmeticError::Overflow)?;

			ray_mul(compound_interest, variable_borrow_index)
		}
	}

	/// Get user's debt in base currency.
	/// Calls Runtime API.
	pub fn get_user_debt_in_base_currency<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(
		&self,
		api_provider: &ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		current_timestamp: u64,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError>
	where
		Block: BlockT,
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	{
		let (_, (stable_debt_address, variable_debt_address)) = self.get_collateral_and_debt_addresses();

		let mut total_debt =
			BalanceOf::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_scaled_balance_of(
				variable_debt_address,
				api_provider,
				hash,
				user,
				caller,
			)?;

		if !total_debt.is_zero() {
			let normalized_debt = self.get_normalized_debt(current_timestamp)?;
			total_debt = ray_mul(total_debt, normalized_debt)?;
		}

		total_debt = total_debt
			.checked_add(
				BalanceOf::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_balance_of(
					stable_debt_address,
					api_provider,
					hash,
					user,
					caller,
				)?,
			)
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

		total_debt
			.full_mul(self.price)
			.checked_div(U512::from(10u128.pow(self.decimals() as u32)))
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow.into())
	}
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct LiquidationAmounts {
	pub debt_amount: U256,
	pub collateral_amount: U256,
	pub debt_in_base_currency: U256,
	pub collateral_in_base_currency: U256,
}

/// Captures the state of the money market related to liquidations.
/// The state is not automatically updated. Any change in the chain can invalidate the data stored in the struct.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>
where
	Block: BlockT,
{
	pap_contract: EvmAddress, // PoolAddressesProvider
	pool_contract: EvmAddress,
	oracle_contract: EvmAddress,
	reserves: Vec<Reserve>, // the order of reserves is given by fetch_reserves_list()
	pub caller: EvmAddress,
	_phantom: PhantomData<(Block, OriginCaller, RuntimeCall, RuntimeEvent)>,
}
impl<Block: BlockT, OriginCaller, RuntimeCall, RuntimeEvent>
	MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>
{
	/// Calls Runtime API.
	pub fn new<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: ApiProvider,
		hash: Block::Hash,
		pap_contract: EvmAddress,
		caller: EvmAddress,
	) -> Result<Self, LiquidationError> {
		let pool_contract = Self::fetch_pool(&api_provider, hash, pap_contract, caller)?;
		let oracle_contract = Self::fetch_price_oracle(&api_provider, hash, pap_contract, caller)?;

		let mut reserves = Vec::new();
		for asset_address in Self::fetch_reserves_list(&api_provider, hash, pool_contract, caller)?.into_iter() {
			let reserve_data = Self::fetch_reserve_data(&api_provider, hash, pool_contract, asset_address, caller)?;
			let symbol = Self::fetch_asset_symbol(&api_provider, hash, &asset_address, caller)?;
			let price = Self::fetch_asset_price(&api_provider, hash, oracle_contract, asset_address, caller)?;
			reserves.push(Reserve {
				reserve_data,
				asset_address,
				symbol,
				price,
			});
		}

		Ok(Self {
			pap_contract,
			pool_contract,
			oracle_contract,
			reserves,
			caller,
			_phantom: PhantomData,
		})
	}

	/// Get oracle address.
	pub fn oracle_contract(&self) -> EvmAddress {
		self.oracle_contract
	}

	/// Get the list of the reserves.
	pub fn reserves(&self) -> &Vec<Reserve> {
		&self.reserves
	}

	/// Calls Runtime API.
	pub fn fetch_pool<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		pap_contract: EvmAddress,
		caller: EvmAddress,
	) -> Result<EvmAddress, LiquidationError> {
		let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();
		let gas_limit = U256::from(100_000);
		let call_info = ApiProvider::call(api_provider, hash, caller, pap_contract, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			Ok(EvmAddress::from(H256::from_slice(&call_info.value)))
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	/// Calls Runtime API.
	pub fn fetch_price_oracle<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		pap_contract: EvmAddress,
		caller: EvmAddress,
	) -> Result<EvmAddress, LiquidationError> {
		let data = Into::<u32>::into(Function::GetPriceOracle).to_be_bytes().to_vec();
		let gas_limit = U256::from(100_000);

		let call_info = ApiProvider::call(api_provider, hash, caller, pap_contract, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			Ok(EvmAddress::from(H256::from_slice(&call_info.value)))
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	/// Get the list of reserve assets.
	/// Calls Runtime API.
	fn fetch_reserves_list<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		mm_pool: EvmAddress,
		caller: EvmAddress,
	) -> Result<Vec<EvmAddress>, LiquidationError> {
		let data = Into::<u32>::into(Function::GetReservesList).to_be_bytes().to_vec();
		let gas_limit = U256::from(500_000);

		let call_info = ApiProvider::call(api_provider, hash, caller, mm_pool, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			let decoded = ethabi::decode(
				&[ethabi::ParamType::Array(Box::new(ethabi::ParamType::Address))],
				&call_info.value,
			)
			.map_err(LiquidationError::EthAbiError)?;

			let decoded = decoded[0].clone().into_array().ok_or(ethabi::Error::InvalidData)?;
			let mut address_arr = Vec::new();
			for i in decoded.iter() {
				address_arr.push(i.clone().into_address().ok_or(ethabi::Error::InvalidData)?);
			}

			Ok(address_arr)
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	/// Calls Runtime API.
	fn fetch_reserve_data<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		mm_pool: EvmAddress,
		asset_address: EvmAddress,
		caller: EvmAddress,
	) -> Result<ReserveData, LiquidationError> {
		let mut data = Into::<u32>::into(Function::GetReserveData).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(asset_address).as_bytes());

		let gas_limit = U256::from(500_000);
		let call_info = ApiProvider::call(api_provider, hash, caller, mm_pool, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			let decoded = ethabi::decode(
				&[
					ethabi::ParamType::Uint(32), // ReserveConfigurationMap
					ethabi::ParamType::Uint(16), // liquidityIndex
					ethabi::ParamType::Uint(16), // currentLiquidityRate
					ethabi::ParamType::Uint(16), // variableBorrowIndex
					ethabi::ParamType::Uint(16), // currentVariableBorrowRate
					ethabi::ParamType::Uint(16), // currentStableBorrowRate
					ethabi::ParamType::Uint(5),  // lastUpdateTimestamp
					ethabi::ParamType::Uint(2),  // id
					ethabi::ParamType::Address,  // aTokenAddress
					ethabi::ParamType::Address,  // stableDebtTokenAddress
					ethabi::ParamType::Address,  // variableDebtTokenAddress
					ethabi::ParamType::Address,  // interestRateStrategyAddress
					ethabi::ParamType::Uint(16), // accruedToTreasury
					ethabi::ParamType::Uint(16), // unbacked
					ethabi::ParamType::Uint(16), // isolationModeTotalDebt
				],
				&call_info.value,
			)?;

			Ok(ReserveData::new(&decoded).ok_or(ethabi::Error::InvalidData)?)
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	/// Calls Runtime API.
	fn fetch_asset_symbol<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		asset_address: &EvmAddress,
		caller: EvmAddress,
	) -> Result<Vec<u8>, LiquidationError> {
		let data = Into::<u32>::into(Function::Symbol).to_be_bytes().to_vec();
		let gas_limit = U256::from(500_000);

		let call_info = ApiProvider::call(api_provider, hash, caller, *asset_address, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			let decoded = ethabi::decode(&[ethabi::ParamType::String], &call_info.value)?;

			let symbol = decoded[0].clone().into_string().ok_or(ethabi::Error::InvalidData)?;
			Ok(symbol.into_bytes())
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	/// Get the price of an asset.
	/// Calls Runtime API.
	pub fn fetch_asset_price<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		api_provider: &ApiProvider,
		hash: Block::Hash,
		oracle_address: EvmAddress,
		asset: EvmAddress,
		caller: EvmAddress,
	) -> Result<U256, LiquidationError> {
		let mut data = Into::<u32>::into(Function::GetAssetPrice).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(asset).as_bytes());

		let gas_limit = U256::from(500_000);
		let call_info = ApiProvider::call(api_provider, hash, caller, oracle_address, data, gas_limit)
			.map_err(LiquidationError::ApiError)?
			.map_err(LiquidationError::DispatchError)?;

		if call_info.exit_reason == Succeed(Returned) {
			Ok(U256::checked_from(&call_info.value[0..32])
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?)
		} else {
			Err(LiquidationError::EvmError(call_info.exit_reason))
		}
	}

	pub fn get_asset_address(&self, asset_str: &str) -> Result<EvmAddress, LiquidationError> {
		let reserve_index = self
			.reserves()
			.iter()
			.position(|x| x.symbol == asset_str.as_bytes().to_vec())
			.ok_or(LiquidationError::ReserveNotFound)?;
		Ok(self.reserves[reserve_index].asset_address)
	}

	/// Change the stored price of some reserve asset.
	/// Reserves are not recalculated.
	pub fn update_reserve_price(&mut self, asset_address: EvmAddress, new_price: &U256) {
		let maybe_reserve = self.reserves.iter_mut().find(|x| x.asset_address == asset_address);

		if let Some(reserve) = maybe_reserve {
			reserve.price = *new_price;
		}
	}

	/// Calls Runtime API.
	/// Returns a list of all asset addresses used as user's collateral or debt.
	pub fn get_user_asset_addresses<ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>>(
		&self,
		api_provider: ApiProvider,
		hash: Block::Hash,
		user: EvmAddress,
		caller: EvmAddress,
	) -> Result<Vec<EvmAddress>, LiquidationError> {
		let configuration = UserConfiguration(UserData::fetch_user_configuration::<
			Block,
			ApiProvider,
			OriginCaller,
			RuntimeCall,
			RuntimeEvent,
		>(&api_provider, hash, self.pool_contract, user, caller)?);

		let mut user_assets = Vec::new();
		for (index, reserve) in self.reserves.iter().enumerate() {
			if configuration.is_collateral(index) || configuration.is_debt(index) {
				user_assets.push(reserve.asset_address);
			};
		}

		Ok(user_assets)
	}

	/// The formula:
	/// `debt_to_liquidate = (THF * Td - Sum(Ci * Pci * LTi)) / (Pd * (THF - LB * LTc))`
	/// where
	///    `THF` - target healt factor
	///    `Td` - total debt in base currency
	///    `Ci` - collateral amount
	///    `Pci` - collateral asset price
	///    `LTi` - liquidity threshold of collateral asset
	///    `Pd` - debt asset price
	///    `LB` - liquidation bonus of the collateral asset
	///    `LTc` - liquidity threshold of collateral asset
	///
	/// `user_data` - User's data, generated from the ` MoneyMarketData ` struct with updated price.
	/// `target_health_factor` - 18 decimal places
	/// `collateral_asset_address` - The address of the collateral asset.
	/// `debt_asset_address` - The address of the debt asset.
	///
	/// Return the amount of debt asset that needs to be liquidated to get the HF to `target_health_factor`
	pub fn calculate_debt_to_liquidate<
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	>(
		&self,
		user_data: &UserData,
		target_health_factor: U256,
		collateral_asset_address: EvmAddress,
		debt_asset_address: EvmAddress,
	) -> Result<LiquidationAmounts, LiquidationError> {
		let mut weighted_total_collateral = U256::zero();
		let mut total_debt_in_base = U256::zero();
		let mut collateral_liquidation_threshold = U256::zero();
		let mut liquidation_bonus = U256::zero();
		let mut collateral_price = U256::zero();
		let mut debt_price = U256::zero();
		let mut debt_decimals = 0u8;
		let mut collateral_decimals = 0u8;
		let mut user_collateral_amount = U256::zero();
		let mut user_debt_amount = U256::zero();
		let percentage_factor = U256::from(10u128.pow(4));
		let hf_one = U256::from(10).pow(18.into());
		let oracle_price_decimals = 8;
		let unit_price = U256::from(10u128.pow(oracle_price_decimals as u32));

		// Iterate through all reserves to calculate total collateral and debt in base currency, and weighted total collateral
		for (index, reserve) in self.reserves().iter().enumerate() {
			let user_balances = if let Some(maybe_reserve) = user_data.reserves().get(index) {
				maybe_reserve.clone()
			} else {
				Default::default()
			};

			weighted_total_collateral = weighted_total_collateral
				.checked_add(
					user_balances
						.collateral
						.checked_mul(U256::from(reserve.liquidation_threshold()))
						.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?,
				)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			total_debt_in_base = total_debt_in_base
				.checked_add(user_balances.debt)
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

			if reserve.asset_address() == collateral_asset_address {
				// Get liquidation threshold of the collateral asset
				collateral_liquidation_threshold = reserve.liquidation_threshold().into();
				// Get liquidation bonus of the collateral asset
				liquidation_bonus = reserve.liquidation_bonus();
				// Get price of the collateral asset
				collateral_price = reserve.price();
				collateral_decimals = reserve.decimals();
				user_collateral_amount = user_balances.collateral;
			}

			if reserve.asset_address() == debt_asset_address {
				// Get price and decimals of the debt asset
				debt_price = reserve.price();
				debt_decimals = reserve.decimals();
				user_debt_amount = user_balances.debt;
			}
		}

		// convert percentage to decimal number
		weighted_total_collateral = weighted_total_collateral
			.checked_div(percentage_factor)
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

		let n: U256 = total_debt_in_base
			.full_mul(target_health_factor)
			.checked_div(hf_one.into())
			.and_then(|r| r.checked_sub(weighted_total_collateral.into()))
			.and_then(|r| r.checked_mul(unit_price.into()))
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		let d: U256 = percentage_factor
			.full_mul(target_health_factor)
			.checked_div(hf_one.into())
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.checked_sub(
				liquidation_bonus
					.full_mul(collateral_liquidation_threshold)
					.checked_div(percentage_factor.into())
					.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?,
			)
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		let d = percent_mul(debt_price, d)?;

		// in debt asset
		let debt_to_liquidate = n
			.checked_div(d)
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?;

		// adjust decimals from `oracle_price_decimals` to `debt_decimals`
		let debt_to_liquidate = if debt_decimals > oracle_price_decimals {
			debt_to_liquidate
				.checked_mul(U256::from(10).pow((debt_decimals - oracle_price_decimals).into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
		} else {
			debt_to_liquidate
				.checked_div(U256::from(10).pow((oracle_price_decimals - debt_decimals).into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
		};

		// Our calculation provides theoretical amount that needs to be liquidated to get the HF close to `target_health_factor`.
		// But there is no guarantee that user has required amount of debt and collateral assets.
		// Adjust these amounts based on how much can be actually liquidated.

		let health_factor =
			user_data.health_factor::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(self)?;
		let close_factor = if health_factor > CLOSE_FACTOR_HF_THRESHOLD.into() {
			DEFAULT_LIQUIDATION_CLOSE_FACTOR
		} else {
			MAX_LIQUIDATION_CLOSE_FACTOR
		}
		.into();

		// in debt asset
		user_debt_amount = user_debt_amount
			.full_mul(U256::from(10u128.pow(debt_decimals.into())))
			.checked_div(debt_price.into())
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		// Calculate max debt that can be liquidated. Max amount is affected by the close factor and user's total debt amount.
		// In debt asset.
		let max_liquidatable_debt = percent_mul(user_debt_amount, close_factor)?;

		let mut actual_debt_to_liquidate = if debt_to_liquidate > max_liquidatable_debt {
			max_liquidatable_debt
		} else {
			debt_to_liquidate
		};

		// in collateral asset without the bonus
		let mut base_collateral_amount: U256 = actual_debt_to_liquidate
			.full_mul(debt_price)
			.checked_div(collateral_price.into())
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;
		base_collateral_amount = if collateral_decimals > debt_decimals {
			base_collateral_amount
				.checked_mul(U256::from(10).pow((collateral_decimals - debt_decimals).into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
		} else {
			base_collateral_amount
				.checked_div(U256::from(10).pow((debt_decimals - collateral_decimals).into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
		};

		// in collateral asset
		let mut collateral_amount = percent_mul(base_collateral_amount, liquidation_bonus)?;

		let mut collateral_in_base_currency: U256 = collateral_amount
			.full_mul(collateral_price)
			.checked_div(U512::from(10).pow(collateral_decimals.into()))
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		let mut debt_in_base_currency = actual_debt_to_liquidate
			.full_mul(debt_price)
			.checked_div(U512::from(10).pow(debt_decimals.into()))
			.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		// Adjust the liquidation amounts if user doesn't have expected amount of the collateral asset.
		if collateral_in_base_currency > user_collateral_amount {
			// in debt asset
			actual_debt_to_liquidate = user_collateral_amount
				.full_mul(percentage_factor)
				.checked_div(liquidation_bonus.into())
				.and_then(|r| r.checked_mul(U512::from(10u128.pow(debt_decimals.into()))))
				.and_then(|r| r.checked_div(debt_price.into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
				.try_into()
				.map_err(|_| ArithmeticError::Overflow)?;

			// in collateral asset without the bonus
			base_collateral_amount = actual_debt_to_liquidate
				.full_mul(debt_price)
				.checked_div(collateral_price.into())
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
				.try_into()
				.map_err(|_| ArithmeticError::Overflow)?;
			base_collateral_amount = if collateral_decimals > debt_decimals {
				base_collateral_amount
					.checked_mul(U256::from(10).pow((collateral_decimals - debt_decimals).into()))
					.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			} else {
				base_collateral_amount
					.checked_div(U256::from(10).pow((debt_decimals - collateral_decimals).into()))
					.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
			};

			// in collateral asset
			collateral_amount = percent_mul(base_collateral_amount, liquidation_bonus)?;

			debt_in_base_currency = actual_debt_to_liquidate
				.full_mul(debt_price)
				.checked_div(U512::from(10).pow(debt_decimals.into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
				.try_into()
				.map_err(|_| ArithmeticError::Overflow)?;

			collateral_in_base_currency = collateral_amount
				.full_mul(collateral_price)
				.checked_div(U512::from(10).pow(collateral_decimals.into()))
				.ok_or::<LiquidationError>(ArithmeticError::Overflow.into())?
				.try_into()
				.map_err(|_| ArithmeticError::Overflow)?;
		}

		Ok(LiquidationAmounts {
			debt_amount: actual_debt_to_liquidate,
			collateral_amount,
			debt_in_base_currency,
			collateral_in_base_currency,
		})
	}

	/// Calculate liquidation options based on the user's reserve, price update and target health factor.
	/// Liquidation options are calculated for all collateral/debt asset pairs.
	/// `user_data` - User's data, generated from the ` MoneyMarketData ` struct with updated price.
	/// `target_health_factor` - 18 decimal places.
	/// `updated_assets` - Skips the calculation if none of the assets is user's collateral or borrow asset.
	///     Use `None` to disable this check.
	///
	/// Return the amount of debt asset that needs to be liquidated to get the HF to `target_health_factor`
	pub fn calculate_liquidation_options<
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	>(
		&mut self,
		user_data: &UserData,
		target_health_factor: U256,
		updated_assets: Option<&Vec<EvmAddress>>,
	) -> Result<Vec<LiquidationOption>, LiquidationError> {
		let mut liquidation_options = Vec::new();

		let collateral_assets = user_data.collateral_assets(self);
		let debt_assets = user_data.debt_assets(self);

		// Early return if assets from `asset_addresses` are not in the user's collateral/debt assets.
		if let Some(updated_assets) = updated_assets {
			let user_assets: Vec<_> = collateral_assets.iter().chain(debt_assets.iter()).collect();
			if !user_assets.iter().any(|(_index, asset)| updated_assets.contains(asset)) {
				return Ok(Vec::new());
			}
		}

		// Calculate the amount of debt that needs to be liquidated to get the HF closer
		// to `target_health_factor`. Calculated for all combinations of collateral and debt assets.
		for &(index_c, collateral_asset) in collateral_assets.iter() {
			for &(index_d, debt_asset) in debt_assets.iter() {
				let Ok(LiquidationAmounts {
					debt_amount,
					collateral_amount: _,
					debt_in_base_currency,
					collateral_in_base_currency,
				}) = self.calculate_debt_to_liquidate::<ApiProvider>(
					user_data,
					target_health_factor,
					collateral_asset,
					debt_asset,
				)
				else {
					continue;
				};

				let mut new_user_data = user_data.clone();

				let mut user_reserve = new_user_data.reserves()[index_c].clone();
				user_reserve.collateral = user_reserve.collateral.saturating_sub(collateral_in_base_currency);
				new_user_data.update_reserves(sp_std::vec!((index_c, user_reserve)));

				let mut user_reserve = new_user_data.reserves()[index_d].clone();
				user_reserve.debt = user_reserve.debt.saturating_sub(debt_in_base_currency);
				new_user_data.update_reserves(sp_std::vec!((index_d, user_reserve)));

				// calculate HF based on updated price and reserves
				let maybe_hf =
					new_user_data.health_factor::<Block, ApiProvider, OriginCaller, RuntimeCall, RuntimeEvent>(self);

				if let Ok(hf) = maybe_hf {
					liquidation_options.push(LiquidationOption::new(hf, collateral_asset, debt_asset, debt_amount));
				}
			}
		}

		Ok(liquidation_options)
	}

	/// Evaluates all liquidation options and returns one that is closest to the `target_health_factor`.
	/// `user_data` - User's data, generated from the ` MoneyMarketData ` struct with updated price.
	/// `target_health_factor` - 18 decimal places.
	/// `updated_assets` - Skips the calculation if none of the assets is user's collateral or borrow asset.
	///     Use `None` to disable this check.
	///
	/// Return the amount of debt asset that needs to be liquidated to get the HF to `target_health_factor`.
	pub fn get_best_liquidation_option<
		ApiProvider: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	>(
		&mut self,
		user_data: &UserData,
		target_health_factor: U256,
		updated_assets: Option<&Vec<EvmAddress>>,
	) -> Result<Option<LiquidationOption>, LiquidationError> {
		let mut liquidation_options =
			self.calculate_liquidation_options::<ApiProvider>(user_data, target_health_factor, updated_assets)?;

		// choose liquidation option with the highest HF. All HFs should be less or close to the target HF.
		liquidation_options.sort_by(|a, b| a.health_factor.cmp(&b.health_factor));

		Ok(liquidation_options.last().cloned())
	}
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct LiquidationOption {
	pub health_factor: U256,
	pub collateral_asset: EvmAddress,
	pub debt_asset: EvmAddress,
	pub debt_to_liquidate: U256,
}
impl LiquidationOption {
	pub fn new(
		health_factor: U256,
		collateral_asset: EvmAddress,
		debt_asset: EvmAddress,
		debt_to_liquidate: U256,
	) -> Self {
		LiquidationOption {
			health_factor,
			collateral_asset,
			debt_asset,
			debt_to_liquidate,
		}
	}
}
