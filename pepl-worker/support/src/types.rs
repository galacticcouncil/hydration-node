use crate::math::percent_mul;
use crate::math::ray_mul;
use crate::math::wad_div;
use crate::math::OCTILL;
use crate::traits::RuntimeApiErr;
use crate::traits::RuntimeApiProvider;
use crate::LOG_TARGET;
use ethabi::ethereum_types::U512;
use fp_rpc::EthereumRuntimeRPCApi;
use hydradx_runtime::evm::precompiles::erc20_mapping::Erc20MappingApi;
use pallet_currencies_rpc_runtime_api::CurrenciesApi;
use primitives::AccountId;
use primitives::EvmAddress;
use sc_client_api::StorageData;
use sc_client_api::StorageKey;
use sc_client_api::{Backend, StorageProvider};
use sp_api::ProvideRuntimeApi;
use sp_core::RuntimeDebug;
use sp_core::U256;
use sp_runtime::traits::Block;
use sp_runtime::traits::Zero;
use sp_std::ops::BitAnd;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

const SECS_PER_YEAR: u32 = 365 * 24 * 60 * 60;
const ONE_HF: u128 = 10u128.pow(18);
const ORACLE_DECIMALS: u8 = 8;
const PERCENTAGE_FACTOR: u128 = 10u128.pow(4);

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

pub type Balance = u128;
pub type AssetId = u32;
pub type Timestamp = u64;
pub type Symbol = String;
pub type EmodeId = U256;
pub type BlockNumber = u32;

/// Collateral and debt amounts of a reserve in the base currency.
#[derive(Default, Eq, PartialEq, RuntimeDebug, Clone)]
pub struct UserReserve {
	pub collateral: U256,
	pub debt: U256,
}

/// User's data. The state is not automatically updated. Any change in the chain can invalidate the data stored in the struct.
#[derive(RuntimeDebug, PartialEq, Eq, Clone)]
pub struct Borrower {
	pub configuration: UserConfiguration,
	pub address: EvmAddress,
	pub reserves: Vec<Option<UserReserve>>,
	pub emode_id: Option<EmodeId>,
	pub total_debt: U256,
	pub total_collateral: U256,
	pub updated_at: BlockNumber,
}

impl Borrower {
	/// Calculates user's health factor.
	pub fn calc_health_factor(&self, money_market: &MoneyMarket) -> Result<U256, Error> {
		// No debt -> nothing to liquidate; mirrors Aave's `type(uint256).max` health factor.
		if self.total_debt.is_zero() {
			return Ok(U256::MAX);
		}

		// Debt with no collateral (e.g. a simulated full seize) is maximally unhealthy, not an
		// error — erroring here silently dropped the only viable liquidation option for deeply
		// underwater borrowers in `calculate_liquidation_options`.
		if self.total_collateral.is_zero() {
			return Ok(U256::zero());
		}

		let mut avg_liq_threshold = U256::zero();

		for r in money_market.reserves.values() {
			let Some(user_reserve) = self.reserves.get(r.idx) else {
				return Err(Error::UnexpectedError(
					"reserves[idx] out of bounds. THIS SHOULD NEVER HAPPEN, please contact project's maintainers",
				));
			};

			let Some(user_reserve) = user_reserve else {
				//Nothing to do if user doesn't have this reserve
				continue;
			};

			let reserve_liq_threshold: U256 =
				r.liquidation_threshold(self.emode_id.is_some() && self.emode_id == r.data.emode_id());
			avg_liq_threshold = avg_liq_threshold
				.checked_add(
					user_reserve
						.collateral
						.checked_mul(reserve_liq_threshold)
						.ok_or(Error::Arithmetic("Overflow"))?,
				)
				.ok_or(Error::Arithmetic("Overflow"))?;
		}

		avg_liq_threshold = avg_liq_threshold
			.checked_div(self.total_collateral)
			.ok_or(Error::Arithmetic("avg_liq_threshold / total_collateral failed"))?;

		let nominator = percent_mul(self.total_collateral, avg_liq_threshold).ok_or(Error::Arithmetic("Overflow"))?;
		wad_div(nominator, self.total_debt).ok_or(Error::Arithmetic("Overflow"))
	}

