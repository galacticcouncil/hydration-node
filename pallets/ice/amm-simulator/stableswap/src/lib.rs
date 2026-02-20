//! Stableswap simulator for off-chain trade simulation.
//!
//! This module provides an `AmmSimulator` implementation for the Stableswap pallet,
//! allowing trades to be simulated without modifying chain state. The simulator
//! supports:
//! - Regular swaps between pool assets
//! - Share asset trades (add/remove liquidity)
//! - Spot price calculation

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use codec::Encode;
use core::marker::PhantomData;
use hydra_dx_math::stableswap::types::AssetReserve;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::AmmSimulator;
use hydradx_traits::amm::SimulatorError;
use hydradx_traits::amm::TradeResult;
use hydradx_traits::router::PoolType;
use ice_support::AssetId;
use ice_support::Balance;
use pallet_stableswap::types::PoolInfo;
use pallet_stableswap::types::PoolPegInfo;
use pallet_stableswap::types::PoolSnapshot;
use sp_runtime::FixedPointNumber;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

const D_ITERATIONS: u8 = hydra_dx_math::stableswap::MAX_D_ITERATIONS;
const Y_ITERATIONS: u8 = hydra_dx_math::stableswap::MAX_Y_ITERATIONS;

pub struct Simulator<DataProvider>(PhantomData<DataProvider>);

/// Snapshot of all Stableswap pools for simulation purposes.
///
/// Contains all pool snapshots needed to simulate trades without
/// accessing chain storage. The pool_id (share asset id) is used as the key.
#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct StableswapSnapshot {
	pub pools: BTreeMap<AssetId, PoolSnapshot<AssetId>>,
	pub min_trading_limit: Balance,
}

impl StableswapSnapshot {
	pub fn get_pool(&self, pool_id: AssetId) -> Option<&PoolSnapshot<AssetId>> {
		self.pools.get(&pool_id)
	}

	pub fn with_updated_pool(mut self, pool_id: AssetId, snapshot: PoolSnapshot<AssetId>) -> Self {
		self.pools.insert(pool_id, snapshot);
		self
	}
}

pub trait DataProvider {
	type BlockNumber;

	fn pools() -> impl Iterator<Item = (AssetId, PoolInfo<AssetId, Self::BlockNumber>)>;

	fn pool_pegs(pool_id: AssetId) -> Option<PoolPegInfo<Self::BlockNumber, AssetId>>;

	fn create_snapshot(pool_id: AssetId) -> Option<PoolSnapshot<AssetId>>;

	fn min_trading_limit() -> Balance;
}

impl<DP: DataProvider> AmmSimulator for Simulator<DP> {
	type Snapshot = StableswapSnapshot;

	fn pool_type() -> PoolType<AssetId> {
		PoolType::Stableswap(0) // Representative value
	}

	/// Override to match any Stableswap pool, regardless of pool_id
	fn matches_pool_type(pool_type: PoolType<AssetId>) -> bool {
		matches!(pool_type, PoolType::Stableswap(_))
	}

