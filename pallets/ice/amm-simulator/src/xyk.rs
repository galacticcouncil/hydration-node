#![cfg_attr(not(feature = "std"), no_std)]

//! XYK simulator.
//!
//! Constant product over the pair account's reserves. The checks and the fee
//! handling mirror `pallet_xyk`'s `validate_sell` / `validate_buy`, so a trade
//! the solver simulates is a trade the router can settle.

use codec::{Decode, Encode};
use core::marker::PhantomData;
use frame_support::pallet_prelude::RuntimeDebug;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AmmSimulator, SimulatorError, TradeResult};
use hydradx_traits::router::{PoolEdge, PoolType};
use ice_support::AssetId;
use ice_support::Balance;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

pub trait DataProvider {
	/// Existing pools as `(asset_a, asset_b, reserve_a, reserve_b)`.
	fn pools() -> Vec<(AssetId, AssetId, Balance, Balance)>;

	fn exchange_fee() -> (u32, u32);

	fn min_trading_limit() -> Balance;

	fn max_in_ratio() -> u128;

	fn max_out_ratio() -> u128;
}

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq, Default)]
pub struct Snapshot {
	/// Reserves keyed by the asset pair sorted ascending, values in that same order.
	pub pools: BTreeMap<(AssetId, AssetId), (Balance, Balance)>,
	pub exchange_fee: (u32, u32),
	pub min_trading_limit: Balance,
	pub max_in_ratio: u128,
	pub max_out_ratio: u128,
}

impl Snapshot {
	fn key(asset_a: AssetId, asset_b: AssetId) -> (AssetId, AssetId) {
		if asset_a < asset_b {
			(asset_a, asset_b)
		} else {
			(asset_b, asset_a)
		}
	}

	/// Reserves oriented as `(in, out)`.
	fn reserves(&self, asset_in: AssetId, asset_out: AssetId) -> Option<(Balance, Balance)> {
		let key = Self::key(asset_in, asset_out);
		let (reserve_a, reserve_b) = *self.pools.get(&key)?;
		Some(if key.0 == asset_in {
			(reserve_a, reserve_b)
		} else {
			(reserve_b, reserve_a)
		})
	}

	fn with_reserves(
		mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		in_reserve: Balance,
		out_reserve: Balance,
	) -> Self {
		let key = Self::key(asset_in, asset_out);
		let value = if key.0 == asset_in {
			(in_reserve, out_reserve)
		} else {
			(out_reserve, in_reserve)
		};
		self.pools.insert(key, value);
		self
	}
}

fn fee(amount: Balance, rate: (u32, u32)) -> Result<Balance, SimulatorError> {
	hydra_dx_math::fee::calculate_pool_trade_fee(amount, rate).ok_or(SimulatorError::MathError)
}

pub struct Simulator<DataProvider>(PhantomData<DataProvider>);

impl<DP: DataProvider> AmmSimulator for Simulator<DP> {
	type Snapshot = Snapshot;

	fn pool_type() -> PoolType<AssetId> {
		PoolType::XYK
	}

	fn snapshot() -> Self::Snapshot {
		let pools = DP::pools()
			.into_iter()
			// An empty side cannot price or fill anything, so it never reaches the solver.
			.filter(|(_, _, reserve_a, reserve_b)| *reserve_a > 0 && *reserve_b > 0)
			.map(|(asset_a, asset_b, reserve_a, reserve_b)| {
				let key = Snapshot::key(asset_a, asset_b);
				let value = if key.0 == asset_a {
					(reserve_a, reserve_b)
				} else {
					(reserve_b, reserve_a)
				};
				(key, value)
			})
			.collect();

		Snapshot {
			pools,
			exchange_fee: DP::exchange_fee(),
			min_trading_limit: DP::min_trading_limit(),
			max_in_ratio: DP::max_in_ratio(),
			max_out_ratio: DP::max_out_ratio(),
		}
	}

	fn simulate_sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if asset_in == asset_out {
			return Err(SimulatorError::Other);
		}