	/// Function substracts `amount` from borrower's reserve under index `idx` and updates total debt
	/// or total collateral amount.
	/// Function returns `None` if `idx` is not valid reserve index.
	pub fn update_reserve(&mut self, idx: usize, opp: ReserveOpp) -> Option<()> {
		let Some(Some(r)) = self.reserves.get_mut(idx) else {
			return None;
		};

		match opp {
			ReserveOpp::SubDebt(amt) => {
				r.debt = r.debt.saturating_sub(amt);
				self.total_debt = self.total_debt.saturating_sub(amt)
			}
			ReserveOpp::SubCollateral(amt) => {
				r.collateral = r.collateral.saturating_sub(amt);
				self.total_collateral = self.total_collateral.saturating_sub(amt)
			}
		}
		Some(())
	}

	//Function returns `true` if borrower has any of `reserves`.
	//
	// `reserves`: vec of reserves' indexes in `MoneyMarket.reserves`
	pub fn has_reserve(&self, reserves: Vec<usize>) -> bool {
		for idx in reserves {
			if let Some(Some(_)) = self.reserves.get(idx) {
				return true;
			}
		}

		false
	}
}

#[derive(Eq, PartialEq, RuntimeDebug)]
pub enum ReserveOpp {
	SubCollateral(U256),
	SubDebt(U256),
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct EModeCategory {
	pub liquidation_threshold: u16,
	pub liquidation_bonus: u16,
}

impl EModeCategory {
	pub fn new(data: &[ethabi::Token]) -> Option<Self> {
		let data_tuple = data.first()?.clone().into_tuple()?;

		Some(Self {
			#[allow(clippy::get_first)]
			liquidation_threshold: data_tuple.get(1)?.clone().into_uint()?.try_into().ok()?,
			liquidation_bonus: data_tuple.get(2)?.clone().into_uint()?.try_into().ok()?,
		})
	}
}

/// The configuration of the user across all the reserves.
/// Bitmap of the users collaterals and borrows. It is divided into pairs of bits, one pair per asset.
/// The first bit indicates if the user uses an asset as collateral, the second whether the user borrows an asset.
/// The corresponding assets are in the same position as `fetch_reserves_list()`.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct UserConfiguration(pub U256);
impl UserConfiguration {
	/// Returns `true` if the user uses the asset as collateral.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_collateral(&self, asset_index: usize) -> bool {
		let bit_mask = U256::from(2) << (2 * asset_index);
		!(self.0 & bit_mask).is_zero()
	}

	/// Returns `true` if the user uses the asset as debt.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_debt(&self, asset_index: usize) -> bool {
		let bit_mask = U256::from(1) << (2 * asset_index);
		!(self.0 & bit_mask).is_zero()
	}

	/// Returns `true` if the user uses any of the reserves (by index) as collateral or debt.
	pub fn uses_any(&self, indices: &[usize]) -> bool {
		indices.iter().any(|&idx| self.is_collateral(idx) || self.is_debt(idx))
	}
}

/// Configuration of the reserve.
/// https://github.com/aave/aave-v3-core/blob/782f51917056a53a2c228701058a6c3fb233684a/contracts/protocol/libraries/types/DataTypes.sol#L5
/// Not all data fields are used.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct ReserveData {
	pub configuration: U256, // https://github.com/aave-dao/aave-v3-origin/blob/3aad8ca184159732e4b3d8c82cd56a8707a106a2/src/core/contracts/protocol/libraries/types/DataTypes.sol#L79
	pub liquidity_index: u128,
	pub current_liquidity_rate: u128,
	pub variable_borrow_index: u128,
	pub current_variable_borrow_rate: u128,
	pub last_update_timestamp: u64,
	pub a_token_address: EvmAddress,
	pub stable_debt_token_address: EvmAddress,
	pub variable_debt_token_address: EvmAddress,
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

	/// Get eMode category of the reserve.
	pub fn emode_id(&self) -> Option<U256> {
		// bits [168..175]
		let r = self.configuration.byte(21);
		if r == 0 {
			return None;
		}

		Some(r.into())
	}
}

/// State of asset reserve.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Reserve {
	//index in original mm constract's array. It's used to access e.g. `UserReserve` by index without hashing
	pub idx: usize,
	pub data: ReserveData,
	pub address: EvmAddress,
	pub asset_id: AssetId,
	pub symbol: Symbol,
	pub price: U256,
	pub existential_deposit: Balance,
	pub emode: Option<EModeCategory>,
}

