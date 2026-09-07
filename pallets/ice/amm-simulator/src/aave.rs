#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use codec::Encode;
use core::marker::PhantomData;
use ethabi::decode;
use ethabi::ParamType;
use evm::ExitReason;
use evm::ExitSucceed;
use frame_support::ensure;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AmmSimulator, SimulatorError, TradeResult};
use hydradx_traits::evm::CallContext;
use hydradx_traits::router::{PoolEdge, PoolType};
use ice_support::AssetId;
use ice_support::Balance;
use ice_support::Price;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use precompile_utils::evm::writer::EvmDataWriter;
use primitive_types::U256;
use primitives::EvmAddress;
use sp_arithmetic::traits::SaturatedConversion;
use sp_std::boxed::Box;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

pub trait DataProvider {
	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> (ExitReason, Vec<u8>);

	fn borrowing_contract() -> EvmAddress;

	fn address_to_asset(address: EvmAddress) -> Option<AssetId>;

	fn pairs() -> Vec<(AssetId, AssetId)>;
}

const GAS_LIMIT: u64 = 1_000_000;
const LOG_TARGET: &str = "aave_simulator";

#[module_evm_utility_macro::generate_function_selector]
#[derive(Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	// Pool
	Supply = "supply(address,uint256,address,uint16)",
	Withdraw = "withdraw(address,uint256,address)",
	GetReserveData = "getReserveData(address)",
	GetConfiguration = "getConfiguration(address)",
	GetReservesList = "getReservesList()",
	// AToken
	UnderlyingAssetAddress = "UNDERLYING_ASSET_ADDRESS()",
	ScaledTotalSupply = "scaledTotalSupply()",
	// Underlying ERC20
	BalanceOf = "balanceOf(address)",
}

#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub struct ReserveData {
	pub configuration: U256,
	pub liquidity_index: U256,
	pub current_liquidity_rate: U256,
	pub variable_borrow_index: U256,
	pub current_variable_borrow_rate: U256,
	pub current_stable_borrow_rate: U256,
	pub last_update_timestamp: U256,
	pub id: u16,
	pub atoken_address: EvmAddress,
	pub stable_debt_token_address: EvmAddress,
	pub variable_debt_token_address: EvmAddress,
	pub interest_rate_strategy_address: EvmAddress,
	pub accrued_to_treasury: U256,
	pub scaled_total_supply: U256,
	/// Underlying held by the aToken contract. A `withdraw` transfers out of this
	/// balance, so it — not the supply cap — is what bounds the withdraw direction.
	pub available_liquidity: U256,
}

impl ReserveData {
	fn decimals(&self) -> u8 {
		//bit 48-55: Decimals
		let mask = U256::from(0xFF) << 48;
		((self.configuration & mask) >> 48).saturated_into()
	}

	fn supply_cap_raw(&self) -> U256 {
		//bit 116-151 supply cap in whole tokens, supplyCap == 0 => no cap
		let mask = U256::from((1u128 << 36) - 1) << 116;
		(self.configuration & mask) >> 116
	}

	fn supply_cap(&self) -> U256 {
		if self.supply_cap_raw().is_zero() {
			U256::MAX
		} else {
			self.supply_cap_raw().saturating_mul(
				U256::from(10)
					.checked_pow(self.decimals().into())
					.unwrap_or_else(U256::one),
			)
		}
	}

	fn current_supply(&self) -> U256 {
		self.scaled_total_supply
			.saturating_add(self.accrued_to_treasury)
			.saturating_mul(self.liquidity_index)
			/ U256::from(10).pow(27.into())
	}

	fn available_supply(&self) -> U256 {
		self.supply_cap().saturating_sub(self.current_supply())
	}

	fn flag(&self, bit: usize) -> bool {
		!(self.configuration & (U256::one() << bit)).is_zero()
	}

	fn is_active(&self) -> bool {
		//bit 56: IS_ACTIVE
		self.flag(56)
	}

	fn is_frozen(&self) -> bool {
		//bit 57: IS_FROZEN
		self.flag(57)
	}

