use crate::traits::*;
use ethabi::ethereum_types::U512;
use evm::ExitReason;
use fp_evm::{ExitReason::Succeed, ExitSucceed::Returned};
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use primitives::EvmAddress;
use sc_client_api::StorageData;
use sc_client_api::StorageKey;
use sp_core::RuntimeDebug;
use sp_core::H256;
use sp_core::U256;
use sp_runtime::traits::Block;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_runtime::SaturatedConversion;
use std::collections::HashMap;
use std::time::Instant;

use math::*;
use types::*;

pub mod math;
pub mod types;

#[cfg(test)]
mod tests;

const LOG_TARGET: &str = "liquidation-worker";

// Functions prefixed with `fetch_` do external (runtime API) calls. The liquidation close-factor
// constants live in `types.rs`, next to the math that uses them.

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	GetPool = "getPool()",
	GetReservesData = "getReservesData(address)",
	GetPriceOracle = "getPriceOracle()",
	GetAssetPrice = "getAssetPrice(address)",
	Supply = "supply(address,uint256,address,uint16)",
	Withdraw = "withdraw(address,uint256,address)",
	Borrow = "borrow(address,uint256,uint256,uint16,address)",
	GetUserAccountData = "getUserAccountData(address)",
	GetReservesList = "getReservesList()",
	GetReserveData = "getReserveData(address)",
	GetAllReservesTokens = "getAllReservesTokens()",
	GetConfiguration = "getConfiguration(address)",
	GetUserConfiguration = "getUserConfiguration(address)",
	GetUserEMode = "getUserEMode(address)",
	GetEModeCategoryData = "getEModeCategoryData(uint8)",
	ScaledBalanceOf = "scaledBalanceOf(address)",
	BalanceOf = "balanceOf(address)",
	SetValue = "setValue(string,uint128,uint128)",
	SetMultipleValues = "setMultipleValues(string[],uint256[])",
	GetValue = "getValue(string)",
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
	Symbol = "symbol()",
	GetUserReservesData = "getUserReservesData(address,address)",
	AddressesProvider = "ADDRESSES_PROVIDER()",
}

/// Resolves a pool's `PoolAddressesProvider` (Aave v3 pools expose it as a public
/// immutable). Free function because instance discovery needs it before a
/// per-market `Hydration` exists.
pub fn fetch_addresses_provider<B: Block, RA: RuntimeApiProvider<B>>(
	api: &RA,
	block: B::Hash,
	caller: EvmAddress,
	pool: EvmAddress,
) -> Result<EvmAddress, Error> {
	let data = Into::<u32>::into(Function::AddressesProvider).to_be_bytes().to_vec();
	// The pool sits behind a proxy: the delegatecall hop pushes the view call just past
	// 100k gas (measured ~104k on mainnet), so give it comfortable headroom.
	let gas_limit = U256::from(400_000);
	let res = api.call(block, caller, pool, data, gas_limit)?;

	if res.exit_reason != Succeed(Returned) {
		return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
	}

	if res.value.len() < 32 {
		return Err(Error::DecodeInvalidLength);
	}

	Ok(EvmAddress::from_slice(&res.value[12..32]))
}

pub mod traits {
	use super::*;

	#[derive(RuntimeDebug)]
	pub enum RuntimeApiErr {
		Api(sp_api::ApiError),
		Dispatch(DispatchError),
		EvmExitReason(ExitReason),
	}

	impl From<DispatchError> for RuntimeApiErr {
		fn from(e: DispatchError) -> Self {
			Self::Dispatch(e)
		}
	}

	impl From<ExitReason> for RuntimeApiErr {
		fn from(e: ExitReason) -> Self {
			Self::EvmExitReason(e)
		}
	}

	impl From<sp_api::ApiError> for RuntimeApiErr {
		fn from(e: sp_api::ApiError) -> Self {
			Self::Api(e)
		}
	}

	pub trait RuntimeClient<B: Block> {
		fn storage(&self, hash: B::Hash, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>>;
	}

	pub trait RuntimeApiProvider<B: Block> {
		fn call(
			&self,
			block: B::Hash,
			from: EvmAddress,
			to: EvmAddress,
			data: Vec<u8>,
			gas_limit: U256,
		) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr>;

