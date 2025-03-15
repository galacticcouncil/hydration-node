use crate::types::Tradability;
use crate::{Balance, Config, Error, Pallet, Pools, D_ITERATIONS, Y_ITERATIONS};
use frame_support::{ensure, BoundedVec};
use hydra_dx_math::stableswap::types::AssetReserve;
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::MultiCurrency;
use sp_core::Get;
use sp_runtime::{ArithmeticError, DispatchError, FixedU128};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

impl<T: Config> TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance> for Pallet<T>
where
	u32: sp_std::convert::From<T::AssetId>,
	sp_std::vec::Vec<(u32, AssetReserve)>: FromIterator<(T::AssetId, AssetReserve)>,
{
	type Error = DispatchError;

	fn calculate_sell(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				if asset_in == pool_id {
					let pool = Pools::<T>::get(pool_id)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let asset_idx = pool
						.find_asset(asset_out)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::AssetNotInPool.into()))?;
					let pool_account = Self::pool_account(pool_id);
					let balances = pool
						.reserves_with_decimals::<T>(&pool_account)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::UnknownDecimals.into()))?;
					let share_issuance = T::Currency::total_issuance(pool_id);

					let amplification = Self::get_amplification(&pool);
					let (trade_fee, asset_pegs) =
						Self::get_updated_pegs(pool_id, &pool).map_err(ExecutorError::Error)?;
					let (amount, _) =
						hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
							&balances,
							amount_in,
							asset_idx,
							share_issuance,
							amplification,
							trade_fee,
							&asset_pegs,
						)
						.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

					Ok(amount)
				} else if asset_out == pool_id {
					let pool = Pools::<T>::get(pool_id).ok_or(ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let pool_account = Self::pool_account(pool_id);

					let assets = [AssetAmount {
						asset_id: asset_in,
						amount: amount_in,
					}];

					let mut added_assets = BTreeMap::<T::AssetId, Balance>::new();
					for asset in assets.iter() {
						ensure!(
							Self::is_asset_allowed(pool_id, asset.asset_id, Tradability::ADD_LIQUIDITY),
							ExecutorError::Error(Error::<T>::NotAllowed.into())
						);
						ensure!(
							asset.amount >= T::MinTradingLimit::get(),
							ExecutorError::Error(Error::<T>::InsufficientTradingAmount.into())
						);

						ensure!(
							pool.find_asset(asset.asset_id).is_some(),
							ExecutorError::Error(Error::<T>::AssetNotInPool.into())
						);
						if added_assets.insert(asset.asset_id, asset.amount).is_some() {
							return Err(ExecutorError::Error(Error::<T>::IncorrectAssets.into()));
						}
					}

					let mut initial_reserves = Vec::with_capacity(pool.assets.len());
					let mut updated_reserves = Vec::with_capacity(pool.assets.len());
					for pool_asset in pool.assets.iter() {
						let decimals = Self::retrieve_decimals(*pool_asset)
							.ok_or(ExecutorError::Error(Error::<T>::UnknownDecimals.into()))?;
						let reserve = T::Currency::free_balance(*pool_asset, &pool_account);
						initial_reserves.push(AssetReserve {
							amount: reserve,
							decimals,
						});
						if let Some(liq_added) = added_assets.remove(pool_asset) {
							let inc_reserve = reserve
								.checked_add(liq_added)
								.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?;
							updated_reserves.push(AssetReserve {
								amount: inc_reserve,
								decimals,
							});
						} else {
							ensure!(
								reserve > 0u128,
								ExecutorError::Error(Error::<T>::InvalidInitialLiquidity.into())
							);
							updated_reserves.push(AssetReserve {
								amount: reserve,
								decimals,
							});
						}
					}

					let amplification = Self::get_amplification(&pool);
					let share_issuance = T::Currency::total_issuance(pool_id);
					let (trade_fee, asset_pegs) =
						Self::get_updated_pegs(pool_id, &pool).map_err(ExecutorError::Error)?;
					let (share_amount, _) = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
						&initial_reserves,
						&updated_reserves[..],
						amplification,
						share_issuance,
						trade_fee,
						&asset_pegs,
					)
					.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?;
					Ok(share_amount)
				} else {
					let (amount_out, _) = Self::calculate_out_amount(pool_id, asset_in, asset_out, amount_in, false)
						.map_err(ExecutorError::Error)?;

					Ok(amount_out)
				}
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn calculate_buy(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				if asset_out == pool_id {
					//I wanna buy 500 shares, how much luqidity i need provide to get 500 shares
					let pool = Pools::<T>::get(pool_id)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let asset_idx = pool
						.find_asset(asset_in)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::AssetNotInPool.into()))?;
					let pool_account = Self::pool_account(pool_id);
					let balances = pool
						.reserves_with_decimals::<T>(&pool_account)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::UnknownDecimals.into()))?;
					let share_issuance = T::Currency::total_issuance(pool_id);
					let amplification = Self::get_amplification(&pool);
					let (trade_fee, asset_pegs) =
						Self::get_updated_pegs(pool_id, &pool).map_err(ExecutorError::Error)?;

					let liqudity = hydra_dx_math::stableswap::calculate_add_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
						&balances,
						amount_out,
						asset_idx,
						share_issuance,
						amplification,
						trade_fee,
						&asset_pegs,
					)
					.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

					Ok(liqudity.0)
				} else if asset_in == pool_id {
					let pool = Pools::<T>::get(pool_id)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let asset_idx = pool
						.find_asset(asset_out)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::AssetNotInPool.into()))?;
					let pool_account = Self::pool_account(pool_id);
					let balances = pool
						.reserves_with_decimals::<T>(&pool_account)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::UnknownDecimals.into()))?;
					let share_issuance = T::Currency::total_issuance(pool_id);
					let amplification = Self::get_amplification(&pool);

					let pool = Pools::<T>::get(pool_id)
						.ok_or_else(|| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let (trade_fee, asset_pegs) =
						Self::get_updated_pegs(pool_id, &pool).map_err(ExecutorError::Error)?;

					let (shares_amount, _fees) =
						hydra_dx_math::stableswap::calculate_shares_for_amount::<D_ITERATIONS>(
							&balances,
							asset_idx,
							amount_out,
							amplification,
							share_issuance,
							trade_fee,
							&asset_pegs,
						)
						.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

					Ok(shares_amount)
				} else {
					let (amount_in, _) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out, false)
						.map_err(ExecutorError::Error)?;

					Ok(amount_in)
				}
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn execute_sell(
		who: T::RuntimeOrigin,
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				if asset_in == pool_id {
					Self::remove_liquidity_one_asset(who, pool_id, asset_out, amount_in, min_limit)
						.map_err(ExecutorError::Error)
				} else if asset_out == pool_id {
					Self::add_assets_liquidity(
						who,
						pool_id,
						BoundedVec::truncate_from(vec![AssetAmount {
							asset_id: asset_in,
							amount: amount_in,
						}]),
						min_limit,
					)
					.map_err(ExecutorError::Error)
				} else {
					Self::sell(who, pool_id, asset_in, asset_out, amount_in, min_limit).map_err(ExecutorError::Error)
				}
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn execute_buy(
		who: T::RuntimeOrigin,
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				if asset_out == pool_id {
					Self::add_liquidity_shares(who, pool_id, amount_out, asset_in, max_limit)
						.map_err(ExecutorError::Error)
				} else if asset_in == pool_id {
					Self::withdraw_asset_amount(who, pool_id, asset_out, amount_out, max_limit)
						.map_err(ExecutorError::Error)
				} else {
					Self::buy(who, pool_id, asset_out, asset_in, amount_out, max_limit).map_err(ExecutorError::Error)
				}
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn get_liquidity_depth(
		pool_type: PoolType<T::AssetId>,
		asset_a: T::AssetId,
		_asset_b: T::AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				let pool_account = Self::pool_account(pool_id);
				Ok(T::Currency::free_balance(asset_a, &pool_account))
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn calculate_spot_price_with_fee(
		pool_type: PoolType<T::AssetId>,
		asset_a: T::AssetId,
		asset_b: T::AssetId,
	) -> Result<FixedU128, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				let pool_account = Self::pool_account(pool_id);
				let pool =
					Pools::<T>::get(pool_id).ok_or_else(|| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
				let balances = pool
					.reserves_with_decimals::<T>(&pool_account)
					.ok_or_else(|| ExecutorError::Error(Error::<T>::UnknownDecimals.into()))?;

				let assets_with_reserves = pool
					.assets
					.iter()
					.zip(balances.iter())
					.map(|(asset_id, reserve)| (*asset_id, *reserve))
					.collect();
				let amp = Pallet::<T>::get_amplification(&pool);
				let share_issuance = T::Currency::total_issuance(pool_id);
				let min_trade_limit = T::MinTradingLimit::get();
				let (trade_fee, asset_pegs) = Self::get_updated_pegs(pool_id, &pool).map_err(ExecutorError::Error)?;

				let spot_price = hydra_dx_math::stableswap::calculate_spot_price(
					pool_id.into(),
					assets_with_reserves,
					amp,
					asset_a.into(),
					asset_b.into(),
					share_issuance,
					min_trade_limit,
					Some(trade_fee),
					&asset_pegs,
				)
				.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

				Ok(spot_price)
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}
}