	fn is_paused(&self) -> bool {
		//bit 60: IS_PAUSED
		self.flag(60)
	}

	/// `ValidationLogic.validateSupply`: active, not paused, not frozen.
	fn supply_allowed(&self) -> bool {
		self.is_active() && !self.is_paused() && !self.is_frozen()
	}

	/// `ValidationLogic.validateWithdraw`: active and not paused. A frozen reserve
	/// still allows withdrawals — freezing only blocks new supply and borrowing.
	fn withdraw_allowed(&self) -> bool {
		self.is_active() && !self.is_paused()
	}

	fn can_supply(&self) -> bool {
		self.supply_allowed() && !self.available_supply().is_zero()
	}

	fn can_withdraw(&self) -> bool {
		self.withdraw_allowed() && !self.available_liquidity.is_zero()
	}

	/// Scaled-balance delta that raises `current_supply()` by `amount`, i.e. the
	/// inverse of the index scaling `current_supply` applies.
	fn scaled_from_underlying(&self, amount: U256) -> Option<U256> {
		amount
			.checked_mul(U256::from(10).pow(27.into()))?
			.checked_div(self.liquidity_index)
	}

	/// Reject a supply the reserve is closed to or cannot absorb, and book the accepted amount so
	/// later legs of the same solution see the reduced headroom. Without booking
	/// it, two legs into one reserve each pass alone and revert together on chain.
	fn take_supply_capacity(&mut self, amount: Balance) -> Result<(), SimulatorError> {
		let amount = U256::from(amount);
		ensure!(self.supply_allowed(), SimulatorError::NotSupported);
		ensure!(amount <= self.available_supply(), SimulatorError::InsufficientLiquidity);

		let scaled = self.scaled_from_underlying(amount).ok_or(SimulatorError::MathError)?;
		self.scaled_total_supply = self.scaled_total_supply.saturating_add(scaled);

		Ok(())
	}

	/// Reject a withdrawal the aToken contract cannot cover, and book it for the
	/// same reason `take_supply_capacity` books supplies.
	fn take_withdraw_liquidity(&mut self, amount: Balance) -> Result<(), SimulatorError> {
		let amount = U256::from(amount);
		ensure!(self.withdraw_allowed(), SimulatorError::NotSupported);
		ensure!(
			amount <= self.available_liquidity,
			SimulatorError::InsufficientLiquidity
		);
		self.available_liquidity = self.available_liquidity.saturating_sub(amount);

		Ok(())
	}
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq)]
pub struct Snapshot {
	/// Map of aave reserves
	pub reserves: BTreeMap<AssetId, ReserveData>,
	/// Aave pool contract address
	pub contract: EvmAddress,

	pub pairs: Vec<(AssetId, AssetId)>,
}

impl Snapshot {
	/// Charge a trade against the reserve it consumes, mirroring the direction test
	/// `AaveTradeExecutor::do_sell` makes: the underlying going in is a `supply`,
	/// bounded by the supply cap; the underlying coming out is a `withdraw`, bounded
	/// by the aToken's underlying balance. Reserves are keyed by the underlying, so
	/// membership decides the direction.
	fn take_capacity(&mut self, asset_in: AssetId, asset_out: AssetId, amount: Balance) -> Result<(), SimulatorError> {
		if let Some(reserve) = self.reserves.get_mut(&asset_in) {
			reserve.take_supply_capacity(amount)
		} else if let Some(reserve) = self.reserves.get_mut(&asset_out) {
			reserve.take_withdraw_liquidity(amount)
		} else {
			Ok(())
		}
	}

	/// Whether the pair can still be traded in at least one direction.
	fn is_tradable(&self, a: &AssetId, b: &AssetId) -> bool {
		match self.reserves.get(a).or_else(|| self.reserves.get(b)) {
			Some(reserve) => reserve.can_supply() || reserve.can_withdraw(),
			None => true,
		}
	}
}