		if amount_in < snapshot.min_trading_limit {
			return Err(SimulatorError::TradeTooSmall);
		}

		let (in_reserve, out_reserve) = snapshot
			.reserves(asset_in, asset_out)
			.ok_or(SimulatorError::AssetNotFound)?;

		let max_in = in_reserve
			.checked_div(snapshot.max_in_ratio)
			.ok_or(SimulatorError::MathError)?;
		if amount_in > max_in {
			return Err(SimulatorError::TradeTooLarge);
		}

		let amount_out = hydra_dx_math::xyk::calculate_out_given_in(in_reserve, out_reserve, amount_in)
			.map_err(|_| SimulatorError::MathError)?;

		let max_out = out_reserve
			.checked_div(snapshot.max_out_ratio)
			.ok_or(SimulatorError::MathError)?;
		if amount_out > max_out {
			return Err(SimulatorError::TradeTooLarge);
		}

		if out_reserve <= amount_out {
			return Err(SimulatorError::InsufficientLiquidity);
		}

		let amount_out_without_fee = amount_out
			.checked_sub(fee(amount_out, snapshot.exchange_fee)?)
			.ok_or(SimulatorError::MathError)?;

		if amount_out_without_fee == 0 {
			return Err(SimulatorError::InsufficientLiquidity);
		}

		if amount_out_without_fee < min_amount_out {
			return Err(SimulatorError::LimitNotMet);
		}

		// The fee is never paid out, so it stays behind as reserve.
		let new_in = in_reserve.checked_add(amount_in).ok_or(SimulatorError::MathError)?;
		let new_out = out_reserve
			.checked_sub(amount_out_without_fee)
			.ok_or(SimulatorError::MathError)?;

