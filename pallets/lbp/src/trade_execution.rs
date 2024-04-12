use crate::*;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use sp_runtime::traits::{BlockNumberProvider, CheckedSub};
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul};
use sp_runtime::DispatchError::Corruption;
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128};
impl<T: Config> TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance> for Pallet<T> {
	type Error = DispatchError;

	fn calculate_sell(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::LBP {
			return Err(ExecutorError::NotSupported);
		}

		let assets = AssetPair { asset_in, asset_out };
		let pool_id = Self::get_pair_id(assets);
		let pool_data =
			<PoolData<T>>::try_get(&pool_id).map_err(|_| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;

		let now = T::BlockNumberProvider::current_block_number();
		let (weight_in, weight_out) = Self::get_sorted_weight(assets.asset_in, now, &pool_data)
			.map_err(|err| ExecutorError::Error(err.into()))?;
		let asset_in_reserve = T::MultiCurrency::free_balance(assets.asset_in, &pool_id);
		let asset_out_reserve = T::MultiCurrency::free_balance(assets.asset_out, &pool_id);

		let amount_out = hydra_dx_math::lbp::calculate_out_given_in(
			asset_in_reserve,
			asset_out_reserve,
			weight_in,
			weight_out,
			amount_in,
		)
		.map_err(|_| ExecutorError::Error(Error::<T>::Overflow.into()))?;

		let fee_asset = pool_data.assets.0;
		if fee_asset == assets.asset_in {
			Ok(amount_out) //amount with fee applied as the user is responsible to send fee to the fee collector
		} else {
			let fee = Self::calculate_fees(&pool_data, amount_out).map_err(ExecutorError::Error)?;
			let amount_out_without_fee = amount_out
				.checked_sub(fee)
				.ok_or_else(|| ExecutorError::Error(Error::<T>::Overflow.into()))?;

			Ok(amount_out_without_fee) //amount without fee as the pool is responsible to send fee to the fee collector
		}
	}

	fn calculate_buy(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::LBP {
			return Err(ExecutorError::NotSupported);
		}

		let assets = AssetPair { asset_in, asset_out };
		let pool_id = Self::get_pair_id(assets);
		let pool_data =
			<PoolData<T>>::try_get(&pool_id).map_err(|_| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;

		let now = T::BlockNumberProvider::current_block_number();
		let (weight_in, weight_out) = Self::get_sorted_weight(assets.asset_in, now, &pool_data)
			.map_err(|err| ExecutorError::Error(err.into()))?;
		let asset_in_reserve = T::MultiCurrency::free_balance(assets.asset_in, &pool_id);
		let asset_out_reserve = T::MultiCurrency::free_balance(assets.asset_out, &pool_id);

		let fee_asset = pool_data.assets.0;
		if fee_asset == assets.asset_out {
			let fee = Self::calculate_fees(&pool_data, amount_out).map_err(ExecutorError::Error)?;
			let amount_out_plus_fee = amount_out
				.checked_add(fee)
				.ok_or_else(|| ExecutorError::Error(Error::<T>::Overflow.into()))?;

			let calculated_in = hydra_dx_math::lbp::calculate_in_given_out(
				asset_in_reserve,
				asset_out_reserve,
				weight_in,
				weight_out,
				amount_out_plus_fee,
			)
			.map_err(|_| ExecutorError::Error(Error::<T>::Overflow.into()))?;

			Ok(calculated_in) //TODO: Double check with someone if this is correct
		} else {
			let calculated_in = hydra_dx_math::lbp::calculate_in_given_out(
				asset_in_reserve,
				asset_out_reserve,
				weight_in,
				weight_out,
				amount_out,
			)
			.map_err(|_| ExecutorError::Error(Error::<T>::Overflow.into()))?;

			Ok(calculated_in) //amount with fee applied as the user is responsible to send fee to the fee collector
		}
	}

	fn execute_sell(
		who: T::RuntimeOrigin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if pool_type != PoolType::LBP {
			return Err(ExecutorError::NotSupported);
		}

		Self::sell(who, asset_in, asset_out, amount_in, min_limit).map_err(ExecutorError::Error)
	}

	fn execute_buy(
		who: T::RuntimeOrigin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if pool_type != PoolType::LBP {
			return Err(ExecutorError::NotSupported);
		}

		Self::buy(who, asset_out, asset_in, amount_out, max_limit).map_err(ExecutorError::Error)
	}

	fn get_liquidity_depth(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::LBP {
			return Err(ExecutorError::NotSupported);
		}

		let asset_pair = AssetPair::new(asset_a, asset_b);
		let pair_account = Self::get_pair_id(asset_pair);

		let liquidty = T::MultiCurrency::free_balance(asset_a, &pair_account);

		Ok(liquidty)
	}

	fn calculate_spot_price(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<FixedU128, ExecutorError<Self::Error>> {
		if pool_type != PoolType::LBP {
			return Err(ExecutorError::NotSupported);
		}

		let spot_price_without_fee = Self::spot_price(asset_a, asset_b).ok_or(ExecutorError::Error(Corruption))?;
		let assets = AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		};
		let pool_id = Self::get_pair_id(assets);
		let pool_data =
			<PoolData<T>>::try_get(&pool_id).map_err(|_| ExecutorError::Error(Error::<T>::PoolNotFound.into()))?;
		let fee_asset = pool_data.assets.0;

		let fee = if Self::is_repay_fee_applied(&pool_data) {
			Self::repay_fee()
		} else {
			pool_data.fee
		};

		let spot_price_with_fee = if fee_asset == assets.asset_out {
			//Pool pays fee, but fee is deducted from asset out.
			//We divide by (1-f) to reflect correct amount out after the fee deduction
			let fee = FixedU128::checked_from_rational(fee.0, fee.1).ok_or(ExecutorError::Error(Corruption))?;
			let fee_multiplier = FixedU128::from_rational(1, 1)
				.checked_sub(&fee)
				.ok_or(ExecutorError::Error(Corruption))?;

			spot_price_without_fee
				.checked_div(&fee_multiplier)
				.ok_or(ExecutorError::Error(Corruption))?
		} else {
			//User pays fee and fee is deducted from asset in
			//We multiply by (1+f) to reflect correct amount in after the fee deduction
			let fee = FixedU128::checked_from_rational(fee.0, fee.1).ok_or(ExecutorError::Error(Corruption))?;
			let fee_multiplier = FixedU128::from_rational(1, 1)
				.checked_add(&fee)
				.ok_or(ExecutorError::Error(Corruption))?;

			spot_price_without_fee
				.checked_mul(&fee_multiplier)
				.ok_or(ExecutorError::Error(Corruption))?
		};

		Ok(spot_price_with_fee)
	}
}