//NOTE: This is tmp. dummy impl. of aave simulator that always trade 1:1 and doesn't do any checks.
pub struct Simulator<DataProvider>(PhantomData<DataProvider>);

impl<DP: DataProvider> Simulator<DP> {
	fn get_reserves_list(aave: EvmAddress) -> Result<Vec<EvmAddress>, SimulatorError> {
		let ctx = CallContext::new_view(aave);
		let data = EvmDataWriter::new_with_selector(Function::GetReservesList).build();

		let (exit_reason, value) = DP::view(ctx, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get reserves list reason: {exit_reason:?}, value: {value:?}");
			return Err(SimulatorError::Other);
		}

		let param_types = vec![ParamType::Array(Box::new(ParamType::Address))];

		let decoded = decode(&param_types, value.as_ref()).map_err(|_| {
			log::error!(target: LOG_TARGET, "to decore reserves list");
			SimulatorError::Other
		})?;

		// Convert decoded addresses to EvmAddress format
		let addresses = decoded[0]
			.clone()
			.into_array()
			.ok_or(SimulatorError::Other)?
			.into_iter()
			.filter_map(|addr| addr.into_address())
			.map(|addr| EvmAddress::from_slice(addr.as_bytes()))
			.collect();

		Ok(addresses)
	}

	fn get_reserve_data(aave: EvmAddress, reserve: EvmAddress) -> Result<ReserveData, SimulatorError> {
		let ctc = CallContext::new_view(aave);
		let data = EvmDataWriter::new_with_selector(Function::GetReserveData)
			.write(reserve)
			.build();

		let (exit_reason, value) = DP::view(ctc, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get reserves data, reason: {exit_reason:?}, value: {value:?}");
			return Err(SimulatorError::Other);
		}

		let param_types = vec![
			ParamType::Uint(256), // configuration
			ParamType::Uint(256), // liquidityIndex
			ParamType::Uint(256), // variableBorrowIndex
			ParamType::Uint(256), // currentLiquidityRate
			ParamType::Uint(256), // currentVariableBorrowRate
			ParamType::Uint(256), // currentStableBorrowRate
			ParamType::Uint(256), // lastUpdateTimestamp
			ParamType::Uint(16),  // id
			ParamType::Address,   // aTokenAddress
			ParamType::Address,   // stableDebtTokenAddress
			ParamType::Address,   // variableDebtTokenAddress
			ParamType::Address,   // interestRateStrategyAddress
			ParamType::Uint(256), // accruedToTreasury
		];

		let decoded = decode(&param_types, value.as_ref()).map_err(|_| {
			log::error!(target: LOG_TARGET, "to decode reserve data");
			SimulatorError::Other
		})?;

		// Ensure sufficient length
		ensure!(decoded.len() == param_types.len(), {
			log::error!(target: LOG_TARGET, "invalid reserve data");
			SimulatorError::Other
		});

		let a_token = EvmAddress::from_slice(decoded[8].clone().into_address().unwrap_or_default().as_ref());
		Ok(ReserveData {
			configuration: decoded[0].clone().into_uint().unwrap_or_default(),
			liquidity_index: decoded[1].clone().into_uint().unwrap_or_default(),
			current_liquidity_rate: decoded[3].clone().into_uint().unwrap_or_default(),
			variable_borrow_index: decoded[2].clone().into_uint().unwrap_or_default(),
			current_variable_borrow_rate: decoded[4].clone().into_uint().unwrap_or_default(),
			current_stable_borrow_rate: decoded[5].clone().into_uint().unwrap_or_default(),
			last_update_timestamp: decoded[6].clone().into_uint().unwrap_or_default(),
			id: decoded[7].clone().into_uint().unwrap_or_default().saturated_into(),
			atoken_address: a_token,
			stable_debt_token_address: EvmAddress::from_slice(
				decoded[9].clone().into_address().unwrap_or_default().as_ref(),
			),
			variable_debt_token_address: EvmAddress::from_slice(
				decoded[10].clone().into_address().unwrap_or_default().as_ref(),
			),
			interest_rate_strategy_address: EvmAddress::from_slice(
				decoded[11].clone().into_address().unwrap_or_default().as_ref(),
			),
			accrued_to_treasury: decoded[12].clone().into_uint().unwrap_or_default(),
			scaled_total_supply: Simulator::<DP>::get_scaled_total_supply(a_token)?,
			available_liquidity: Simulator::<DP>::get_balance_of(reserve, a_token)?,
		})
	}

