use codec::{Decode, Encode};
use evm::ExitReason;
use frame_support::pallet_prelude::*;
use frame_support::Deserialize;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_core::H160;
use sp_std::vec::Vec;

pub type Balance = u128;
pub type AssetId = u32;
pub type CallResult = (ExitReason, Vec<u8>);

pub const MAX_LIQUIDATIONS: u32 = 5;
pub const UNSIGNED_TXS_PRIORITY: u64 = 1_000_000;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
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
pub struct BorrowerData<AccountId> {
	pub last_global_update: u32,
	pub last_update: u32,
	pub borrowers: Vec<(H160, BorrowerDataDetails<AccountId>)>,
}

pub mod offchain_worker {
	use super::*;
	use ethabi::ethereum_types::U512;
	use fp_evm::{ExitReason::Succeed, ExitSucceed::Returned};
	use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApiV5;
	use frame_support::sp_runtime::traits::{Block as BlockT, CheckedConversion};
	use pallet_ethereum::Transaction;
	use hydradx_traits::evm::EvmAddress;
	use sp_core::{H256, U256};
	use sp_std::{boxed::Box, ops::BitAnd};

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
		SetMultipleValues = "setMultipleValues(string[],uint256[])",
		GetValue = "getValue(string)",
		LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
		Symbol = "symbol()",
	}

	/// Multiplies two ray, rounding half up to the nearest ray.
	pub fn ray_mul(a: U256, b: U256) -> Option<U256> {
		if a.is_zero() || b.is_zero() {
			return Some(U256::zero());
		}

		let ray = U512::from(10u128.pow(27));

		let res512 = a.full_mul(b).checked_add(ray / 2)?.checked_div(ray)?;
		res512.try_into().ok()
	}

	/// Divides two wad, rounding half up to the nearest wad.
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

	/// Calls Runtime API.
	pub fn fetch_current_evm_block_timestamp<Block, Runtime>() -> Option<u64>
	where
		Block: BlockT,
		Runtime: EthereumRuntimeRPCApiV5<Block>,
	{
		let timestamp = Runtime::current_block().map(|block| block.header.timestamp)?;
		timestamp.checked_div(1_000)
	}

	/// Executes a percentage multiplication.
	/// Params:
	///     value: The value of which the percentage needs to be calculated
	///     percentage: The percentage of the value to be calculated, in basis points.
	pub fn percent_mul(value: U256, percentage: U256) -> Option<U256> {
		if percentage.is_zero() {
			return Some(U256::zero());
		}

		let percentage_factor = U512::from(10u128.pow(4));
		let half_percentage_factor = percentage_factor / 2;
		let nominator = value.full_mul(percentage).checked_add(half_percentage_factor)?;
		let res = nominator.checked_div(percentage_factor)?;
		res.try_into().ok()
	}

	/// Collateral and debt amounts of some reserve in the base currency.
	#[derive(Default, Eq, PartialEq, Clone, RuntimeDebug)]
	pub struct UserReserve {
		pub collateral: U256,
		pub debt: U256,
	}

	/// The configuration of the user across all the reserves.
	/// Bitmap of the users collaterals and borrows. It is divided into pairs of bits, one pair per asset.
	/// The first bit indicates if an asset is used as collateral by the user, the second whether an asset is borrowed by the user.
	/// The corresponding assets are in the same position as `fetch_reserves_list()`.
	#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
	struct UserConfiguration(U256);
	impl UserConfiguration {
		/// Returns `true` if the asset is used as collateral by the user.
		/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
		pub fn is_collateral(&self, asset_index: usize) -> bool {
			let bit_mask = U256::from(2 << (2 * asset_index));
			!(self.0 & bit_mask).is_zero()
		}

		/// Returns `true` if the asset is used as debt by the user.
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
		pub current_evm_timestamp: u64,
	}
	impl UserData {
		/// Calls Runtime API.
		pub fn new<Block, Runtime>(
			money_market: &MoneyMarketData<Block, Runtime>,
			address: H160,
			current_evm_timestamp: u64,
			caller: EvmAddress,
		) -> Option<Self>
		where
			Block: BlockT,
			Runtime: EthereumRuntimeRPCApiV5<Block>,
		{
			let configuration = UserConfiguration(Self::fetch_user_configuration::<Block, Runtime>(
				money_market.pool_contract,
				address,
				caller,
			)?);

			let mut reserves = Vec::new();
			for (index, reserve) in money_market.reserves.iter().enumerate() {
				// skip the computation if the reserve is not used as user's collateral or debt
				let (collateral, debt) = if configuration.is_collateral(index) || configuration.is_debt(index) {
					let c = reserve
						.get_user_collateral_in_base_currency::<Block, Runtime>(address, current_evm_timestamp, caller)
						.unwrap_or_default();
					let d = reserve
						.get_user_debt_in_base_currency::<Block, Runtime>(address, current_evm_timestamp, caller)
						.unwrap_or_default();
					(c, d)
				} else {
					(U256::zero(), U256::zero())
				};
				reserves.push(UserReserve { collateral, debt });
			}

			Some(Self {
				address,
				configuration,
				reserves,
				current_evm_timestamp,
			})
		}

		/// Get user's address.
		pub fn address(&self) -> EvmAddress {
			self.address
		}

		/// Get user's reserves.
		pub fn reserves(&self) -> &Vec<UserReserve> {
			&self.reserves
		}

		/// Returns `true` if the asset is used as collateral by the user.
		/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
		pub fn is_collateral(&self, asset_index: usize) -> bool {
			self.configuration.is_collateral(asset_index)
		}

		/// Returns `true` if the asset is used as debt by the user.
		/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
		pub fn is_debt(&self, asset_index: usize) -> bool {
			self.configuration.is_debt(asset_index)
		}

		pub fn update_reserves(&mut self, reserves: Vec<(usize, UserReserve)>) {
			let len = self.reserves.len();

			for (i, reserve) in reserves {
				if i < len {
					self.reserves[i] = reserve;
				}
			}
		}

		/// Calculates user's health factor.
		pub fn health_factor<Block, Runtime>(&self, money_market: &MoneyMarketData<Block, Runtime>) -> Option<U256>
		where
			Block: BlockT,
			Runtime: EthereumRuntimeRPCApiV5<Block>,
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
					.checked_add(partial_collateral.checked_mul(U256::from(reserve.liquidation_threshold()))?)?;
				total_collateral = total_collateral.checked_add(partial_collateral)?;
				total_debt = total_debt.checked_add(user_reserve.debt)?;
			}

			avg_liquidation_threshold = avg_liquidation_threshold.checked_div(total_collateral)?;

			let nominator = percent_mul(total_collateral, avg_liquidation_threshold)?;
			wad_div(nominator, total_debt)
		}

		/// Returns Bitmap of the users collaterals and borrows.
		/// It is divided into pairs of bits, one pair per asset.
		/// The first bit indicates if an asset is used as collateral by the user, the second whether an asset is borrowed by the user.
		/// The corresponding assets are in the same position as getReservesList().
		/// Calls Runtime API.
		pub fn fetch_user_configuration<Block, Runtime>(
			mm_pool: EvmAddress,
			user: EvmAddress,
			caller: EvmAddress,
		) -> Option<U256>
		where
			Block: BlockT,
			Runtime: EthereumRuntimeRPCApiV5<Block>,
		{
			let mut data = Into::<u32>::into(Function::GetUserConfiguration).to_be_bytes().to_vec();
			data.extend_from_slice(H256::from(user).as_bytes());

			let gas_limit = U256::from(500_000);
			let call_info = Runtime::call(
				caller,
				mm_pool,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				Some(U256::checked_from(&call_info.value[0..32])?)
			} else {
				None
			}
		}
	}

	trait BalanceOf<Block, Runtime>
	where
		Block: BlockT,
		Runtime: EthereumRuntimeRPCApiV5<Block>,
	{
		fn fetch_scaled_balance_of(self, user: EvmAddress, caller: EvmAddress) -> Option<U256>;
		fn fetch_balance_of(self, user: EvmAddress, caller: EvmAddress) -> Option<U256>;
	}
	impl<Block, Runtime> BalanceOf<Block, Runtime> for EvmAddress
	where
		Block: BlockT,
		Runtime: EthereumRuntimeRPCApiV5<Block>,
	{
		/// Calls Runtime API.
		fn fetch_scaled_balance_of(self, user: EvmAddress, caller: EvmAddress) -> Option<U256> {
			let mut data = Into::<u32>::into(Function::ScaledBalanceOf).to_be_bytes().to_vec();
			data.extend_from_slice(H256::from(user).as_bytes());

			let gas_limit = U256::from(500_000);
			let call_info = Runtime::call(
				caller,
				self,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				Some(U256::checked_from(&call_info.value[0..32])?)
			} else {
				None
			}
		}

		/// Calls Runtime API.
		fn fetch_balance_of(self, user: EvmAddress, caller: EvmAddress) -> Option<U256> {
			let mut data = Into::<u32>::into(Function::BalanceOf).to_be_bytes().to_vec();
			data.extend_from_slice(H256::from(user).as_bytes());

			let gas_limit = U256::from(500_000);
			let call_info = Runtime::call(
				caller,
				self,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				Some(U256::checked_from(&call_info.value[0..32])?)
			} else {
				None
			}
		}
	}

	/// Configuration of the reserve.
	/// https://github.com/aave/aave-v3-core/blob/782f51917056a53a2c228701058a6c3fb233684a/contracts/protocol/libraries/types/DataTypes.sol#L5
	#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
	pub struct ReserveData {
		configuration: U256, // https://github.com/aave-dao/aave-v3-origin/blob/3aad8ca184159732e4b3d8c82cd56a8707a106a2/src/core/contracts/protocol/libraries/types/DataTypes.sol#L79
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
		pub fn new(data: &[ethabi::Token]) -> Option<Self> {
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
	}

	/// State of the reserve.
	#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
	pub struct Reserve {
		reserve_data: ReserveData,
		asset_address: EvmAddress,
		symbol: Vec<u8>,
		price: U256,
	}
	impl Reserve {
		/// Get price of the reserve.
		pub fn price(&self) -> U256 {
			self.price
		}

		/// Get asset symbol of the reserve.
		pub fn symbol(&self) -> &Vec<u8> {
			&self.symbol
		}

		/// Get address of the reserve.
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

		fn get_normalized_income(&self, current_timestamp: u64) -> Option<U256> {
			if current_timestamp == self.reserve_data.last_update_timestamp {
				Some(U256::from(self.reserve_data.liquidity_index))
			} else {
				let current_liquidity_rate = U256::from(self.reserve_data.current_liquidity_rate);
				let timestamp_diff =
					U256::from(current_timestamp.checked_sub(self.reserve_data.last_update_timestamp)?);
				let nominator = current_liquidity_rate.checked_mul(timestamp_diff)?;
				let seconds_per_year = U256::from(365 * 24 * 60 * 60);
				let ray = U256::from(10u128.pow(27));
				let linear_interest = nominator.checked_div(seconds_per_year)?.checked_add(ray)?;
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
		pub fn get_user_collateral_in_base_currency<Block, Runtime>(
			&self,
			user: EvmAddress,
			current_timestamp: u64,
			caller: EvmAddress,
		) -> Option<U256>
		where
			Block: BlockT,
			Runtime: EthereumRuntimeRPCApiV5<Block>,
		{
			let (collateral_address, _) = self.get_collateral_and_debt_addresses();
			let scaled_balance = BalanceOf::<Block, Runtime>::fetch_scaled_balance_of(collateral_address, user, caller)?;
			let normalized_income = self.get_normalized_income(current_timestamp)?;

			ray_mul(scaled_balance, normalized_income)?
				.checked_mul(self.price)?
				.checked_div(U256::from(10u128.pow(self.decimals() as u32)))
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
				let base_power_two = ray_mul(rate, rate)?.checked_div(seconds_per_year * seconds_per_year)?;
				let base_power_three = ray_mul(base_power_two, rate)?;

				let second_term = exp.checked_mul(exp_minus_one)?.checked_mul(base_power_two)? / 2;
				let third_term = exp
					.checked_mul(exp_minus_one)?
					.checked_mul(exp_minus_two)?
					.checked_mul(base_power_three)?
					/ 6;

				let compound_interest = rate
					.checked_mul(exp)?
					.checked_div(seconds_per_year)?
					.checked_add(ray)?
					.checked_add(second_term)?
					.checked_add(third_term)?;

				ray_mul(compound_interest, variable_borrow_index)
			}
		}

		/// Get user's debt in base currency.
		/// Calls Runtime API.
		pub fn get_user_debt_in_base_currency<Block, Runtime>(
			&self,
			user: EvmAddress,
			current_timestamp: u64,
			caller: EvmAddress,
		) -> Option<U256>
		where
			Block: BlockT,
			Runtime: EthereumRuntimeRPCApiV5<Block>,
		{
			let (_, (stable_debt_address, variable_debt_address)) = self.get_collateral_and_debt_addresses();
			let mut total_debt = BalanceOf::<Block, Runtime>::fetch_scaled_balance_of(variable_debt_address, user, caller)?;
			if !total_debt.is_zero() {
				let normalized_debt = self.get_normalized_debt(current_timestamp)?;
				total_debt = ray_mul(total_debt, normalized_debt)?;
			}

			total_debt = total_debt.checked_add(BalanceOf::<Block, Runtime>::fetch_balance_of(stable_debt_address, user, caller)?)?;

			total_debt
				.checked_mul(self.price)?
				.checked_div(U256::from(10u128.pow(self.decimals() as u32)))
		}
	}

	/// Captures the state of the money market related to liquidations.
	/// The state is not automatically updated. Any change in the chain can invalidate the data stored in the struct.
	#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
	pub struct MoneyMarketData<Block, Runtime>
	where
		Block: BlockT,
		Runtime: EthereumRuntimeRPCApiV5<Block>,
	{
		pap_contract: EvmAddress, // PoolAddressesProvider
		pool_contract: EvmAddress,
		oracle_contract: EvmAddress,
		reserves: Vec<Reserve>, // the order of reserves is given by fetch_reserves_list()
		pub caller: EvmAddress,
		_phantom: PhantomData<(Block, Runtime)>,
	}
	impl<Block: BlockT, Runtime: EthereumRuntimeRPCApiV5<Block>> MoneyMarketData<Block, Runtime> {
		/// Calls Runtime API.
		pub fn new(pap_contract: EvmAddress, caller: EvmAddress) -> Option<Self> {
			let pool_contract = Self::fetch_pool(pap_contract, caller)?;
			let oracle_contract = Self::fetch_price_oracle(pap_contract, caller)?;

			let mut reserves = Vec::new();
			for asset_address in Self::fetch_reserves_list(pool_contract, caller)?.into_iter() {
				let reserve_data = Self::fetch_reserve_data(pool_contract, asset_address, caller)?;
				let symbol = Self::fetch_asset_symbol(&asset_address, caller)?;
				let price = Self::fetch_asset_price(oracle_contract, asset_address, caller)?;
				reserves.push(Reserve {
					reserve_data,
					asset_address,
					symbol,
					price,
				});
			}

			Some(Self {
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

		/// Get list of the reserves.
		pub fn reserves(&self) -> &Vec<Reserve> {
			&self.reserves
		}

		/// Calls Runtime API.
		pub fn fetch_pool(pap_contract: EvmAddress, caller: EvmAddress) -> Option<EvmAddress> {
			let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();
			let gas_limit = U256::from(100_000);
			let call_info = Runtime::call(
				caller,
				pap_contract,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				Some(EvmAddress::from(H256::from_slice(&call_info.value)))
			} else {
				None
			}
		}

		/// Calls Runtime API.
		pub fn fetch_price_oracle(pap_contract: EvmAddress, caller: EvmAddress) -> Option<EvmAddress> {
			let data = Into::<u32>::into(Function::GetPriceOracle).to_be_bytes().to_vec();
			let gas_limit = U256::from(100_000);

			let call_info = Runtime::call(
				caller,
				pap_contract,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				Some(EvmAddress::from(H256::from_slice(&call_info.value)))
			} else {
				None
			}
		}

		/// Get the list of reserve assets.
		/// Calls Runtime API.
		fn fetch_reserves_list(mm_pool: EvmAddress, caller: EvmAddress) -> Option<Vec<EvmAddress>> {
			let data = Into::<u32>::into(Function::GetReservesList).to_be_bytes().to_vec();
			let gas_limit = U256::from(500_000);

			let call_info = Runtime::call(
				caller,
				mm_pool,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				let decoded = ethabi::decode(
					&[ethabi::ParamType::Array(Box::new(ethabi::ParamType::Address))],
					&call_info.value,
				)
				.ok()?;

				let decoded = decoded[0].clone().into_array()?;
				let mut address_arr = Vec::new();
				for i in decoded.iter() {
					address_arr.push(i.clone().into_address()?);
				}

				Some(address_arr)
			} else {
				None
			}
		}

		/// Calls Runtime API.
		fn fetch_reserve_data(mm_pool: EvmAddress, asset_address: EvmAddress, caller: EvmAddress) -> Option<ReserveData> {
			let mut data = Into::<u32>::into(Function::GetReserveData).to_be_bytes().to_vec();
			data.extend_from_slice(H256::from(asset_address).as_bytes());

			let gas_limit = U256::from(500_000);
			let call_info = Runtime::call(
				caller,
				mm_pool,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

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
				)
				.ok()?;

				ReserveData::new(&decoded)
			} else {
				None
			}
		}

		/// Calls Runtime API.
		fn fetch_asset_symbol(asset_address: &EvmAddress, caller: EvmAddress) -> Option<Vec<u8>> {
			let data = Into::<u32>::into(Function::Symbol).to_be_bytes().to_vec();
			let gas_limit = U256::from(500_000);

			let call_info = Runtime::call(
				caller,
				*asset_address,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				let decoded = ethabi::decode(&[ethabi::ParamType::String], &call_info.value).ok()?;

				let symbol = decoded[0].clone().into_string()?;
				Some(symbol.into_bytes())
			} else {
				None
			}
		}

		/// Get price of an asset.
		/// Calls Runtime API.
		pub fn fetch_asset_price(oracle_address: EvmAddress, asset: EvmAddress, caller: EvmAddress) -> Option<U256> {
			let mut data = Into::<u32>::into(Function::GetAssetPrice).to_be_bytes().to_vec();
			data.extend_from_slice(H256::from(asset).as_bytes());

			let gas_limit = U256::from(500_000);
			let call_info = Runtime::call(
				caller,
				oracle_address,
				data,
				U256::zero(),
				gas_limit,
				None,
				None,
				None,
				true,
				None,
			)
			.ok()?;

			if call_info.exit_reason == Succeed(Returned) {
				Some(U256::checked_from(&call_info.value[0..32])?)
			} else {
				None
			}
		}

		pub fn get_asset_address(&self, asset_str: &str) -> Option<EvmAddress> {
			let reserve_index = self.reserves().iter().position(|x| x.symbol == asset_str.as_bytes().to_vec())?;
			Some(self.reserves[reserve_index].asset_address)
		}

		/// Change the stored price of some reserve asset.
		/// Reserves are not recalculated.
		pub fn update_reserve_price(&mut self, asset_address: EvmAddress, new_price: U256) {
			let maybe_reserve = self.reserves.iter_mut().find(|x| x.asset_address == asset_address);

			if let Some(reserve) = maybe_reserve {
				reserve.price = new_price;
			}
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
		/// `user_address` - Address of the user that will be liquidated
		/// `target_health_factor` - 18 decimal places
		/// `caller` - Account executing runtime RPC call, needs to have some WETH balance.
		///
		/// Return the amount of debt asset that needs to be liquidated to get the HF to `target_health_factor`
		pub fn calculate_debt_to_liquidate(
			&self,
			user_data: &UserData,
			target_health_factor: U256,
			collateral_asset_address: EvmAddress,
			debt_asset_address: EvmAddress,
		) -> Option<((U256, U256), (U256, U256))> {
			// all amounts are in base currency
			let mut weighted_total_collateral = U256::zero();
			let mut total_collateral_in_base = U256::zero();
			let mut total_debt_in_base = U256::zero();
			let mut collateral_liquidation_threshold = U256::zero();
			let mut liquidation_bonus = U256::zero();
			let mut collateral_price = U256::zero();
			let mut debt_price = U256::zero();
			let mut debt_decimals = 0u8;
			let mut user_collateral_amount = U256::zero();
			let mut user_debt_amount = U256::zero();
			let unit_price = U256::from(10_000_000_000u128);
			let percentage_factor = U256::from(10u128.pow(4));

			// Iterate through all reserves to calculate total collateral and debt in base currency, and weighted total collateral
			for (index, reserve) in self.reserves().iter().enumerate() {
				let user_balances = if let Some(maybe_reserve) = user_data.reserves().get(index) {
					maybe_reserve.clone()
				} else {
					Default::default()
				};

				weighted_total_collateral = weighted_total_collateral.checked_add(
					user_balances
						.collateral
						.checked_mul(U256::from(reserve.liquidation_threshold()))?,
				)?;

				total_collateral_in_base = total_collateral_in_base.checked_add(user_balances.collateral)?;

				total_debt_in_base = total_debt_in_base.checked_add(user_balances.debt)?;

				if reserve.asset_address() == collateral_asset_address {
					// Get liquidation threshold of the collateral asset
					collateral_liquidation_threshold = reserve.liquidation_threshold().into();
					// Get liquidation bonus of the collateral asset
					liquidation_bonus = reserve.liquidation_bonus();
					// Get price of the collateral asset
					collateral_price = reserve.price();
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
			weighted_total_collateral = weighted_total_collateral.checked_div(percentage_factor)?;

			let target_health_factor = target_health_factor.checked_div(U256::from(10).pow(8.into()))?;

			let n: U256 = total_debt_in_base
				.full_mul(target_health_factor)
				.checked_div(unit_price.into())?
				.checked_sub(weighted_total_collateral.into())?
				.checked_mul(unit_price.into())?.try_into().ok()?;

			let d: U256 = percentage_factor
				.full_mul(target_health_factor)
				.checked_div(unit_price.into())?
				.checked_sub(
					liquidation_bonus
						.full_mul(collateral_liquidation_threshold.into())
						.checked_div(percentage_factor.into())?,
				)?.try_into().ok()?;

			log::info!("\n-- - - \nn: {:?}\nd: {:?}", n, d);
			let d = percent_mul(debt_price, d)?;

			let debt_to_liquidate = n.checked_div(d)?;
			log::info!("\nd: {:?}\ndtl: {:?}", d, debt_to_liquidate);

			let total_debt: U256 = total_debt_in_base.full_mul(U256::from(10u128.pow(debt_decimals.into()))).checked_div(debt_price.into())?.try_into().ok()?;

			// Our calculation provides theoretical amount that needs to be liquidated to get the HF close to `target_health_factor`.
			// But there is no guarantee that user has required amount of debt and collateral assets.
			// Adjust these amounts based on how much can be actually liquidated.

			let health_factor = user_data.health_factor(self)?;
			let close_factor = if health_factor > CLOSE_FACTOR_HF_THRESHOLD.into() {
				DEFAULT_LIQUIDATION_CLOSE_FACTOR
			} else {
				MAX_LIQUIDATION_CLOSE_FACTOR
			}
			.into();

			// in debt asset
			user_debt_amount = user_debt_amount.full_mul(U256::from(10u128.pow(debt_decimals.into()))).checked_div(debt_price.into())?.try_into().ok()?;

			// Calculate max debt that can be liquidated. Max amount is affected by the close factor and user's total debt amount.
			let max_liquidatable_debt = percent_mul(user_debt_amount, close_factor)?;

			let mut actual_debt_to_liquidate = if debt_to_liquidate > max_liquidatable_debt {
				max_liquidatable_debt
			} else {
				debt_to_liquidate
			};

			// Adjust the liquidation amounts if user doesn't have expected amount of the collateral asset.
			let base_collateral_amount = actual_debt_to_liquidate
				.full_mul(debt_price)
				.checked_div(collateral_price.into())?
				.try_into()
				.ok()?;
			let collateral_amount = percent_mul(base_collateral_amount, liquidation_bonus)?;

			// let debt_in_base_currency = actual_debt_to_liquidate
			// 	.full_mul(debt_price)
			// 	.checked_div(unit_price.into())?
			// 	.try_into()
			// 	.ok()?;
			let collateral_in_base_currency: U256 = collateral_amount
				.full_mul(collateral_price)
				.checked_div(unit_price.into())?
				.try_into()
				.ok()?;

			if collateral_in_base_currency > user_collateral_amount {
				// collateral in base currency, without bonus
				actual_debt_to_liquidate = user_collateral_amount
					.full_mul(percentage_factor)
					.checked_div(liquidation_bonus.into())?
					.checked_div(debt_price.into())?
					.checked_mul(unit_price.into())?
					.try_into()
					.ok()?;
			}

			let base_collateral_amount = actual_debt_to_liquidate
				.full_mul(debt_price)
				.checked_div(collateral_price.into())?
				.try_into()
				.ok()?;
			let collateral_amount = percent_mul(base_collateral_amount, liquidation_bonus)?;

			let debt_in_base_currency = actual_debt_to_liquidate
				.full_mul(debt_price)
				.checked_div(unit_price.into())?
				.try_into()
				.ok()?;
			let collateral_in_base_currency = collateral_amount
				.full_mul(collateral_price)
				.checked_div(unit_price.into())?
				.try_into()
				.ok()?;

			// //////////////////////

			Some((
				(actual_debt_to_liquidate, collateral_amount),
				(debt_in_base_currency, collateral_in_base_currency),
			))
		}

		pub fn calculate_liquidation_options(
			&mut self,
			user_data: &UserData,
			target_health_factor: U256,
			new_price: (EvmAddress, U256),
		) -> Option<Vec<LiquidationOption>> {
			let mut liquidation_options = Vec::new();
			let mut collateral_assets = Vec::new();
			let mut debt_assets = Vec::new();

			for (index, reserve) in self.reserves().iter().enumerate() {
				if user_data.is_collateral(index) {
					collateral_assets.push((index, reserve.asset_address()));
				}

				if user_data.is_debt(index) {
					debt_assets.push((index, reserve.asset_address()));
				}
			}

			// update the price
			let reserve_index = self.reserves().iter().position(|x| x.asset_address() == new_price.0)?;
			let reserve = self.reserves().get(reserve_index)?;
			let old_price = reserve.price();
			self.update_reserve_price(new_price.0, new_price.1);

			// TODO: continue if the price of callateral decreased/debt increased (the cases when HF decreases)

			// calculate amount of debt that needs to be liquidated to get the HF closer
			// to `target_health_factor`. Calculated for all combinations of collateral and debt assets
			for &(index_c, collateral_asset) in collateral_assets.iter() {
				for &(index_d, debt_asset) in debt_assets.iter() {
					let Some((
						(debt_to_liquidate, _collateral_received),
						(debt_to_liquidate_in_base, collateral_received_in_base),
					)) = self.calculate_debt_to_liquidate(user_data, target_health_factor, collateral_asset, debt_asset)
					else {
						continue;
					};

					let mut new_user_data = user_data.clone();

					let mut user_reserve = new_user_data.reserves()[index_c].clone();
					user_reserve.collateral = user_reserve.collateral.saturating_sub(collateral_received_in_base);
					new_user_data.update_reserves(sp_std::vec!((index_c, user_reserve)));

					let mut user_reserve = new_user_data.reserves()[index_d].clone();
					user_reserve.debt = user_reserve.debt.saturating_sub(debt_to_liquidate_in_base);
					new_user_data.update_reserves(sp_std::vec!((index_d, user_reserve)));

					// calculate HF based on updated price
					let maybe_hf = new_user_data.health_factor(self);

					if let Some(hf) = maybe_hf {
						liquidation_options.push(LiquidationOption::new(
							hf,
							collateral_asset,
							debt_asset,
							debt_to_liquidate,
						));
					}
				}
			}

			// revert the price back
			self.update_reserve_price(new_price.0, old_price);

			Some(liquidation_options)
		}

		/// Evaluates all liquidation options and return the one that is closest to the `target_health_factor`.
		pub fn get_best_liquidation_option(
			&mut self,
			user_data: &UserData,
			target_health_factor: U256,
			new_price: (EvmAddress, U256),
		) -> Option<LiquidationOption> {
			let mut liquidation_options =
				self.calculate_liquidation_options(user_data, target_health_factor, new_price)?;

			// TODO: find better criteria for determining which liquidation option to choose as the best one
			liquidation_options.sort_by(|a, b| {
				a.health_factor
					.abs_diff(target_health_factor)
					.cmp(&b.health_factor.abs_diff(target_health_factor))
			});

			liquidation_options.first().cloned()
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

	/// Recover signer from EVM transaction.
	fn recover_signer(transaction: &Transaction) -> Option<H160> {
		let mut sig = [0u8; 65];
		let mut msg = [0u8; 32];
		match transaction {
			Transaction::Legacy(t) => {
				sig[0..32].copy_from_slice(&t.signature.r()[..]);
				sig[32..64].copy_from_slice(&t.signature.s()[..]);
				sig[64] = t.signature.standard_v();
				msg.copy_from_slice(&ethereum::LegacyTransactionMessage::from(t.clone()).hash()[..]);
			}
			Transaction::EIP2930(t) => {
				sig[0..32].copy_from_slice(&t.r[..]);
				sig[32..64].copy_from_slice(&t.s[..]);
				sig[64] = t.odd_y_parity as u8;
				msg.copy_from_slice(&ethereum::EIP2930TransactionMessage::from(t.clone()).hash()[..]);
			}
			Transaction::EIP1559(t) => {
				sig[0..32].copy_from_slice(&t.r[..]);
				sig[32..64].copy_from_slice(&t.s[..]);
				sig[64] = t.odd_y_parity as u8;
				msg.copy_from_slice(&ethereum::EIP1559TransactionMessage::from(t.clone()).hash()[..]);
			}
		}
		let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg).ok()?;
		Some(H160::from(H256::from(sp_io::hashing::keccak_256(&pubkey))))
	}

	pub fn verify_signer(transaction: &Transaction, maybe_signer: EvmAddress) -> bool {
		match recover_signer(transaction) {
			Some(signer) => signer == maybe_signer,
			None => false,
		}
	}
}
