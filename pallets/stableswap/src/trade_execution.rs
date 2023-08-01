use crate::types::AssetBalance;
use crate::{Balance, Config, Pallet};
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use sp_runtime::DispatchError;
use sp_std::vec;

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
					//we are selling shares, how much stuff I get for the share
					todo!("calculate how much we remove") //TODO: we need a helper function
				} else if asset_out == pool_id {
					let shares_amount = Self::calculate_shares(
						pool_id,
						&vec![AssetBalance {
							asset_id: asset_in,
							amount: amount_in,
						}],
					)
					.map_err(ExecutorError::Error)?;

					Ok(shares_amount)
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
					Self::calculate_shares(
						asset_in,
						&vec![AssetBalance {
							asset_id: asset_out,
							amount: amount_out,
						}],
					);

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