		Ok((
			snapshot.clone().with_reserves(asset_in, asset_out, new_in, new_out),
			TradeResult::new(amount_in, amount_out_without_fee),
		))
	}

	fn simulate_buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if asset_in == asset_out {
			return Err(SimulatorError::Other);
		}

		if amount_out < snapshot.min_trading_limit {
			return Err(SimulatorError::TradeTooSmall);
		}

		let (in_reserve, out_reserve) = snapshot
			.reserves(asset_in, asset_out)
			.ok_or(SimulatorError::AssetNotFound)?;

		if out_reserve <= amount_out {
			return Err(SimulatorError::InsufficientLiquidity);
		}

		let max_out = out_reserve
			.checked_div(snapshot.max_out_ratio)
			.ok_or(SimulatorError::MathError)?;
		if amount_out > max_out {
			return Err(SimulatorError::TradeTooLarge);
		}

		let buy_price = hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, amount_out)
			.map_err(|_| SimulatorError::MathError)?;

		let max_in = in_reserve
			.checked_div(snapshot.max_in_ratio)
			.ok_or(SimulatorError::MathError)?;
		if buy_price > max_in {
			return Err(SimulatorError::TradeTooLarge);
		}

		let amount_in = buy_price
			.checked_add(fee(buy_price, snapshot.exchange_fee)?)
			.ok_or(SimulatorError::MathError)?;

		if amount_in > max_amount_in {
			return Err(SimulatorError::LimitNotMet);
		}

		let new_in = in_reserve.checked_add(amount_in).ok_or(SimulatorError::MathError)?;
		let new_out = out_reserve.checked_sub(amount_out).ok_or(SimulatorError::MathError)?;

		Ok((
			snapshot.clone().with_reserves(asset_in, asset_out, new_in, new_out),
			TradeResult::new(amount_in, amount_out),
		))
	}

	/// Fee-free, matching the other simulators' convention.
	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		snapshot: &Self::Snapshot,
	) -> Result<Ratio, SimulatorError> {
		if asset_in == asset_out {
			return Err(SimulatorError::Other);
		}

		let (in_reserve, out_reserve) = snapshot
			.reserves(asset_in, asset_out)
			.ok_or(SimulatorError::AssetNotFound)?;

		Ok(Ratio::new(out_reserve, in_reserve))
	}

	fn can_trade(asset_in: AssetId, asset_out: AssetId, snapshot: &Self::Snapshot) -> Option<PoolType<AssetId>> {
		if asset_in == asset_out {
			return None;
		}
		snapshot.reserves(asset_in, asset_out).map(|_| PoolType::XYK)
	}

	fn pool_edges(snapshot: &Self::Snapshot) -> Vec<PoolEdge<AssetId>> {
		snapshot
			.pools
			.keys()
			.map(|(asset_a, asset_b)| PoolEdge {
				pool_type: PoolType::XYK,
				assets: vec![*asset_a, *asset_b],
			})
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const ASSET_A: AssetId = 1;
	const ASSET_B: AssetId = 2;
	const ASSET_C: AssetId = 3;

	struct Provider;

	impl DataProvider for Provider {
		fn pools() -> Vec<(AssetId, AssetId, Balance, Balance)> {
			vec![(ASSET_A, ASSET_B, 1_000_000, 2_000_000), (ASSET_C, ASSET_B, 0, 5_000)]
		}

		fn exchange_fee() -> (u32, u32) {
			(3, 1_000)
		}

		fn min_trading_limit() -> Balance {
			1_000
		}

		fn max_in_ratio() -> u128 {
			3
		}

		fn max_out_ratio() -> u128 {
			3
		}
	}

	type Sim = Simulator<Provider>;

	#[test]
	fn snapshot_should_skip_a_pool_with_an_empty_side() {
		let snapshot = Sim::snapshot();

		assert_eq!(snapshot.pools.len(), 1);
		assert_eq!(snapshot.pools.get(&(ASSET_A, ASSET_B)), Some(&(1_000_000, 2_000_000)));
	}

	#[test]
	fn reserves_should_orient_to_the_requested_direction() {
		let snapshot = Sim::snapshot();

		assert_eq!(snapshot.reserves(ASSET_A, ASSET_B), Some((1_000_000, 2_000_000)));
		assert_eq!(snapshot.reserves(ASSET_B, ASSET_A), Some((2_000_000, 1_000_000)));
	}

	#[test]
	fn simulate_sell_should_deduct_the_fee_from_the_output() {
		let (_, result) = Sim::simulate_sell(ASSET_A, ASSET_B, 100_000, 0, &Sim::snapshot()).unwrap();

		// 2_000_000 * 100_000 / 1_100_000 = 181_818, fee (181_818 / 1_000) * 3 = 543.
		assert_eq!(result, TradeResult::new(100_000, 181_275));
	}

	#[test]
	fn simulate_sell_should_leave_the_fee_in_the_pool() {
		let (snapshot, _) = Sim::simulate_sell(ASSET_A, ASSET_B, 100_000, 0, &Sim::snapshot()).unwrap();

		assert_eq!(snapshot.reserves(ASSET_A, ASSET_B), Some((1_100_000, 1_818_725)));
	}

	#[test]
	fn simulate_sell_should_yield_less_when_a_second_leg_runs_the_same_direction() {
		let snapshot = Sim::snapshot();
		let (snapshot, _) = Sim::simulate_sell(ASSET_A, ASSET_B, 100_000, 0, &snapshot).unwrap();

		let (_, result) = Sim::simulate_sell(ASSET_A, ASSET_B, 100_000, 0, &snapshot).unwrap();

		assert_eq!(result, TradeResult::new(100_000, 151_107));
	}

	#[test]
	fn selling_back_should_return_less_than_the_original_input() {
		let snapshot = Sim::snapshot();
		let (snapshot, sold) = Sim::simulate_sell(ASSET_A, ASSET_B, 100_000, 0, &snapshot).unwrap();

		let (_, back) = Sim::simulate_sell(ASSET_B, ASSET_A, sold.amount_out, 0, &snapshot).unwrap();

		assert_eq!(back, TradeResult::new(181_275, 99_404));
	}

	#[test]
	fn simulate_sell_should_fail_when_amount_is_below_the_min_trading_limit() {
		let result = Sim::simulate_sell(ASSET_A, ASSET_B, 999, 0, &Sim::snapshot());

		assert_eq!(result, Err(SimulatorError::TradeTooSmall));
	}

	#[test]
	fn simulate_sell_should_fail_when_amount_exceeds_the_max_in_ratio() {
		let result = Sim::simulate_sell(ASSET_A, ASSET_B, 333_334, 0, &Sim::snapshot());

		assert_eq!(result, Err(SimulatorError::TradeTooLarge));
	}

	#[test]
	fn simulate_sell_should_fail_when_the_pool_does_not_exist() {
		let result = Sim::simulate_sell(ASSET_A, ASSET_C, 100_000, 0, &Sim::snapshot());

		assert_eq!(result, Err(SimulatorError::AssetNotFound));
	}

	#[test]
	fn simulate_sell_should_fail_when_the_limit_is_not_met() {
		let result = Sim::simulate_sell(ASSET_A, ASSET_B, 100_000, 181_276, &Sim::snapshot());

		assert_eq!(result, Err(SimulatorError::LimitNotMet));
	}

	#[test]
	fn simulate_buy_should_round_the_input_up_and_add_the_fee() {
		let (_, result) = Sim::simulate_buy(ASSET_A, ASSET_B, 100_000, u128::MAX, &Sim::snapshot()).unwrap();

		// 1_000_000 * 100_000 / 1_900_000 = 52_631, rounded up to 52_632,
		// fee (52_632 / 1_000) * 3 = 156.
		assert_eq!(result, TradeResult::new(52_788, 100_000));
	}

	#[test]
	fn simulate_buy_should_credit_the_whole_input_to_the_pool() {
		let (snapshot, _) = Sim::simulate_buy(ASSET_A, ASSET_B, 100_000, u128::MAX, &Sim::snapshot()).unwrap();

		assert_eq!(snapshot.reserves(ASSET_A, ASSET_B), Some((1_052_788, 1_900_000)));
	}

	#[test]
	fn simulate_buy_should_fail_when_the_input_exceeds_the_limit() {
		let result = Sim::simulate_buy(ASSET_A, ASSET_B, 100_000, 52_787, &Sim::snapshot());

		assert_eq!(result, Err(SimulatorError::LimitNotMet));
	}

	#[test]
	fn simulate_buy_should_fail_when_the_output_exceeds_the_max_out_ratio() {
		let result = Sim::simulate_buy(ASSET_A, ASSET_B, 666_667, u128::MAX, &Sim::snapshot());

		assert_eq!(result, Err(SimulatorError::TradeTooLarge));
	}

	#[test]
	fn get_spot_price_should_be_the_reserve_ratio() {
		let snapshot = Sim::snapshot();

		assert_eq!(
			Sim::get_spot_price(ASSET_A, ASSET_B, &snapshot),
			Ok(Ratio::new(2_000_000, 1_000_000))
		);
		assert_eq!(
			Sim::get_spot_price(ASSET_B, ASSET_A, &snapshot),
			Ok(Ratio::new(1_000_000, 2_000_000))
		);
	}

	#[test]
	fn can_trade_should_report_xyk_only_for_a_registered_pair() {
		let snapshot = Sim::snapshot();

		assert_eq!(Sim::can_trade(ASSET_A, ASSET_B, &snapshot), Some(PoolType::XYK));
		assert_eq!(Sim::can_trade(ASSET_A, ASSET_C, &snapshot), None);
	}

	#[test]
	fn pool_edges_should_list_every_pool_once() {
		let edges = Sim::pool_edges(&Sim::snapshot());

		assert_eq!(edges.len(), 1);
		assert_eq!(edges[0].pool_type, PoolType::XYK);
		assert_eq!(edges[0].assets, vec![ASSET_A, ASSET_B]);
	}
}