	fn snapshot() -> Self::Snapshot {
		let mut pools = BTreeMap::new();

		for (pool_id, pool) in DP::pools() {
			// TODO: we skip incorrect pools - this was likely due to incorrect snapshots used in tests
			// but verify!
			if let Some(peg_info) = DP::pool_pegs(pool_id) {
				if peg_info.current.len() != pool.assets.len() {
					continue;
				}
			}

			if let Some(pool_snapshot) = DP::create_snapshot(pool_id) {
				// TODO: same here as above
				if pool_snapshot.pegs.len() != pool_snapshot.reserves.len() {
					continue;
				}

				// TODO: this should be removed, some pools dont have pegs
				// but issue with snapshosting mechanism?!
				if pool_snapshot.pegs.is_empty() {
					continue;
				}

				let assets: Vec<AssetId> = pool_snapshot.assets.iter().copied().collect();
				let snapshot = PoolSnapshot {
					assets: assets.try_into().unwrap_or_default(),
					reserves: pool_snapshot.reserves,
					amplification: pool_snapshot.amplification,
					fee: pool_snapshot.fee,
					block_fee: pool_snapshot.block_fee,
					pegs: pool_snapshot.pegs,
					share_issuance: pool_snapshot.share_issuance,
				};
				pools.insert(pool_id, snapshot);
			}
		}

		StableswapSnapshot {
			pools,
			min_trading_limit: DP::min_trading_limit(),
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

		let (pool_id, pool_snapshot) = find_pool(asset_in, asset_out, snapshot)?;

		if asset_in == pool_id {
			return simulate_remove_liquidity_sell(
				pool_id,
				asset_out,
				amount_in,
				min_amount_out,
				pool_snapshot,
				snapshot,
			);
		}

		if asset_out == pool_id {
			return simulate_add_liquidity_sell(pool_id, asset_in, amount_in, min_amount_out, pool_snapshot, snapshot);
		}

		simulate_regular_sell(
			pool_id,
			asset_in,
			asset_out,
			amount_in,
			min_amount_out,
			pool_snapshot,
			snapshot,
		)
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

		let (pool_id, pool_snapshot) = find_pool(asset_in, asset_out, snapshot)?;

		if asset_in == pool_id {
			return simulate_remove_liquidity_buy(
				pool_id,
				asset_out,
				amount_out,
				max_amount_in,
				pool_snapshot,
				snapshot,
			);
		}

		if asset_out == pool_id {
			return simulate_add_liquidity_buy(pool_id, asset_in, amount_out, max_amount_in, pool_snapshot, snapshot);
		}

		simulate_regular_buy(
			pool_id,
			asset_in,
			asset_out,
			amount_out,
			max_amount_in,
			pool_snapshot,
			snapshot,
		)
	}

	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		snapshot: &Self::Snapshot,
	) -> Result<Ratio, SimulatorError> {
		let (pool_id, pool_snapshot) = find_pool(asset_in, asset_out, snapshot)?;

		if asset_in == pool_id {
			// Price = how much asset_out you get per 1 share
			// Using a small simulation to determine spot price
			let test_shares = pool_snapshot.share_issuance / 10000; // 0.01% of total shares
			if test_shares == 0 {
				return Err(SimulatorError::InsufficientLiquidity);
			}

			let asset_idx = pool_snapshot
				.asset_idx(asset_out)
				.ok_or(SimulatorError::AssetNotFound)?;
			let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

			let (amount_out, _fee) =
				hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
					&pool_snapshot.reserves,
					test_shares,
					asset_idx,
					pool_snapshot.share_issuance,
					pool_snapshot.amplification,
					pool_snapshot.block_fee,
					&pegs,
				)
				.ok_or(SimulatorError::MathError)?;

			// Price = amount_out / test_shares
			return Ok(Ratio::new(amount_out, test_shares));
		}

		if asset_out == pool_id {
			// Price = how many shares you get per 1 unit of asset_in
			let asset_idx = pool_snapshot.asset_idx(asset_in).ok_or(SimulatorError::AssetNotFound)?;
			let decimals = pool_snapshot.reserves[asset_idx].decimals;
			let test_amount = 10u128.pow(decimals as u32); // 1 unit of asset

			let mut updated_reserves: Vec<AssetReserve> = pool_snapshot.reserves.to_vec();
			updated_reserves[asset_idx].amount = updated_reserves[asset_idx]
				.amount
				.checked_add(test_amount)
				.ok_or(SimulatorError::MathError)?;

			let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

			let (shares_out, _fees) = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
				&pool_snapshot.reserves,
				&updated_reserves,
				pool_snapshot.amplification,
				pool_snapshot.share_issuance,
				pool_snapshot.block_fee,
				&pegs,
			)
			.ok_or(SimulatorError::MathError)?;

			// Price = shares_out / test_amount
			return Ok(Ratio::new(shares_out, test_amount));
		}

		let assets_with_reserves: Vec<(u32, AssetReserve)> = pool_snapshot
			.assets
			.iter()
			.zip(pool_snapshot.reserves.iter())
			.map(|(id, r)| (*id, *r))
			.collect();

		let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

		let spot_price = hydra_dx_math::stableswap::calculate_spot_price(
			pool_id,
			assets_with_reserves,
			pool_snapshot.amplification,
			asset_in,
			asset_out,
			pool_snapshot.share_issuance,
			snapshot.min_trading_limit,
			Some(pool_snapshot.block_fee),
			&pegs,
		)
		.ok_or(SimulatorError::MathError)?;

		Ok(Ratio::new(spot_price.into_inner(), sp_runtime::FixedU128::DIV))
	}

	fn can_trade(asset_in: AssetId, asset_out: AssetId, snapshot: &Self::Snapshot) -> Option<PoolType<AssetId>> {
		// Use existing find_pool logic to check if both assets are in the same pool
		if let Ok((pool_id, _)) = find_pool(asset_in, asset_out, snapshot) {
			Some(PoolType::Stableswap(pool_id))
		} else {
			None
		}
	}
}

