use crate::{Balance, Config, Pallet};
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use sp_runtime::DispatchError;

impl<T: Config> TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance> for Pallet<T> {
    type Error = DispatchError;

    fn calculate_sell(
        pool_type: PoolType<T::AssetId>,
        asset_in: T::AssetId,
        asset_out: T::AssetId,
        amount_in: Balance,
    ) -> Result<Balance, ExecutorError<Self::Error>> {
        match pool_type {
            PoolType::Stableswap(pool_id) => {
                let (amount_out, _) = Self::calculate_out_amount(pool_id, asset_in, asset_out, amount_in)
                    .map_err(ExecutorError::Error)?;

                Ok(amount_out)
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
                let (amount_in, _) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out)
                    .map_err(ExecutorError::Error)?;

                Ok(amount_in)
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
                Self::sell(who, pool_id, asset_in, asset_out, amount_in, min_limit).map_err(ExecutorError::Error)
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
                Self::buy(who, pool_id, asset_out, asset_in, amount_out, max_limit).map_err(ExecutorError::Error)
            }
            _ => Err(ExecutorError::NotSupported),
        }
    }
}
