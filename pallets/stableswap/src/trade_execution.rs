use crate::types::AssetBalance;
use crate::{Balance, Config, Error, Pallet, Pools, D_ITERATIONS, Y_ITERATIONS};
use frame_support::ensure;
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use orml_traits::MultiCurrency;
use sp_runtime::traits::CheckedAdd;
use sp_runtime::{ArithmeticError, DispatchError};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

impl<T: Config> TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance> for Pallet<T> {
	type Error = DispatchError;

	fn calculate_sell(
		//TODO: rename to calculate_out_given_in
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				if asset_in == pool_id {
					let pool = Pools::<T>::get(pool_id).ok_or(ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let asset_idx = pool
						.find_asset(asset_out)
						.ok_or(ExecutorError::Error(Error::<T>::AssetNotInPool.into()))?;
					let pool_account = Self::pool_account(pool_id);
					let balances = pool.balances::<T>(&pool_account);
					let share_issuance = T::Currency::total_issuance(pool_id);

					let amplification = Self::get_amplification(&pool);
					let (amount, _) =
						hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
							&balances,
							amount_in,
							asset_idx,
							share_issuance,
							amplification,
							pool.withdraw_fee,
						)
						.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?;

					Ok(amount)
				} else if asset_out == pool_id {
					let pool = Pools::<T>::get(pool_id).ok_or(ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
					let pool_account = Self::pool_account(pool_id);

					let mut added_assets = BTreeMap::<T::AssetId, Balance>::new();
					if added_assets.insert(asset_in, amount_in).is_some() {
						return Err(ExecutorError::Error(Error::<T>::IncorrectAssets.into()));
					}

					let mut initial_reserves = Vec::new();
					let mut updated_reserves = Vec::new();
					for pool_asset in pool.assets.iter() {
						let reserve = T::Currency::free_balance(*pool_asset, &pool_account);
						initial_reserves.push(reserve);
						if let Some(liq_added) = added_assets.get(pool_asset) {
							updated_reserves.push(
								reserve
									.checked_add(*liq_added)
									.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?,
							);
						} else {
							updated_reserves.push(reserve);
						}
					}

					let amplification = Self::get_amplification(&pool);
					let share_issuance = T::Currency::total_issuance(pool_id);
					let share_amount = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
						&initial_reserves,
						&updated_reserves,
						amplification,
						share_issuance,
					)
					.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?;

					Ok(share_amount)
				} else {
					let (amount_out, _) = Self::calculate_out_amount(pool_id, asset_in, asset_out, amount_in)
						.map_err(ExecutorError::Error)?;

					Ok(amount_out)
				}
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn calculate_buy(
		//TODO: rename calculate_in_given_out
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::Stableswap(pool_id) => {
				if asset_out == pool_id {
					//I wanna buy 500 shares, how much luqidity i need provide to get 500 shares
					/*let s = Self::calculate_liquidity_for_share(
						pool_id,
						asset_in,
						amount_out
					)
					.map_err(ExecutorError::Error);*/
					todo!()
				} else if asset_in == pool_id {
					/*Self::calculate_shares(
						asset_in,
						&vec![AssetBalance {
							asset_id: asset_out,
							amount: amount_out,
						}],
					);*/

					todo!()
				//BUy 1000 USDT, how muhc shares I need to provide to receive 1000 USDT
				} else {
					let (amount_in, _) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out)
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
					//NOTE: user pays the withhdraw fee which is higher than the trade fee
					Self::remove_liquidity_one_asset(who, pool_id, asset_out, amount_in, min_limit)
						.map_err(ExecutorError::Error)
				} else if asset_out == pool_id {
					Self::add_liquidity(
						who,
						pool_id,
						vec![AssetBalance {
							asset_id: asset_in,
							amount: amount_in,
						}],
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
					//we buy shares. to receive share, we need to add liquditity
					//TODO: Add check for what we provide is less than max_limit
					let liquidity_to_provide = Self::calculate_buy(pool_type, asset_in, asset_out, amount_out)?;
					Self::add_liquidity(
						who,
						pool_id,
						vec![AssetBalance {
							asset_id: asset_in,
							amount: liquidity_to_provide,
						}],
					)
					.map_err(ExecutorError::Error)
				} else if asset_in == pool_id {
					todo!("we need the amount of shares we need to remove")
				} else {
					Self::buy(who, pool_id, asset_out, asset_in, amount_out, max_limit).map_err(ExecutorError::Error)
				}
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}
}