impl Reserve {
	pub fn get_normalized_income(&self, now: Timestamp) -> Option<U256> {
		if now == self.data.last_update_timestamp {
			return Some(U256::from(self.data.liquidity_index));
		}

		let current_liquidity_rate = U256::from(self.data.current_liquidity_rate);
		let timestamp_diff = U256::from(now.checked_sub(self.data.last_update_timestamp)?);

		let nominator: U256 = current_liquidity_rate.checked_mul(timestamp_diff)?;

		let seconds_per_year = U256::from(SECS_PER_YEAR);
		let ray = U256::from(OCTILL);

		let linear_interest = nominator.checked_div(seconds_per_year)?.checked_add(ray)?;

		ray_mul(linear_interest, self.data.liquidity_index.into())
	}

	pub fn get_normalized_debt(&self, now: Timestamp) -> Option<U256> {
		let variable_borrow_index = U256::from(self.data.variable_borrow_index);
		if now == self.data.last_update_timestamp {
			return Some(variable_borrow_index);
		}

		let exp = U256::from(now.checked_sub(self.data.last_update_timestamp)?);

		let ray = U256::from(OCTILL);

		if exp.is_zero() {
			return Some(ray);
		}

		let exp_minus_one = exp.checked_sub(U256::from(1))?;

		let exp_minus_two = exp.saturating_sub(U256::from(2));

		let seconds_per_year = U256::from(SECS_PER_YEAR);

		let rate = U256::from(self.data.current_variable_borrow_rate);

		let base_power_two = ray_mul(rate, rate)?.checked_div(seconds_per_year * seconds_per_year)?;

		let base_power_three = ray_mul(base_power_two, rate)?.checked_div(seconds_per_year)?;

		let second_term = exp.full_mul(exp_minus_one).checked_mul(base_power_two.into())? / 2;

		let third_term = exp
			.full_mul(exp_minus_one)
			.checked_mul(exp_minus_two.into())?
			.checked_mul(base_power_three.into())?
			/ 6;

		let compound_interest = rate
			.full_mul(exp)
			.checked_div(seconds_per_year.into())?
			.checked_add(ray.into())?
			.checked_add(second_term)?
			.checked_add(third_term)?
			.try_into()
			.ok()?;

		ray_mul(compound_interest, variable_borrow_index)
	}

	/// Get the number of decimals of the reserve.
	pub fn decimals(&self) -> u8 {
		// bits [48..55]
		self.data.configuration.byte(6)
	}

	/// Returns liquidation threshold or emode liquidation threshold of the reserve  if `emode` is
	/// `true`
	/// WARN: fn returns normale liq. threshold if reserve doesn't have emode data even if `emode` is
	/// `true`.
	pub fn liquidation_threshold(&self, emode: bool) -> U256 {
		if emode {
			if let Some(e) = &self.emode {
				return e.liquidation_threshold.into();
			}
		}

		// bits [16..31]
		(self.data.configuration.low_u32() >> 16).into()
	}

	/// Function returns liquidation bons or emode liq. bonus of the reserve if `emode` is `true`
	/// WARN: liq. bonus is returned reserve doesn't have emode data even if `emode` is `true`.
	pub fn liquidation_bonus(&self, emode: bool) -> U256 {
		if emode {
			if let Some(e) = &self.emode {
				return e.liquidation_bonus.into();
			}
		}

		// bits [32..47]
		let shifted_config = self.data.configuration >> 32;

		let bit_mask: u32 = 0b0000_0000_0000_0000_1111_1111_1111_1111;
		shifted_config.low_u32().bitand(bit_mask).into()
	}
}

/// Captures the state of the money market related to liquidations.
/// The state is not automatically updated. Any change in the chain can invalidate the data stored in the struct.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct MoneyMarket {
	pub pool: EvmAddress,
	pub oracle: EvmAddress,
	pub reserves: HashMap<EvmAddress, Reserve>,
	/// Reserve-list indices that failed to load (`fetch_money_market`). One broken reserve must
	/// not disable ALL liquidations; instead, borrowers holding a poisoned reserve are skipped
	/// (`fetch_borrower`) — computing their HF on a partially-loaded market would misprice them.
	pub poisoned: Vec<usize>,
}

impl MoneyMarket {
	/// Total on-chain reserve count, including reserves that failed to load.
	pub fn reserve_count(&self) -> usize {
		self.reserves.len() + self.poisoned.len()
	}

