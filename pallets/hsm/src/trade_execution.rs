use crate::types::Balance;
use crate::{Collaterals, Config, Error, Pallet};
use frame_support::pallet_prelude::{Get, IsType};
use hydra_dx_math::stableswap::types::AssetReserve;
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use sp_core::crypto::AccountId32;
use sp_runtime::{ArithmeticError, DispatchError, FixedU128};

impl<T: Config> TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance> for Pallet<T>
where
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	u32: sp_std::convert::From<T::AssetId>,
	sp_std::vec::Vec<(u32, AssetReserve)>: FromIterator<(T::AssetId, AssetReserve)>,
{
	type Error = DispatchError;

	fn calculate_out_given_in(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::HSM => {
				if asset_in == asset_out {
					return Err(ExecutorError::Error(Error::<T>::InvalidAssetPair.into()));
				}
				let amount_out = if asset_in == T::HollarId::get() {
					// Calculate collateral asset amount out given Hollar in
					|| -> Result<Balance, DispatchError> {
						let collateral_info = Collaterals::<T>::get(asset_out).ok_or(Error::<T>::AssetNotApproved)?;
						let pool_state = Self::get_stablepool_state(collateral_info.pool_id)?;
						let collateral_amount = Self::simulate_in_given_out(
							collateral_info.pool_id,
							asset_out,
							T::HollarId::get(),
							amount_in,
							Balance::MAX,
							&pool_state,
						)?;
						let execution_price = (collateral_amount, amount_in);
						let buy_price = hydra_dx_math::hsm::calculate_buy_price_with_fee(
							execution_price,
							collateral_info.buy_back_fee,
						)
						.ok_or(ArithmeticError::Overflow)?;

						let collateral_amount = hydra_dx_math::hsm::calculate_collateral_amount(amount_in, buy_price)
							.ok_or(ArithmeticError::Overflow)?;
						Ok(collateral_amount)
					}()
					.map_err(ExecutorError::Error)?
				} else if asset_out == T::HollarId::get() {
					// Calculate Hollar amount out given collateral asset in
					// dev note: buying hollar from hsm
					|| -> Result<Balance, DispatchError> {
						let collateral_info = Collaterals::<T>::get(asset_in).ok_or(Error::<T>::AssetNotApproved)?;
						let pool_state = Self::get_stablepool_state(collateral_info.pool_id)?;
						let peg = Self::get_asset_peg(asset_in, collateral_info.pool_id, &pool_state)?;
						let purchase_price =
							hydra_dx_math::hsm::calculate_purchase_price(peg, collateral_info.purchase_fee);
						let hollar_amount = hydra_dx_math::hsm::calculate_hollar_amount(amount_in, purchase_price)
							.ok_or(ArithmeticError::Overflow)?;
						Ok(hollar_amount)
					}()
					.map_err(ExecutorError::Error)?
				} else {
					return Err(ExecutorError::Error(Error::<T>::InvalidAssetPair.into()));
				};
				Ok(amount_out)
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn calculate_in_given_out(
		pool_type: PoolType<T::AssetId>,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::HSM => {
				if asset_in == asset_out {
					return Err(ExecutorError::Error(Error::<T>::InvalidAssetPair.into()));
				}
				let amount_in = if asset_in == T::HollarId::get() {
					// Calculate Hollar amount in given collateral asset out
					|| -> Result<Balance, DispatchError> {
						let collateral_info = Collaterals::<T>::get(asset_out).ok_or(Error::<T>::AssetNotApproved)?;
						let pool_state = Self::get_stablepool_state(collateral_info.pool_id)?;
						let hollar_amount = Self::simulate_out_given_in(
							collateral_info.pool_id,
							asset_out,
							T::HollarId::get(),
							amount_out,
							0,
							&pool_state,
						)?;

						let execution_price = (amount_out, hollar_amount);
						let buy_price = hydra_dx_math::hsm::calculate_buy_price_with_fee(
							execution_price,
							collateral_info.buy_back_fee,
						)
						.ok_or(ArithmeticError::Overflow)?;

						let hollar_amount_to_pay = hydra_dx_math::hsm::calculate_hollar_amount(amount_out, buy_price)
							.ok_or(ArithmeticError::Overflow)?;
						Ok(hollar_amount_to_pay)
					}()
					.map_err(ExecutorError::Error)?
				} else if asset_out == T::HollarId::get() {
					// Calculate collateral asset amount in given Hollar out
					// dev note: buying hollar from hsm
					|| -> Result<Balance, DispatchError> {
						let collateral_info = Collaterals::<T>::get(asset_in).ok_or(Error::<T>::AssetNotApproved)?;
						let pool_state = Self::get_stablepool_state(collateral_info.pool_id)?;
						let peg = Self::get_asset_peg(asset_in, collateral_info.pool_id, &pool_state)?;
						let purchase_price =
							hydra_dx_math::hsm::calculate_purchase_price(peg, collateral_info.purchase_fee);
						let hollar_amount = hydra_dx_math::hsm::calculate_collateral_amount(amount_out, purchase_price)
							.ok_or(ArithmeticError::Overflow)?;
						Ok(hollar_amount)
					}()
					.map_err(ExecutorError::Error)?
				} else {
					return Err(ExecutorError::Error(Error::<T>::InvalidAssetPair.into()));
				};
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
			PoolType::HSM => Self::sell(who, asset_in, asset_out, amount_in, min_limit).map_err(ExecutorError::Error),
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
			PoolType::HSM => Self::buy(who, asset_in, asset_out, amount_out, max_limit).map_err(ExecutorError::Error),
			_ => Err(ExecutorError::NotSupported),
		}
	}

	fn get_liquidity_depth(
		pool_type: PoolType<T::AssetId>,
		asset_a: T::AssetId,
		asset_b: T::AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		match pool_type {
			PoolType::HSM => {
				let collateral_info = if asset_a == T::HollarId::get() {
					Collaterals::<T>::get(asset_b).ok_or(ExecutorError::Error(Error::<T>::AssetNotApproved.into()))?
				} else {
					Collaterals::<T>::get(asset_a).ok_or(ExecutorError::Error(Error::<T>::AssetNotApproved.into()))?
				};
				<pallet_stableswap::Pallet<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance>>::get_liquidity_depth(
					PoolType::Stableswap(collateral_info.pool_id),
					asset_a,
					asset_b,
				)
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
			PoolType::HSM => {
				let collateral_info = if asset_a == T::HollarId::get() {
					Collaterals::<T>::get(asset_b).ok_or(ExecutorError::Error(Error::<T>::AssetNotApproved.into()))?
				} else {
					Collaterals::<T>::get(asset_a).ok_or(ExecutorError::Error(Error::<T>::AssetNotApproved.into()))?
				};
				<pallet_stableswap::Pallet<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance>>::calculate_spot_price_with_fee(
					PoolType::Stableswap(collateral_info.pool_id),
					asset_a,
					asset_b,
				)
			}
			_ => Err(ExecutorError::NotSupported),
		}
	}
}