fn find_pool(
	asset_a: AssetId,
	asset_b: AssetId,
	snapshot: &StableswapSnapshot,
) -> Result<(AssetId, &PoolSnapshot<AssetId>), SimulatorError> {
	if let Some(pool) = snapshot.pools.get(&asset_a) {
		if pool.assets.iter().any(|&a| a == asset_b) {
			return Ok((asset_a, pool));
		}
	}

	if let Some(pool) = snapshot.pools.get(&asset_b) {
		if pool.assets.iter().any(|&a| a == asset_a) {
			return Ok((asset_b, pool));
		}
	}

	for (pool_id, pool) in &snapshot.pools {
		let has_a = pool.assets.iter().any(|&a| a == asset_a);
		let has_b = pool.assets.iter().any(|&a| a == asset_b);
		if has_a && has_b {
			return Ok((*pool_id, pool));
		}
	}

	Err(SimulatorError::AssetNotFound)
}

fn simulate_regular_sell(
	_pool_id: AssetId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_amount_out: Balance,
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<(StableswapSnapshot, TradeResult), SimulatorError> {
	let index_in = pool_snapshot.asset_idx(asset_in).ok_or(SimulatorError::AssetNotFound)?;
	let index_out = pool_snapshot
		.asset_idx(asset_out)
		.ok_or(SimulatorError::AssetNotFound)?;

	let initial_reserves = &pool_snapshot.reserves;

	if initial_reserves[index_in].is_zero() || initial_reserves[index_out].is_zero() {
		return Err(SimulatorError::InsufficientLiquidity);
	}

	let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

	let (amount_out, _fee) = hydra_dx_math::stableswap::calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		initial_reserves,
		index_in,
		index_out,
		amount_in,
		pool_snapshot.amplification,
		pool_snapshot.fee,
		&pegs,
	)
	.ok_or(SimulatorError::MathError)?;

	if amount_out < min_amount_out {
		return Err(SimulatorError::LimitNotMet);
	}

	let updated_pool = pool_snapshot.clone().update_reserves(
		hydradx_traits::stableswap::AssetAmount::new(asset_in, amount_in),
		hydradx_traits::stableswap::AssetAmount::new(asset_out, amount_out),
	);

	let pool_id = find_pool_id_for_snapshot(pool_snapshot, snapshot)?;
	let updated_snapshot = snapshot.clone().with_updated_pool(pool_id, updated_pool);

	Ok((updated_snapshot, TradeResult::new(amount_in, amount_out)))
}

fn simulate_regular_buy(
	_pool_id: AssetId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_out: Balance,
	max_amount_in: Balance,
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<(StableswapSnapshot, TradeResult), SimulatorError> {
	let index_in = pool_snapshot.asset_idx(asset_in).ok_or(SimulatorError::AssetNotFound)?;
	let index_out = pool_snapshot
		.asset_idx(asset_out)
		.ok_or(SimulatorError::AssetNotFound)?;

	let initial_reserves = &pool_snapshot.reserves;

	if initial_reserves[index_out].amount <= amount_out || initial_reserves[index_in].is_zero() {
		return Err(SimulatorError::InsufficientLiquidity);
	}

	let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

	let (amount_in, _fee) = hydra_dx_math::stableswap::calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		initial_reserves,
		index_in,
		index_out,
		amount_out,
		pool_snapshot.amplification,
		pool_snapshot.fee,
		&pegs,
	)
	.ok_or(SimulatorError::MathError)?;

	if amount_in > max_amount_in {
		return Err(SimulatorError::LimitNotMet);
	}

	// Update reserves
	let updated_pool = pool_snapshot.clone().update_reserves(
		hydradx_traits::stableswap::AssetAmount::new(asset_in, amount_in),
		hydradx_traits::stableswap::AssetAmount::new(asset_out, amount_out),
	);

	let pool_id = find_pool_id_for_snapshot(pool_snapshot, snapshot)?;
	let updated_snapshot = snapshot.clone().with_updated_pool(pool_id, updated_pool);

	Ok((updated_snapshot, TradeResult::new(amount_in, amount_out)))
}

fn simulate_add_liquidity_sell(
	pool_id: AssetId,
	asset_in: AssetId,
	amount_in: Balance,
	min_shares_out: Balance,
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<(StableswapSnapshot, TradeResult), SimulatorError> {
	let asset_idx = pool_snapshot.asset_idx(asset_in).ok_or(SimulatorError::AssetNotFound)?;

	let mut updated_reserves: Vec<AssetReserve> = pool_snapshot.reserves.to_vec();
	updated_reserves[asset_idx].amount = updated_reserves[asset_idx]
		.amount
		.checked_add(amount_in)
		.ok_or(SimulatorError::MathError)?;

	let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

	let (shares_out, _fees) = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
		&pool_snapshot.reserves,
		&updated_reserves,
		pool_snapshot.amplification,
		pool_snapshot.share_issuance,
		pool_snapshot.block_fee,
		&pegs,
	)
	.ok_or(SimulatorError::MathError)?;

	if shares_out < min_shares_out {
		return Err(SimulatorError::LimitNotMet);
	}

	let updated_pool = pool_snapshot
		.clone()
		.update_shares_and_reserve(asset_in, amount_in as i128, shares_out as i128);
	let updated_snapshot = snapshot.clone().with_updated_pool(pool_id, updated_pool);

	Ok((updated_snapshot, TradeResult::new(amount_in, shares_out)))
}

