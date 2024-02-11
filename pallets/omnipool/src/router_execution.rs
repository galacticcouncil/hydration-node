use crate::types::Balance;
use crate::{Config, Error, HubAssetImbalance, Pallet};
use frame_system::pallet_prelude::OriginFor;
use hydra_dx_math::omnipool::types::I129;

use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use orml_traits::{GetByKey, MultiCurrency};
use sp_runtime::traits::Get;
use sp_runtime::{ArithmeticError, DispatchError};

// dev note: The code is calculate sell and buy is copied from the corresponding functions.
// This is not ideal and should be refactored to avoid code duplication.
impl<T: Config> TradeExecution<OriginFor<T>, T::AccountId, T::AssetId, Balance> for Pallet<T> {
	type Error = DispatchError;

	fn calculate_sell(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::Omnipool {
			return Err(ExecutorError::NotSupported);
		}

		if asset_out == T::HubAssetId::get() {
			return Err(ExecutorError::Error(Error::<T>::NotAllowed.into()));
		}

		let asset_out_state = Self::load_asset_state(asset_out).map_err(ExecutorError::Error)?;
		let current_imbalance = <HubAssetImbalance<T>>::get();

		if asset_in == T::HubAssetId::get() {
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			let (asset_fee, _) = T::Fee::get(&asset_out);

			let state_changes = hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
				&(&asset_out_state).into(),
				amount_in,
				asset_fee,
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
			)
			.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

			return Ok(*state_changes.asset.delta_reserve);
		}

		let (asset_fee, _) = T::Fee::get(&asset_out);
		let (_, protocol_fee) = T::Fee::get(&asset_in);

		let asset_in_state = Self::load_asset_state(asset_in).map_err(ExecutorError::Error)?;
		let state_changes = hydra_dx_math::omnipool::calculate_sell_state_changes(
			&(&asset_in_state).into(),
			&(&asset_out_state).into(),
			amount_in,
			asset_fee,
			protocol_fee,
			current_imbalance.value,
		)
		.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

		Ok(*state_changes.asset_out.delta_reserve)
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
		let asset_out_state = Self::load_asset_state(asset_out).map_err(ExecutorError::Error)?;
		let current_imbalance = <HubAssetImbalance<T>>::get();

		if asset_in == T::HubAssetId::get() {
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			let (asset_fee, _) = T::Fee::get(&asset_out);

			let state_changes = hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
				&(&asset_out_state).into(),
				amount_out,
				asset_fee,
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
			)
			.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

			return Ok(*state_changes.asset.delta_hub_reserve);
		}

		let asset_in_state = Self::load_asset_state(asset_in).map_err(ExecutorError::Error)?;

		let (asset_fee, _) = T::Fee::get(&asset_out);
		let (_, protocol_fee) = T::Fee::get(&asset_in);

		let state_changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
			&(&asset_in_state).into(),
			&(&asset_out_state).into(),
			amount_out,
			asset_fee,
			protocol_fee,
			current_imbalance.value,
		)
		.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

		Ok(*state_changes.asset_in.delta_reserve)
	}

	fn execute_sell(
		who: OriginFor<T>,
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if pool_type != PoolType::Omnipool {
			return Err(ExecutorError::NotSupported);
		}

		Self::sell(who, asset_in, asset_out, amount_in, min_limit).map_err(ExecutorError::Error)
	}

	fn execute_buy(
		who: OriginFor<T>,
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if pool_type != PoolType::Omnipool {
			return Err(ExecutorError::NotSupported);
		}

		Self::buy(who, asset_out, asset_in, amount_out, max_limit).map_err(ExecutorError::Error)
	}

	fn get_liquidity_depth(
		pool_type: PoolType<T::AssetId>,
		asset_a: T::AssetId,
		_asset_b: T::AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::Omnipool {
			return Err(ExecutorError::NotSupported);
		}

		let asset_state = Self::load_asset_state(asset_a).map_err(ExecutorError::Error)?;

		Ok(asset_state.reserve)
	}
}
