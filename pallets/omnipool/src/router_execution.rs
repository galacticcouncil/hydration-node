use crate::types::Balance;
use crate::{Config, Error, Pallet};
use frame_system::pallet_prelude::OriginFor;

use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use orml_traits::GetByKey;
use sp_runtime::traits::Get;
use sp_runtime::DispatchError::Corruption;
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128};

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

		if asset_in == T::HubAssetId::get() {
			let (asset_fee, _) = T::Fee::get(&(asset_out, asset_out_state.reserve));

			let state_changes = hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
				&(&asset_out_state).into(),
				amount_in,
				asset_fee,
			)
			.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

			return Ok(*state_changes.asset.delta_reserve);
		}
		let asset_in_state = Self::load_asset_state(asset_in).map_err(ExecutorError::Error)?;

		let (asset_fee, _) = T::Fee::get(&(asset_out, asset_out_state.reserve));
		let (_, protocol_fee) = T::Fee::get(&(asset_in, asset_in_state.reserve));

		let state_changes = hydra_dx_math::omnipool::calculate_sell_state_changes(
			&(&asset_in_state).into(),
			&(&asset_out_state).into(),
			amount_in,
			asset_fee,
			protocol_fee,
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

		if asset_in == T::HubAssetId::get() {
			let (asset_fee, _) = T::Fee::get(&(asset_out, asset_out_state.reserve));

			let state_changes = hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
				&(&asset_out_state).into(),
				amount_out,
				asset_fee,
			)
			.ok_or_else(|| ExecutorError::Error(ArithmeticError::Overflow.into()))?;

			return Ok(*state_changes.asset.delta_hub_reserve);
		}

		let asset_in_state = Self::load_asset_state(asset_in).map_err(ExecutorError::Error)?;

		let (asset_fee, _) = T::Fee::get(&(asset_out, asset_out_state.reserve));
		let (_, protocol_fee) = T::Fee::get(&(asset_in, asset_in_state.reserve));

		let state_changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
			&(&asset_in_state).into(),
			&(&asset_out_state).into(),
			amount_out,
			asset_fee,
			protocol_fee,
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

	fn calculate_spot_price_with_fee(
		pool_type: PoolType<T::AssetId>,
		asset_a: T::AssetId,
		asset_b: T::AssetId,
	) -> Result<FixedU128, ExecutorError<Self::Error>> {
		if pool_type != PoolType::Omnipool {
			return Err(ExecutorError::NotSupported);
		}

		if asset_b == T::HubAssetId::get() {
			return Err(ExecutorError::Error(Error::<T>::NotAllowed.into()));
		}

		let spot_price = if asset_a == T::HubAssetId::get() {
			let asset_b_state = Self::load_asset_state(asset_b).map_err(ExecutorError::Error)?;
			let (asset_fee, _) = T::Fee::get(&(asset_b, asset_b_state.reserve));

			hydra_dx_math::omnipool::calculate_lrna_spot_price(&asset_b_state.into(), Some(asset_fee))
				.ok_or(ExecutorError::Error(Corruption))?
				.reciprocal()
				.ok_or(ExecutorError::Error(Corruption))?
		} else {
			let asset_a_state = Self::load_asset_state(asset_a).map_err(ExecutorError::Error)?;
			let asset_b_state = Self::load_asset_state(asset_b).map_err(ExecutorError::Error)?;

			let (_, protocol_fee) = T::Fee::get(&(asset_a, asset_a_state.reserve));
			let (asset_fee, _) = T::Fee::get(&(asset_b, asset_b_state.reserve));

			hydra_dx_math::omnipool::calculate_spot_price(
				&asset_a_state.into(),
				&asset_b_state.into(),
				Some((protocol_fee, asset_fee)),
			)
			.ok_or(ExecutorError::Error(Corruption))?
			.reciprocal()
			.ok_or(ExecutorError::Error(Corruption))?
		};

		Ok(spot_price)
	}
}