	/// `balanceOf(account)` on `token`.
	fn get_balance_of(token: EvmAddress, account: EvmAddress) -> Result<U256, SimulatorError> {
		let ctx = CallContext::new_view(token);
		let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
			.write(account)
			.build();

		let (exit_reason, value) = DP::view(ctx, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get balance of {account:?} on {token:?}, reason: {exit_reason:?}, value: {value:?}");
			return Err(SimulatorError::Other);
		}

		ensure!(value.len() <= 32, {
			log::error!(target: LOG_TARGET, "invalid balance");
			SimulatorError::Other
		});
		Ok(U256::from_big_endian(value.as_slice()))
	}

	fn get_scaled_total_supply(reserve: EvmAddress) -> Result<U256, SimulatorError> {
		let ctx = CallContext::new_view(reserve);
		let data = EvmDataWriter::new_with_selector(Function::ScaledTotalSupply).build();

		let (exit_reason, value) = DP::view(ctx, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get scaled total supply, reserve: {reserve:?}, reason: {exit_reason:?}, value: {value:?}");
			return Err(SimulatorError::Other);
		}

		ensure!(value.len() <= 32, {
			log::error!(target: LOG_TARGET, "invalid scaled total supply");
			SimulatorError::Other
		});
		Ok(U256::from_big_endian(value.as_slice()))
	}
}

impl<DP: DataProvider> AmmSimulator for Simulator<DP> {
	type Snapshot = Snapshot;

	fn snapshot() -> Self::Snapshot {
		let mut snapshot = Snapshot {
			reserves: BTreeMap::new(),
			contract: DP::borrowing_contract(),
			pairs: DP::pairs(),
		};

		let Ok(reserves) = Self::get_reserves_list(snapshot.contract) else {
			return snapshot;
		};

		for addr in reserves {
			let Ok(reserve) = Self::get_reserve_data(snapshot.contract, addr) else {
				snapshot.reserves.clear();
				break;
			};

			let Some(asset_id) = DP::address_to_asset(addr) else {
				debug_assert!(false, "Failed to map reserve address to asset, reserve: {addr:?}");
				log::error!(target: LOG_TARGET, "to map reserve address to asset, reserve: {addr:?}");
				snapshot.reserves.clear();
				break;
			};

			snapshot.reserves.insert(asset_id, reserve);
		}

		snapshot
	}

	fn pool_type() -> PoolType<AssetId> {
		PoolType::Aave
	}

	fn simulate_buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		_max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if !snapshot.reserves.contains_key(&asset_in) && !snapshot.reserves.contains_key(&asset_out) {
			return Err(SimulatorError::AssetNotFound);
		}

		let mut next = snapshot.clone();
		next.take_capacity(asset_in, asset_out, amount_out)?;