	/// Evaluates all liquidation options and returns one that is closest to the `target_health_factor`.
	/// `borrower` - borrower's data, generated from the `MoneyMarket` with updated price.
	/// `target_health_factor` - 18 decimal places.
	///
	/// Return the amount of debt asset that needs to be liquidated to get the HF to `target_health_factor`.
	pub fn calc_best_liquidation_option_for(
		&self,
		borrower: &Borrower,
		target_health_factor: U256,
		log_prefix: &str,
	) -> Result<Option<LiquidationOption>, Error> {
		let liq_opts = self.calculate_liquidation_options(borrower, target_health_factor, log_prefix)?;

		Ok(select_best_liquidation_option(liq_opts, target_health_factor))
	}

	/// Calculate liquidation options based on the user's reserve, price update and target health factor.
	/// Liquidation options are calculated for all collateral/debt asset pairs.
	/// `borrower` - borrowers's data generated from the `MoneyMarket` with updated price.
	/// `target_health_factor` - 18 decimal places.
	/// `updated_assets` - Skips the calculation if none of the assets is borrower's collateral or borrow asset.
	///     Use `None` to disable this check.
	///
	/// Return the amount of debt asset that needs to be liquidated to get the HF to `target_health_factor`
	pub fn calculate_liquidation_options(
		&self,
		borrower: &Borrower,
		target_health_factor: U256,
		log_prefix: &str,
	) -> Result<Vec<LiquidationOption>, Error> {
		let mut liq_options = Vec::with_capacity(self.reserves.len() / 2);

		let mut coll_reserves = Vec::<(usize, EvmAddress, &Reserve)>::with_capacity(self.reserves.len());
		let mut debt_reserves = Vec::<(usize, EvmAddress, &Reserve)>::with_capacity(self.reserves.len());

		for (addr, r) in &self.reserves {
			let Some(Some(u_reserve)) = borrower.reserves.get(r.idx) else {
				continue;
			};

			if !u_reserve.collateral.is_zero() {
				coll_reserves.push((r.idx, *addr, r));
			}
			if !u_reserve.debt.is_zero() {
				debt_reserves.push((r.idx, *addr, r));
			}
		}

		// Calculate the amount of debt that needs to be liquidated to get the HF closer
		// to `target_health_factor`. Calculated for all combinations of collateral and debt assets.
		for (d_idx, d_addr, d) in &debt_reserves {
			for (c_idx, c_addr, c) in &coll_reserves {
				let Ok(l) =  self.calc_debt_to_liquidate(borrower, target_health_factor, c, d).inspect_err(|e| {
					log::warn!(target: LOG_TARGET, "{:?} calculate_liquidation_options(): failed to calc. liquidation option, who: {:?}, collateral: {:?}, debt: {:?}, err: {:?}", log_prefix, borrower.address, c_addr, d_addr, e);
				}) else {
					continue;
				};

				let mut tmp_borrower = borrower.clone();
				tmp_borrower
					.update_reserve(*c_idx, ReserveOpp::SubCollateral(l.collateral_in_base_currency))
					.ok_or(Error::ReserveNotFound(c.address))?;

				tmp_borrower
					.update_reserve(*d_idx, ReserveOpp::SubDebt(l.debt_in_base_currency))
					.ok_or(Error::ReserveNotFound(d.address))?;

				if let Ok(health_factor) = tmp_borrower.calc_health_factor(self) {
					liq_options.push(LiquidationOption {
						health_factor,
						collateral_asset: *c_addr,
						debt_asset: *d_addr,
						debt_to_liquidate: l.debt_amount,
					});
				}
			}
		}

		Ok(liq_options)
	}

