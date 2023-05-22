use crate::types::Balance;
use crate::{Config, Error, HubAssetImbalance, Pallet};
use hydra_dx_math::omnipool::types::I129;

use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use orml_traits::MultiCurrency;
use sp_runtime::traits::Get;
use sp_runtime::{ArithmeticError, DispatchError};

impl<T: Config> TradeExecution<T::Origin, T::AccountId, T::AssetId, Balance> for Pallet<T> {
	type Error = DispatchError;

	fn calculate_sell(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		todo!()
	}

	fn calculate_buy(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::Omnipool {
			return Err(ExecutorError::NotSupported);
		}
		// Special handling when one of the asset is Hub Asset
		if asset_out == T::HubAssetId::get() {
			return Err(ExecutorError::Error(Error::<T>::NotAllowed.into()));
		}
		let asset_in_state = Self::load_asset_state(asset_in).map_err(ExecutorError::Error)?;
		let asset_out_state = Self::load_asset_state(asset_out).map_err(ExecutorError::Error)?;
		let current_imbalance = <HubAssetImbalance<T>>::get();

		if asset_in == T::HubAssetId::get() {
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			let state_changes = hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
				&(&asset_out_state).into(),
				amount_out,
				T::AssetFee::get(),
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
			)
			.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?;

			return Ok(*state_changes.asset.delta_hub_reserve);
		}

		let state_changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
			&(&asset_in_state).into(),
			&(&asset_out_state).into(),
			amount_out,
			T::AssetFee::get(),
			T::ProtocolFee::get(),
			current_imbalance.value,
		)
		.ok_or(ExecutorError::Error(ArithmeticError::Overflow.into()))?;

		//TODO: we should return amount in + fee

		Ok(*state_changes.asset_in.delta_reserve)
	}

	fn execute_sell(
		who: T::Origin,
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		todo!()
	}

	fn execute_buy(
		who: T::Origin,
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		todo!()
	}
}