		Ok((
			next,
			TradeResult {
				amount_in: amount_out,
				amount_out,
			},
		))
	}

	fn simulate_sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		_min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if !snapshot.reserves.contains_key(&asset_in) && !snapshot.reserves.contains_key(&asset_out) {
			return Err(SimulatorError::AssetNotFound);
		}

		let mut next = snapshot.clone();
		next.take_capacity(asset_in, asset_out, amount_in)?;

		Ok((
			next,
			TradeResult {
				amount_in,
				amount_out: amount_in,
			},
		))
	}

	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		snapshot: &Self::Snapshot,
	) -> Result<Price, SimulatorError> {
		if !snapshot.reserves.contains_key(&asset_in) && !snapshot.reserves.contains_key(&asset_out) {
			return Err(SimulatorError::AssetNotFound);
		}
		Ok(Ratio { n: 1, d: 1 })
	}

	fn can_trade(_asset_in: AssetId, _asset_out: AssetId, _snapshot: &Self::Snapshot) -> Option<PoolType<AssetId>> {
		// no, Dave, you cannot trade this now.
		None
	}

	fn pool_edges(snapshot: &Self::Snapshot) -> sp_std::vec::Vec<hydradx_traits::router::PoolEdge<AssetId>> {
		snapshot
			.pairs
			.iter()
			// A reserve reverts either way once it is inactive or paused, or once it is
			// both capped and drained, so stop advertising it rather than routing into a
			// guaranteed revert. The edge is undirected, so a pair still usable one way
			// stays listed and the simulators reject the dead direction per trade.
			.filter(|(a, b)| snapshot.is_tradable(a, b))
			.map(|(a, b)| PoolEdge {
				pool_type: PoolType::Aave,
				assets: vec![*a, *b],
			})
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const UNDERLYING: AssetId = 1;
	const ATOKEN: AssetId = 2;
	const RAY: u128 = 1_000_000_000_000_000_000_000_000_000;

	struct TestDp;

	impl DataProvider for TestDp {
		fn view(_: CallContext, _: Vec<u8>, _: u64) -> (ExitReason, Vec<u8>) {
			unimplemented!("snapshot-only tests never call the EVM")
		}
		fn borrowing_contract() -> EvmAddress {
			EvmAddress::default()
		}
		fn address_to_asset(_: EvmAddress) -> Option<AssetId> {
			None
		}
		fn pairs() -> Vec<(AssetId, AssetId)> {
			Vec::new()
		}
	}

	type Sim = Simulator<TestDp>;

	const ACTIVE_BIT: usize = 56;
	const FROZEN_BIT: usize = 57;
	const PAUSED_BIT: usize = 60;

	fn with_bit(mut r: ReserveData, bit: usize) -> ReserveData {
		r.configuration |= U256::one() << bit;
		r
	}

	fn without_bit(mut r: ReserveData, bit: usize) -> ReserveData {
		r.configuration &= !(U256::one() << bit);
		r
	}

	/// `decimals` is kept at 0 so the supply cap is expressed in raw units, and
	/// `liquidity_index` at 1 RAY so `current_supply == scaled_total_supply`.
	/// The reserve is active, unfrozen and unpaused unless a test says otherwise.
	fn reserve(supply_cap: u128, current_supply: u128, available_liquidity: u128) -> ReserveData {
		ReserveData {
			configuration: (U256::from(supply_cap) << 116) | (U256::one() << ACTIVE_BIT),
			liquidity_index: U256::from(RAY),
			current_liquidity_rate: U256::zero(),
			variable_borrow_index: U256::zero(),
			current_variable_borrow_rate: U256::zero(),
			current_stable_borrow_rate: U256::zero(),
			last_update_timestamp: U256::zero(),
			id: 0,
			atoken_address: EvmAddress::default(),
			stable_debt_token_address: EvmAddress::default(),
			variable_debt_token_address: EvmAddress::default(),
			interest_rate_strategy_address: EvmAddress::default(),
			accrued_to_treasury: U256::zero(),
			scaled_total_supply: U256::from(current_supply),
			available_liquidity: U256::from(available_liquidity),
		}
	}

	fn snapshot(reserve: ReserveData) -> Snapshot {
		let mut reserves = BTreeMap::new();
		reserves.insert(UNDERLYING, reserve);
		Snapshot {
			reserves,
			contract: EvmAddress::default(),
			pairs: vec![(UNDERLYING, ATOKEN)],
		}
	}

	#[test]
	fn simulate_sell_should_fail_when_supply_cap_is_reached() {
		let snapshot = snapshot(reserve(1_000, 1_000, u128::MAX));

		let result = Sim::simulate_sell(UNDERLYING, ATOKEN, 1, 0, &snapshot);

		assert_eq!(result, Err(SimulatorError::InsufficientLiquidity));
	}

	#[test]
	fn simulate_sell_should_fail_when_amount_exceeds_remaining_headroom() {
		let snapshot = snapshot(reserve(1_000, 900, u128::MAX));

		let result = Sim::simulate_sell(UNDERLYING, ATOKEN, 101, 0, &snapshot);

		assert_eq!(result, Err(SimulatorError::InsufficientLiquidity));
	}

	#[test]
	fn simulate_sell_should_succeed_when_amount_exactly_fills_headroom() {
		let snapshot = snapshot(reserve(1_000, 900, u128::MAX));

		let (next, trade) = Sim::simulate_sell(UNDERLYING, ATOKEN, 100, 0, &snapshot).unwrap();

		assert_eq!(
			trade,
			TradeResult {
				amount_in: 100,
				amount_out: 100
			}
		);
		assert_eq!(next.reserves[&UNDERLYING].available_supply(), U256::zero());
	}

	#[test]
	fn simulate_sell_should_consume_capacity_so_a_second_leg_sees_less_headroom() {
		let snapshot = snapshot(reserve(1_000, 900, u128::MAX));

		let (next, _) = Sim::simulate_sell(UNDERLYING, ATOKEN, 60, 0, &snapshot).unwrap();
		let second = Sim::simulate_sell(UNDERLYING, ATOKEN, 60, 0, &next);

		assert_eq!(next.reserves[&UNDERLYING].available_supply(), U256::from(40));
		assert_eq!(second, Err(SimulatorError::InsufficientLiquidity));
	}

	#[test]
	fn simulate_sell_should_ignore_the_cap_when_withdrawing() {
		let snapshot = snapshot(reserve(1_000, 1_000, 500));

		let (_, trade) = Sim::simulate_sell(ATOKEN, UNDERLYING, 500, 0, &snapshot).unwrap();

		assert_eq!(
			trade,
			TradeResult {
				amount_in: 500,
				amount_out: 500
			}
		);
	}

	#[test]
	fn simulate_sell_should_succeed_when_reserve_has_no_cap() {
		// supplyCap == 0 means "uncapped" in the Aave configuration bitmap.
		let snapshot = snapshot(reserve(0, u128::MAX / 2, u128::MAX));

		let (_, trade) = Sim::simulate_sell(UNDERLYING, ATOKEN, 1_000_000, 0, &snapshot).unwrap();

		assert_eq!(trade.amount_out, 1_000_000);
	}

	#[test]
	fn simulate_buy_should_fail_when_supply_cap_is_reached() {
		let snapshot = snapshot(reserve(1_000, 1_000, u128::MAX));

		let result = Sim::simulate_buy(UNDERLYING, ATOKEN, 1, Balance::MAX, &snapshot);

		assert_eq!(result, Err(SimulatorError::InsufficientLiquidity));
	}

	#[test]
	fn pool_edges_should_exclude_pair_when_reserve_is_capped_and_drained() {
		let snapshot = snapshot(reserve(1_000, 1_000, 0));

		assert!(Sim::pool_edges(&snapshot).is_empty());
	}

	#[test]
	fn pool_edges_should_include_pair_when_capped_but_still_withdrawable() {
		let snapshot = snapshot(reserve(1_000, 1_000, 500));

		assert_eq!(Sim::pool_edges(&snapshot).len(), 1);
	}

	#[test]
	fn pool_edges_should_include_pair_when_reserve_has_headroom() {
		let snapshot = snapshot(reserve(1_000, 999, 0));

		let edges = Sim::pool_edges(&snapshot);

		assert_eq!(edges.len(), 1);
		assert_eq!(edges[0].assets, vec![UNDERLYING, ATOKEN]);
	}

	#[test]
	fn simulate_sell_should_fail_when_withdraw_liquidity_is_exhausted() {
		let snapshot = snapshot(reserve(1_000, 500, 100));

		let result = Sim::simulate_sell(ATOKEN, UNDERLYING, 101, 0, &snapshot);

		assert_eq!(result, Err(SimulatorError::InsufficientLiquidity));
	}

	#[test]
	fn simulate_sell_should_consume_withdraw_liquidity_so_a_second_leg_sees_less() {
		let snapshot = snapshot(reserve(1_000, 500, 100));

		let (next, _) = Sim::simulate_sell(ATOKEN, UNDERLYING, 60, 0, &snapshot).unwrap();
		let second = Sim::simulate_sell(ATOKEN, UNDERLYING, 60, 0, &next);

		assert_eq!(next.reserves[&UNDERLYING].available_liquidity, U256::from(40));
		assert_eq!(second, Err(SimulatorError::InsufficientLiquidity));
	}

	#[test]
	fn simulate_sell_should_not_consume_withdraw_liquidity_when_supplying() {
		let snapshot = snapshot(reserve(1_000, 0, 100));

		let (next, _) = Sim::simulate_sell(UNDERLYING, ATOKEN, 50, 0, &snapshot).unwrap();

		assert_eq!(next.reserves[&UNDERLYING].available_liquidity, U256::from(100));
	}

	#[test]
	fn simulate_sell_should_fail_when_supplying_into_a_frozen_reserve() {
		let snapshot = snapshot(with_bit(reserve(1_000, 0, 500), FROZEN_BIT));

		let result = Sim::simulate_sell(UNDERLYING, ATOKEN, 10, 0, &snapshot);

		assert_eq!(result, Err(SimulatorError::NotSupported));
	}

	#[test]
	fn simulate_sell_should_succeed_when_withdrawing_from_a_frozen_reserve() {
		// Freezing blocks new supply only — withdrawals stay open.
		let snapshot = snapshot(with_bit(reserve(1_000, 0, 500), FROZEN_BIT));

		let (_, trade) = Sim::simulate_sell(ATOKEN, UNDERLYING, 500, 0, &snapshot).unwrap();

		assert_eq!(trade.amount_out, 500);
	}

	#[test]
	fn simulate_sell_should_fail_when_supplying_into_a_paused_reserve() {
		let snapshot = snapshot(with_bit(reserve(1_000, 0, 500), PAUSED_BIT));

		let result = Sim::simulate_sell(UNDERLYING, ATOKEN, 10, 0, &snapshot);

		assert_eq!(result, Err(SimulatorError::NotSupported));
	}

	#[test]
	fn simulate_sell_should_fail_when_withdrawing_from_a_paused_reserve() {
		let snapshot = snapshot(with_bit(reserve(1_000, 0, 500), PAUSED_BIT));

		let result = Sim::simulate_sell(ATOKEN, UNDERLYING, 10, 0, &snapshot);

		assert_eq!(result, Err(SimulatorError::NotSupported));
	}

	#[test]
	fn simulate_sell_should_fail_when_reserve_is_inactive() {
		let snapshot = snapshot(without_bit(reserve(1_000, 0, 500), ACTIVE_BIT));

		assert_eq!(
			Sim::simulate_sell(UNDERLYING, ATOKEN, 10, 0, &snapshot),
			Err(SimulatorError::NotSupported)
		);
		assert_eq!(
			Sim::simulate_sell(ATOKEN, UNDERLYING, 10, 0, &snapshot),
			Err(SimulatorError::NotSupported)
		);
	}

	#[test]
	fn pool_edges_should_exclude_pair_when_reserve_is_paused() {
		let snapshot = snapshot(with_bit(reserve(1_000, 0, 500), PAUSED_BIT));

		assert!(Sim::pool_edges(&snapshot).is_empty());
	}

	#[test]
	fn pool_edges_should_exclude_pair_when_reserve_is_inactive() {
		let snapshot = snapshot(without_bit(reserve(1_000, 0, 500), ACTIVE_BIT));

		assert!(Sim::pool_edges(&snapshot).is_empty());
	}

	#[test]
	fn pool_edges_should_include_pair_when_frozen_but_still_withdrawable() {
		let snapshot = snapshot(with_bit(reserve(1_000, 0, 500), FROZEN_BIT));

		assert_eq!(Sim::pool_edges(&snapshot).len(), 1);
	}
}