	/// The formula:
	/// `debt_to_liquidate = (THF * Td - Sum(Ci * Pci * LTi)) / (Pd * (THF - LB * LTc))`
	/// where
	///    `THF` - target health factor
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
	pub fn calc_debt_to_liquidate(
		&self,
		borrower: &Borrower,
		target_health_factor: U256,
		collateral: &Reserve,
		debt: &Reserve,
	) -> Result<LiquidationAmounts, Error> {
		let percentage_factor = U256::from(PERCENTAGE_FACTOR);

		let mut weighted_total_collateral = U256::zero();
		for r in self.reserves.values() {
			let Some(Some(u_reserve)) = borrower.reserves.get(r.idx) else {
				continue;
			};

			let lt = r.liquidation_threshold(borrower.emode_id.is_some() && borrower.emode_id == r.data.emode_id());
			weighted_total_collateral = weighted_total_collateral
				.checked_add(
					u_reserve
						.collateral
						.checked_mul(lt)
						.ok_or(Error::Arithmetic("Overflow"))?,
				)
				.ok_or(Error::Arithmetic("Overflow"))?;
		}

		let is_coll_emode = borrower.emode_id.is_some() && borrower.emode_id == collateral.data.emode_id();
		let liq_bonus = collateral.liquidation_bonus(is_coll_emode);

		let Some(Some(borrower_coll)) = borrower.reserves.get(collateral.idx) else {
			return Err(Error::UnexpectedError("borrower.reserves[collateral.idx] out of bounds. THIS SHOULD NEVER HAPPEN, please contact project's maintainers"));
		};
		let Some(Some(borrower_debt_reserve)) = borrower.reserves.get(debt.idx) else {
			return Err(Error::UnexpectedError("borrower.reserves[debt.idx] out of bounds. THIS SHOULD NEVER HAPPEN, please contact project's maintainers"));
		};

		// Convert percentage to decimal number
		let weighted_total_collateral = weighted_total_collateral
			.checked_div(percentage_factor)
			.ok_or(Error::Arithmetic("Overflow"))?;

		let coll_liq_threshold = collateral.liquidation_threshold(is_coll_emode);

		let n: U256 = borrower
			.total_debt
			.full_mul(target_health_factor)
			.checked_div(ONE_HF.into())
			.and_then(|r| r.checked_sub(weighted_total_collateral.into()))
			.and_then(|r| r.checked_mul(U512::from(10u128.pow(ORACLE_DECIMALS.into()))))
			.ok_or(Error::Arithmetic("Overflow"))?
			.try_into()
			.map_err(|_| Error::Arithmetic("Overflow"))?;

		let d: U256 = percentage_factor
			.full_mul(target_health_factor)
			.checked_div(ONE_HF.into())
			.ok_or(Error::Arithmetic("Overflow"))?
			.checked_sub(
				liq_bonus
					.full_mul(coll_liq_threshold)
					.checked_div(percentage_factor.into())
					.ok_or(Error::Arithmetic("Overflow"))?,
			)
			.ok_or(Error::Arithmetic("Overflow"))?
			.try_into()
			.map_err(|_| Error::Arithmetic("Overflow"))?;

		let Some(d) = percent_mul(debt.price, d) else {
			return Err(Error::Arithmetic("Overflow"));
		};

		// In debt asset
		let debt_to_liquidate = n.checked_div(d).ok_or(Error::Arithmetic("Overflow"))?;

		// Adjust decimals from `oracle_price_decimals` to `debt_decimals`
		let debt_to_liquidate = if debt.decimals() > ORACLE_DECIMALS {
			debt_to_liquidate
				.checked_mul(U256::from(10).pow((debt.decimals() - ORACLE_DECIMALS).into()))
				.ok_or(Error::Arithmetic("Overflow"))?
		} else {
			debt_to_liquidate
				.checked_div(U256::from(10).pow((ORACLE_DECIMALS - debt.decimals()).into()))
				.ok_or(Error::Arithmetic("Overflow"))?
		};

		// Our calculation provides a theoretical amount that needs to be liquidated to get the HF close to `target_health_factor`.
		// But there is no guarantee that user has required amount of debt and collateral assets.
		// Adjust these amounts based on how much can be actually liquidated.
		let close_factor = if borrower.calc_health_factor(self)? > CLOSE_FACTOR_HF_THRESHOLD.into() {
			DEFAULT_LIQUIDATION_CLOSE_FACTOR
		} else {
			MAX_LIQUIDATION_CLOSE_FACTOR
		}
		.into();

		// In debt asset
		let borrower_debt_amt = borrower_debt_reserve
			.debt
			.full_mul(U256::from(10u128.pow(debt.decimals().into())))
			.checked_div(debt.price.into())
			.ok_or(Error::Arithmetic("Overflow"))?
			.try_into()
			.map_err(|_| Error::Arithmetic("Overflow"))?;

		// Calculate max debt that can be liquidated. Max amount is affected by the close factor and user's total debt amount.
		// In debt asset.
		let max_liquidatable_debt =
			percent_mul(borrower_debt_amt, close_factor).ok_or(Error::Arithmetic("Overflow"))?;

		let mut actual_debt_to_liquidate = max_liquidatable_debt.min(debt_to_liquidate);
		// In collateral asset without the bonus
		let base_collateral_amount: U256 = actual_debt_to_liquidate
			.full_mul(debt.price)
			.checked_div(collateral.price.into())
			.ok_or(Error::Arithmetic("Overflow"))?
			.try_into()
			.map_err(|_| Error::Arithmetic("Overflow"))?;

		let mut base_collateral_amount = if collateral.decimals() > debt.decimals() {
			base_collateral_amount
				.checked_mul(U256::from(10).pow((collateral.decimals() - debt.decimals()).into()))
				.ok_or(Error::Arithmetic("Overflow"))?
		} else {
			base_collateral_amount
				.checked_div(U256::from(10).pow((debt.decimals() - collateral.decimals()).into()))
				.ok_or(Error::Arithmetic("Overflow"))?
		};

		// In collateral asset
		let Some(collateral_amount) = percent_mul(base_collateral_amount, liq_bonus) else {
			return Err(Error::Arithmetic("Overflow"));
		};

		let mut collateral_in_base: U256 = collateral_amount
			.full_mul(collateral.price)
			.checked_div(U512::from(10).pow(collateral.decimals().into()))
			.ok_or(Error::Arithmetic("Overflow"))?
			.try_into()
			.map_err(|_| Error::Arithmetic("Overflow"))?;

		let mut debt_in_base = actual_debt_to_liquidate
			.full_mul(debt.price)
			.checked_div(U512::from(10).pow(debt.decimals().into()))
			.ok_or(Error::Arithmetic("Overflow"))?
			.try_into()
			.map_err(|_| Error::Arithmetic("Overflow"))?;

		// Adjust the liquidation amounts if user doesn't have expected amount of the collateral asset.
		if collateral_in_base > borrower_coll.collateral {
			// In debt asset
			actual_debt_to_liquidate = borrower_coll
				.collateral
				.full_mul(percentage_factor)
				.checked_div(liq_bonus.into())
				.and_then(|r| r.checked_mul(U512::from(10u128.pow(debt.decimals().into()))))
				.and_then(|r| r.checked_div(debt.price.into()))
				.ok_or(Error::Arithmetic("Overflow"))?
				.try_into()
				.map_err(|_| Error::Arithmetic("Overflow"))?;

			// In collateral asset without the bonus
			base_collateral_amount = actual_debt_to_liquidate
				.full_mul(debt.price)
				.checked_div(collateral.price.into())
				.ok_or(Error::Arithmetic("Overflow"))?
				.try_into()
				.map_err(|_| Error::Arithmetic("Overflow"))?;
			base_collateral_amount = if collateral.decimals() > debt.decimals() {
				base_collateral_amount
					.checked_mul(U256::from(10).pow((collateral.decimals() - debt.decimals()).into()))
					.ok_or(Error::Arithmetic("Overflow"))?
			} else {
				base_collateral_amount
					.checked_div(U256::from(10).pow((debt.decimals() - collateral.decimals()).into()))
					.ok_or(Error::Arithmetic("Overflow"))?
			};

			// In collateral asset
			let Some(collateral_amount) = percent_mul(base_collateral_amount, liq_bonus) else {
				return Err(Error::Arithmetic("Overflow"));
			};

			debt_in_base = actual_debt_to_liquidate
				.full_mul(debt.price)
				.checked_div(U512::from(10).pow(debt.decimals().into()))
				.ok_or(Error::Arithmetic("Overflow"))?
				.try_into()
				.map_err(|_| Error::Arithmetic("Overflow"))?;

			collateral_in_base = collateral_amount
				.full_mul(collateral.price)
				.checked_div(U512::from(10).pow(collateral.decimals().into()))
				.ok_or(Error::Arithmetic("Overflow"))?
				.try_into()
				.map_err(|_| Error::Arithmetic("Overflow"))?;
		}

		// Ignore tiny positions that can cause issues with the existential deposit.
		if collateral_amount < collateral.existential_deposit.into()
			|| actual_debt_to_liquidate < debt.existential_deposit.into()
		{
			return Err(Error::LiquidationBelowED);
		}

		Ok(LiquidationAmounts {
			debt_amount: actual_debt_to_liquidate,
			collateral_amount,
			debt_in_base_currency: debt_in_base,
			collateral_in_base_currency: collateral_in_base,
		})
	}