/// Simulate adding liquidity: buy specific amount of shares with asset
fn simulate_add_liquidity_buy(
	pool_id: AssetId,
	asset_in: AssetId,
	shares_out: Balance,
	max_amount_in: Balance,
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<(StableswapSnapshot, TradeResult), SimulatorError> {
	let asset_idx = pool_snapshot.asset_idx(asset_in).ok_or(SimulatorError::AssetNotFound)?;

	let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

	// Calculate how much asset is needed to get the desired shares
	let (amount_in, _fee) = hydra_dx_math::stableswap::calculate_add_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&pool_snapshot.reserves,
		shares_out,
		asset_idx,
		pool_snapshot.share_issuance,
		pool_snapshot.amplification,
		pool_snapshot.block_fee,
		&pegs,
	)
	.ok_or(SimulatorError::MathError)?;

	if amount_in > max_amount_in {
		return Err(SimulatorError::LimitNotMet);
	}

	let updated_pool = pool_snapshot
		.clone()
		.update_shares_and_reserve(asset_in, amount_in as i128, shares_out as i128);
	let updated_snapshot = snapshot.clone().with_updated_pool(pool_id, updated_pool);

	Ok((updated_snapshot, TradeResult::new(amount_in, shares_out)))
}

fn simulate_remove_liquidity_sell(
	pool_id: AssetId,
	asset_out: AssetId,
	shares_in: Balance,
	min_amount_out: Balance,
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<(StableswapSnapshot, TradeResult), SimulatorError> {
	let asset_idx = pool_snapshot
		.asset_idx(asset_out)
		.ok_or(SimulatorError::AssetNotFound)?;

	let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

	let (amount_out, _fee) = hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&pool_snapshot.reserves,
		shares_in,
		asset_idx,
		pool_snapshot.share_issuance,
		pool_snapshot.amplification,
		pool_snapshot.block_fee,
		&pegs,
	)
	.ok_or(SimulatorError::MathError)?;

	if amount_out < min_amount_out {
		return Err(SimulatorError::LimitNotMet);
	}

	let updated_pool =
		pool_snapshot
			.clone()
			.update_shares_and_reserve(asset_out, -(amount_out as i128), -(shares_in as i128));
	let updated_snapshot = snapshot.clone().with_updated_pool(pool_id, updated_pool);

	Ok((updated_snapshot, TradeResult::new(shares_in, amount_out)))
}

fn simulate_remove_liquidity_buy(
	pool_id: AssetId,
	asset_out: AssetId,
	amount_out: Balance,
	max_shares_in: Balance,
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<(StableswapSnapshot, TradeResult), SimulatorError> {
	let asset_idx = pool_snapshot
		.asset_idx(asset_out)
		.ok_or(SimulatorError::AssetNotFound)?;

	let pegs: Vec<(Balance, Balance)> = pool_snapshot.pegs.to_vec();

	let (shares_in, _fees) = hydra_dx_math::stableswap::calculate_shares_for_amount::<D_ITERATIONS>(
		&pool_snapshot.reserves,
		asset_idx,
		amount_out,
		pool_snapshot.amplification,
		pool_snapshot.share_issuance,
		pool_snapshot.block_fee,
		&pegs,
	)
	.ok_or(SimulatorError::MathError)?;

	if shares_in > max_shares_in {
		return Err(SimulatorError::LimitNotMet);
	}

	let updated_pool =
		pool_snapshot
			.clone()
			.update_shares_and_reserve(asset_out, -(amount_out as i128), -(shares_in as i128));
	let updated_snapshot = snapshot.clone().with_updated_pool(pool_id, updated_pool);

	Ok((updated_snapshot, TradeResult::new(shares_in, amount_out)))
}

fn find_pool_id_for_snapshot(
	pool_snapshot: &PoolSnapshot<AssetId>,
	snapshot: &StableswapSnapshot,
) -> Result<u32, SimulatorError> {
	for (pool_id, pool) in &snapshot.pools {
		if pool.assets == pool_snapshot.assets {
			return Ok(*pool_id);
		}
	}
	Err(SimulatorError::AssetNotFound)
}

//TODO: copy tests from simulator