		fn address_to_asset(&self, block: B::Hash, address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr>;

		fn minimum_balance(&self, block: B::Hash, asset_id: AssetId) -> Result<Balance, RuntimeApiErr>;

		fn timestamp(&self, block: B::Hash) -> Option<Timestamp>;
	}
}

pub struct Hydration {
	pub caller: EvmAddress,
	pub pool_address_provider: EvmAddress,
	pub log_prefix: String,
}

impl Hydration {
	pub fn new(caller: EvmAddress, pool_address_provider: EvmAddress, log_prefix: &str) -> Self {
		Self {
			caller,
			pool_address_provider,
			log_prefix: log_prefix.to_string(),
		}
	}

	/// Function fetches and returns money market data for PEPL purposes
	pub fn fetch_money_market<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
	) -> Option<MoneyMarket> {
		let timer = Instant::now();
		log::trace!(target: LOG_TARGET, "{:?}: fetch_money_market()", self.log_prefix);

		//NOTE: these 2 doesn't have to be fetched always. We can fetch them on node startup and
		//just use these addresses.
		let pool = self.fetch_pool(api, block).inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch pool's address, err: {:?}, duration: {:?}", self.log_prefix, e, timer.elapsed().as_nanos());
		}).ok()?;

		let oracle = self.fetch_price_oracle(api, block).inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch oracle's address, err: {:?} duration: {:?}", self.log_prefix, e, timer.elapsed().as_nanos());
		}).ok()?;

		let reserves_list = self.fetch_reserves_list(api, block, pool).inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch reserves list, err: {:?} duration: {:?}", self.log_prefix, e, timer.elapsed().as_nanos());
		}).ok()?;

		// A reserve that fails to load must NOT abort the whole money market — one broken reserve
		// would otherwise disable ALL liquidations. Poison its index instead; borrowers holding a
		// poisoned reserve are skipped in `fetch_borrower`.
		let mut reserves = HashMap::with_capacity(reserves_list.len());
		let mut poisoned = Vec::new();
		for (idx, reserve_addr) in reserves_list.into_iter().enumerate() {
			let Ok(reserve) = self.fetch_reserve_data(api, block, pool, reserve_addr).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch reserve data, reserve: {:?}, err: {:?}, duration: {:?}", self.log_prefix, reserve_addr, e, timer.elapsed().as_nanos());
			}) else {
				poisoned.push(idx);
				continue;
			};

			let Ok(symbol) = self.fetch_symbol(api, block, reserve_addr).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch reserve's symbol, reserve: {:?}, err: {:?}, duration: {:?}", self.log_prefix, reserve_addr, e, timer.elapsed().as_nanos());
			}) else {
				poisoned.push(idx);
				continue;
			};

			let Ok(Some(asset_id)) = api.address_to_asset(block, reserve_addr).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to convert reserve to asset id, reserve: {:?}, symbol: {:?}, err: {:?}, duration: {:?}", self.log_prefix, reserve_addr, symbol, e, timer.elapsed().as_nanos());
			}) else {
				log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): no asset id for reserve, reserve: {:?}, symbol: {:?}, duration: {:?}", self.log_prefix, reserve_addr, symbol, timer.elapsed().as_nanos());
				poisoned.push(idx);
				continue;
			};

			let Ok(existential_deposit) = api.minimum_balance(block, asset_id).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to get reserve's existential deposit, reserve: {:?}, symbol: {:?}, asset_id: {:?}, err: {:?}, duration: {:?}", self.log_prefix, reserve_addr, symbol, asset_id, e, timer.elapsed().as_nanos());
			}) else {
				poisoned.push(idx);
				continue;
			};

			let emode = if let Some(emode_id) = reserve.emode_id() {
				let Ok(emode) = self.fetch_emode_category(api, block, pool, emode_id).inspect_err(|e| {
					log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch emode data, emode_id: {:?}, err: {:?}, duration: {:?}", self.log_prefix, emode_id, e, timer.elapsed().as_nanos());
				}) else {
					poisoned.push(idx);
					continue;
				};

				Some(emode)
			} else {
				None
			};

			let Ok(price) = self.fetch_asset_price(api, block, oracle, reserve_addr).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): failed to fetch reserves's price, reserve: {:?}, symbol: {:?}, err: {:?}, duration: {:?}", self.log_prefix, reserve_addr, symbol,e, timer.elapsed().as_nanos());
			}) else {
				poisoned.push(idx);
				continue;
			};

			reserves.insert(
				reserve_addr,
				Reserve {
					idx,
					data: reserve,
					address: reserve_addr,
					asset_id,
					symbol,
					price,
					existential_deposit,
					emode,
				},
			);
		}

		if !poisoned.is_empty() {
			log::error!(target: LOG_TARGET, "{:?}: fetch_money_market(): {} reserve(s) failed to load and are poisoned; borrowers holding them will be skipped, indices: {:?}", self.log_prefix, poisoned.len(), poisoned);
		}

		log::debug!(target: LOG_TARGET, "{:?}: fetch_money_market(): finished, duration: {:?}", self.log_prefix, timer.elapsed().as_nanos());
		Some(MoneyMarket {
			pool,
			oracle,
			reserves,
			poisoned,
		})
	}

	/// Function loads borrower's data from the money market contracts.
	pub fn fetch_borrower<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		block_number: BlockNumber,
		mm: &MoneyMarket,
		who: EvmAddress,
		now: Timestamp,
	) -> Option<Borrower> {
		let timer = Instant::now();
		log::trace!(target: LOG_TARGET, "{:?}: fetch_borrower()", self.log_prefix);

		let configuration = self.fetch_borrower_configuration(api, block, mm.pool, who).inspect_err(|e| {
		log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): failed to fetch borrower's configuration data, who: {:?}, err: {:?}, duration: {:?}", self.log_prefix, who, e, timer.elapsed().as_nanos());
	}).ok()?;

		// Never compute HF on a partially-loaded market: a borrower holding a poisoned reserve
		// would be silently mispriced. Skip them (loudly) until the reserve loads again.
		if configuration.uses_any(&mm.poisoned) {
			log::warn!(target: LOG_TARGET, "{:?}: fetch_borrower(): borrower holds a reserve that failed to load, skipping, who: {:?}, poisoned: {:?}", self.log_prefix, who, mm.poisoned);
			return None;
		}

		let mut total_debt = U256::zero();
		let mut total_collateral = U256::zero();

		//NOTE: we are using option so we can access reserves by index (poisoned indices stay None)
		let mut reserves: Vec<Option<UserReserve>> = vec![None; mm.reserve_count()];
		for (addr, r) in &mm.reserves {
			let coll: U256 = if configuration.is_collateral(r.idx) {
				self.fetch_borrower_collateral_and_convert_to_base(api, block, who, r, now).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): failed to fetch borrower's collateral for reserve, who: {:?}, reserve: {:?}, symbol: {:?}, err: {:?}, duration: {:?}", self.log_prefix, who, addr, r.symbol, e, timer.elapsed().as_nanos());
			}).ok()?
			} else {
				Zero::zero()
			};

			let debt: U256 = if configuration.is_debt(r.idx) {
				self.fetch_borrower_debt_and_convert_to_base(api, block, who, r, now).inspect_err(|e| {
				log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): failed to fetch borrower's debt for reserve, who: {:?}, reserve: {:?}, symbol: {:?}, err: {:?}, duration: {:?}", self.log_prefix, who, addr, r.symbol, e, timer.elapsed().as_nanos());
			}).ok()?
			} else {
				Zero::zero()
			};

			//NOTE: `reserves` is pre-filled with `None` so this is ok
			if !coll.is_zero() || !debt.is_zero() {
				if r.idx >= reserves.len() {
					log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): index is out of bound. THIS SHOULD NEVER HAPPEN, please report to maintainers, reserve: {:?}, symbol: {:?}, reserve_idx: {:?}, reserves_count: {:?}, duration: {:?}", self.log_prefix, addr, r.symbol, r.idx, reserves.len(), timer.elapsed().as_nanos());
					return None;
				}

				total_debt = if let Some(td) = total_debt.checked_add(debt) {
					td
				} else {
					log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): total_debt calculation overflowed, who: {:?}, duration: {:?}", self.log_prefix, who, timer.elapsed().as_nanos());
					return None;
				};

				total_collateral = if let Some(tc) = total_collateral.checked_add(coll) {
					tc
				} else {
					log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): total_collateral calculation overflowed, who: {:?}, duration: {:?}", self.log_prefix, who, timer.elapsed().as_nanos());
					return None;
				};

				reserves[r.idx] = Some(UserReserve { collateral: coll, debt });
			}
		}

		let emode_id = self.fetch_user_emode_id(api, block, mm.pool, who).inspect_err(|e| {
		log::error!(target: LOG_TARGET, "{:?}: fetch_borrower(): failed to fetch borrower's emode data, who: {:?}, err: {:?}, duration: {:?}", self.log_prefix, who, e, timer.elapsed().as_nanos());
	}).ok()?;

		let emode_id = if emode_id.is_zero() { None } else { Some(emode_id) };

		log::trace!(target: LOG_TARGET, "{:?}: fetch_borrower(): finished, duration: {:?}", self.log_prefix, timer.elapsed().as_nanos());
		Some(Borrower {
			configuration,
			address: who,
			reserves,
			emode_id,
			updated_at: block_number,
			total_collateral,
			total_debt,
		})
	}

	pub fn fetch_borrower_collateral_and_convert_to_base<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		who: EvmAddress,
		reserve: &Reserve,
		now: Timestamp,
	) -> Result<U256, Error> {
		let b = self.fetch_scaled_balance_of(api, block, who, reserve.data.a_token_address)?;

		if b.is_zero() {
			return Ok(b);
		}

		let Some(norm_income) = reserve.get_normalized_income(now) else {
			return Err(Error::Arithmetic("normalized income calculation overflow"));
		};

		convert_to_base_normalized(b, norm_income, reserve)
			.ok_or(Error::Arithmetic("convert to base calculation overflow"))
	}

	// Function fetches and convert variable debt to [BASE] currency.
	// NOTE: stable debt is deprecated and we are not using it so this function doesn't account for it.
	fn fetch_borrower_debt_and_convert_to_base<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		who: EvmAddress,
		reserve: &Reserve,
		now: Timestamp,
	) -> Result<U256, Error> {
		let b = self.fetch_scaled_balance_of(api, block, who, reserve.data.variable_debt_token_address)?;

		if b.is_zero() {
			return Ok(b);
		};

		let Some(norm_debt) = reserve.get_normalized_debt(now) else {
			return Err(Error::Arithmetic("normalized debt calculation overflow"));
		};

		convert_to_base_normalized(b, norm_debt, reserve)
			.ok_or(Error::Arithmetic("convert to base calculation overflow"))
	}

	fn fetch_scaled_balance_of<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		user: EvmAddress,
		token: EvmAddress,
	) -> Result<U256, Error> {
		let mut data = Into::<u32>::into(Function::ScaledBalanceOf).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(user).as_bytes());

		let gas_limit = U256::from(500_000);
		let res = api.call(block, self.caller, token, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		if res.value.len() < 32 {
			return Err(Error::DecodeInvalidLength);
		}

		Ok(U256::from_big_endian(&res.value[0..32]))
	}

	/// Function fetches and returns Bitmap of the users collaterals and borrows.
	/// It is divided into pairs of bits, one pair per asset.
	/// The first bit indicates if an asset is used as collateral by the user, the second whether an asset is borrowed by the user.
	/// The corresponding assets are in the same position as `getReservesList()`.
	/// Calls Runtime API.
	fn fetch_borrower_configuration<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		contract: EvmAddress,
		who: EvmAddress,
	) -> Result<UserConfiguration, Error> {
		let mut data = Into::<u32>::into(Function::GetUserConfiguration).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(who).as_bytes());

		let gas_limit = U256::from(500_000);
		let res = api.call(block, self.caller, contract, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		};

		if res.value.len() < 32 {
			return Err(Error::DecodeInvalidLength);
		};

		Ok(UserConfiguration(U256::from_big_endian(&res.value[0..32])))
	}

	/// Function fetches and returns eMode the user is using. 0 is a non-eMode category.
	fn fetch_user_emode_id<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		pool: EvmAddress,
		user: EvmAddress,
	) -> Result<U256, Error> {
		let mut data = Into::<u32>::into(Function::GetUserEMode).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(user).as_bytes());

		let gas_limit = U256::from(200_000);
		let res = api.call(block, self.caller, pool, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		if res.value.len() < 32 {
			return Err(Error::DecodeInvalidLength);
		};

		Ok(U256::from_big_endian(&res.value[0..32]))
	}

	/// Function fetches and returns reserve's data
	fn fetch_reserve_data<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		pool: EvmAddress,
		reserve: EvmAddress,
	) -> Result<ReserveData, Error> {
		let mut data = Into::<u32>::into(Function::GetReserveData).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(reserve).as_bytes());

		let gas_limit = U256::from(500_000);
		let res = api.call(block, self.caller, pool, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		};

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
			&res.value,
		)?;

		ReserveData::new(&decoded).ok_or(Error::TypeDecode("ReservedData"))
	}

	pub fn fetch_pool<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
	) -> Result<EvmAddress, Error> {
		let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();
		let gas_limit = U256::from(100_000);
		let res = api.call(block, self.caller, self.pool_address_provider, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		if res.value.len() < 32 {
			return Err(Error::DecodeInvalidLength);
		}

		Ok(EvmAddress::from_slice(&res.value[12..32]))
	}

	fn fetch_price_oracle<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
	) -> Result<EvmAddress, Error> {
		let data = Into::<u32>::into(Function::GetPriceOracle).to_be_bytes().to_vec();
		let gas_limit = U256::from(100_000);

		let res = api.call(block, self.caller, self.pool_address_provider, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		if res.value.len() < 32 {
			return Err(Error::DecodeInvalidLength);
		}

		Ok(EvmAddress::from_slice(&res.value[12..32]))
	}

	fn fetch_emode_category<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		pool: EvmAddress,
		emode_category_id: EmodeId,
	) -> Result<EModeCategory, Error> {
		let emode_id: u64 = emode_category_id.saturated_into();
		let mut data = Into::<u32>::into(Function::GetEModeCategoryData).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from_low_u64_be(emode_id).as_bytes());

		let gas_limit = U256::from(200_000);
		let res = api.call(block, self.caller, pool, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		let decoded = ethabi::decode(
			&[ethabi::ParamType::Tuple(vec![
				ethabi::ParamType::Uint(2), // ltv
				ethabi::ParamType::Uint(2), // liquidationThreshold
				ethabi::ParamType::Uint(2), // liquidationBonus
				ethabi::ParamType::Address, // priceSource
				ethabi::ParamType::String,  // label
			])],
			&res.value,
		)?;

		EModeCategory::new(&decoded).ok_or(Error::TypeDecode("EModeCategory"))
	}

	fn fetch_asset_price<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		oracle: EvmAddress,
		asset: EvmAddress,
	) -> Result<U256, Error> {
		let mut data = Into::<u32>::into(Function::GetAssetPrice).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(asset).as_bytes());

		let gas_limit = U256::from(500_000);
		let res = api.call(block, self.caller, oracle, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		};

		if res.value.len() < 32 {
			return Err(Error::DecodeInvalidLength);
		}

		Ok(U256::from_big_endian(&res.value[0..32]))
	}

	/// Get the list of reserve assets.
	/// Calls Runtime API.
	fn fetch_reserves_list<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		pool: EvmAddress,
	) -> Result<Vec<EvmAddress>, Error> {
		let data = Into::<u32>::into(Function::GetReservesList).to_be_bytes().to_vec();
		let gas_limit = U256::from(500_000);

		let res = api.call(block, self.caller, pool, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		let decoded = ethabi::decode(
			&[ethabi::ParamType::Array(Box::new(ethabi::ParamType::Address))],
			&res.value,
		)?;

		let decoded = decoded[0].clone().into_array().ok_or(ethabi::Error::InvalidData)?;

		let mut reserves = Vec::with_capacity(decoded.len());
		for addr in decoded.iter() {
			reserves.push(
				addr.clone()
					.into_address()
					.ok_or(Error::TypeDecode("reserve into address"))?,
			);
		}

		Ok(reserves)
	}

	fn fetch_symbol<B: Block, RA: RuntimeApiProvider<B>>(
		&self,
		api: &RA,
		block: B::Hash,
		asset: EvmAddress,
	) -> Result<Symbol, Error> {
		let data = Into::<u32>::into(Function::Symbol).to_be_bytes().to_vec();
		let gas_limit = U256::from(500_000);

		let res = api.call(block, self.caller, asset, data, gas_limit)?;

		if res.exit_reason != Succeed(Returned) {
			return Err(RuntimeApiErr::EvmExitReason(res.exit_reason).into());
		}

		let decoded = ethabi::decode(&[ethabi::ParamType::String], &res.value)?;

		decoded[0]
			.clone()
			.into_string()
			.ok_or(Error::TypeDecode("symbol into string"))
	}
}

#[inline(always)]
fn convert_to_base_normalized(n: U256, norm_multiplier: U256, r: &Reserve) -> Option<U256> {
	ray_mul(n, norm_multiplier)?
		.full_mul(r.price)
		.checked_div(U512::from(10u128.pow(r.decimals() as u32)))?
		.try_into()
		.ok()
}