	/// Change the stored price of some reserve asset.
	/// Reserves are not recalculated.
	pub fn update_price(&mut self, asset: EvmAddress, price: U256) -> Result<(), Error> {
		let Some(r) = self.reserves.get_mut(&asset) else {
			return Err(Error::ReserveNotFound(asset));
		};

		r.price = price;
		Ok(())
	}
}

pub struct RuntimeClient<RA, B, BE> {
	c: Arc<RA>,
	_phantom: PhantomData<(B, BE)>,
}

impl<RA, B, BE> RuntimeClient<RA, B, BE>
where
	B: Block,
	RA: ProvideRuntimeApi<B> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
{
	pub fn new(c: Arc<RA>) -> Self {
		Self {
			c,
			_phantom: PhantomData,
		}
	}
}

impl<RA, B, BE> crate::traits::RuntimeClient<B> for RuntimeClient<RA, B, BE>
where
	B: Block,
	RA: ProvideRuntimeApi<B> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
{
	fn storage(&self, hash: B::Hash, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
		self.c.storage(hash, key)
	}
}

pub struct ApiProvider<C>(pub C);
impl<B: Block, C> RuntimeApiProvider<B> for ApiProvider<&C>
where
	C: EthereumRuntimeRPCApi<B> + Erc20MappingApi<B> + CurrenciesApi<B, AssetId, AccountId, Balance>,
{
	fn call(
		&self,
		block: B::Hash,
		from: EvmAddress,
		to: EvmAddress,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr> {
		match self.0.call(
			block,
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
		) {
			Ok(Ok(r)) => Ok(r),
			Ok(Err(e)) => Err(RuntimeApiErr::Dispatch(e)),
			Err(e) => Err(RuntimeApiErr::Api(e)),
		}
	}

	fn minimum_balance(&self, block: B::Hash, asset_id: AssetId) -> Result<Balance, RuntimeApiErr> {
		Ok(self.0.minimum_balance(block, asset_id)?)
	}

	fn address_to_asset(&self, block: B::Hash, address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr> {
		Ok(self.0.address_to_asset(block, address)?)
	}

	fn timestamp(&self, block: <B as Block>::Hash) -> Option<Timestamp> {
		let b = self.0.current_block(block).ok()??;
		// milliseconds to seconds
		b.header.timestamp.checked_div(1_000)
	}
}

/// Selection policy, best first:
/// 1. the option landing closest to `target_hf` from within `[1.0, target_hf]` — heals the
///    position while seizing the least collateral (the partial-to-target design);
/// 2. otherwise the smallest overshoot above the target (e.g. a simulated full debt repay
///    yields `U256::MAX`) — heals, seizing no more than necessary;
/// 3. otherwise (every option leaves HF < 1.0, e.g. close-factor-capped) the highest HF —
///    best effort; the per-block re-scan drives the follow-up round.
pub fn select_best_liquidation_option(
	mut options: Vec<LiquidationOption>,
	target_hf: U256,
) -> Option<LiquidationOption> {
	options.sort_by(|a, b| a.health_factor.cmp(&b.health_factor));

	let one = U256::from(ONE_HF);
	if let Some(best_healthy) = options
		.iter()
		.rev()
		.find(|o| o.health_factor >= one && o.health_factor <= target_hf)
	{
		return Some(best_healthy.clone());
	}

	if let Some(smallest_overshoot) = options.iter().find(|o| o.health_factor > target_hf) {
		return Some(smallest_overshoot.clone());
	}

	options.into_iter().next_back()
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct LiquidationOption {
	pub health_factor: U256,
	pub collateral_asset: EvmAddress,
	pub debt_asset: EvmAddress,
	pub debt_to_liquidate: U256,
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct LiquidationAmounts {
	pub debt_amount: U256,
	pub collateral_amount: U256,
	pub debt_in_base_currency: U256,
	pub collateral_in_base_currency: U256,
}

#[derive(RuntimeDebug)]
pub enum Error {
	RuntimeApi(RuntimeApiErr),
	AbiDecode(ethabi::Error),
	LiquidationBelowED,
	DecodeInvalidLength,
	TypeDecode(&'static str),
	Arithmetic(&'static str),
	UnexpectedError(&'static str),
	ReserveNotFound(EvmAddress),
}

impl From<RuntimeApiErr> for Error {
	fn from(e: RuntimeApiErr) -> Self {
		Self::RuntimeApi(e)
	}
}

impl From<ethabi::Error> for Error {
	fn from(e: ethabi::Error) -> Self {
		Self::AbiDecode(e)
	}
}
