use crate::types::{AssetId, AssetPair, Balance};
use crate::{Config, Error, Pallet, XYKSpotPrice};
use frame_support::ensure;
use frame_support::traits::Get;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use sp_runtime::traits::CheckedDiv;
use sp_runtime::traits::CheckedSub;
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
		if pool_type != PoolType::XYK {
			return Err(ExecutorError::NotSupported);
		}

		let assets = AssetPair { asset_in, asset_out };

		if !Self::exists(assets) {
			return Err(ExecutorError::Error(Error::<T>::TokenPoolNotFound.into()));
		}

		let pair_account = Self::get_pair_id(assets);

		let asset_in_reserve = T::Currency::free_balance(assets.asset_in, &pair_account);
		let asset_out_reserve = T::Currency::free_balance(assets.asset_out, &pair_account);

		let amount_out = hydra_dx_math::xyk::calculate_out_given_in(asset_in_reserve, asset_out_reserve, amount_in)
			.map_err(|_| ExecutorError::Error(Error::<T>::SellAssetAmountInvalid.into()))?;

		ensure!(
			asset_out_reserve > amount_out,
			ExecutorError::Error(Error::<T>::InsufficientPoolAssetBalance.into())
		);

		let transfer_fee = Self::calculate_fee(amount_out).map_err(ExecutorError::Error)?;

		let amount_out_without_fee = amount_out
			.checked_sub(transfer_fee)
			.ok_or_else(|| ExecutorError::Error(Error::<T>::SellAssetAmountInvalid.into()))?;

		Ok(amount_out_without_fee)
	}

	fn calculate_buy(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::XYK {
			return Err(ExecutorError::NotSupported);
		}

		let assets = AssetPair { asset_in, asset_out };

		ensure!(
			Self::exists(assets),
			ExecutorError::Error(Error::<T>::TokenPoolNotFound.into())
		);

		let pair_account = Self::get_pair_id(assets);

		let asset_out_reserve = T::Currency::free_balance(assets.asset_out, &pair_account);
		let asset_in_reserve = T::Currency::free_balance(assets.asset_in, &pair_account);

		ensure!(
			asset_out_reserve > amount_out,
			ExecutorError::Error(Error::<T>::InsufficientPoolAssetBalance.into())
		);

		ensure!(
			amount_out >= T::MinTradingLimit::get(),
			ExecutorError::Error(Error::<T>::InsufficientTradingAmount.into())
		);

		let amount_in = hydra_dx_math::xyk::calculate_in_given_out(asset_out_reserve, asset_in_reserve, amount_out)
			.map_err(|_| ExecutorError::Error(Error::<T>::BuyAssetAmountInvalid.into()))?;

		let transfer_fee = Self::calculate_fee(amount_in).map_err(ExecutorError::Error)?;

		let amount_in_with_fee = amount_in
			.checked_add(transfer_fee)
			.ok_or_else(|| ExecutorError::Error(Error::<T>::BuyAssetAmountInvalid.into()))?;

		Ok(amount_in_with_fee)
	}

	fn execute_sell(
		who: T::RuntimeOrigin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if pool_type != PoolType::XYK {
			return Err(ExecutorError::NotSupported);
		}

		Self::sell(who, asset_in, asset_out, amount_in, min_limit, false).map_err(ExecutorError::Error)
	}

	fn execute_buy(
		who: T::RuntimeOrigin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if pool_type != PoolType::XYK {
			return Err(ExecutorError::NotSupported);
		}

		Self::buy(who, asset_out, asset_in, amount_out, max_limit, false).map_err(ExecutorError::Error)
	}

	fn get_liquidity_depth(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if pool_type != PoolType::XYK {
			return Err(ExecutorError::NotSupported);
		}

		let pair_account = Self::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		let liquidty = T::Currency::free_balance(asset_a, &pair_account);

		Ok(liquidty)
	}

	fn calculate_spot_price(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<FixedU128, ExecutorError<Self::Error>> {
		if pool_type != PoolType::XYK {
			return Err(ExecutorError::NotSupported);
		}

		//TODO: we have differente based on sell or buy, fix
		// Formula: spot-price-with-fee = spot-price-withoit-fee / (1 - fee)
		// We divide by (1 - f) to adjust for the fee, because the fee reduces the amount of the output asset received per unit of input asset.
		// It makes asset B more expensive, so the spot price should be increased
		let fee = T::GetExchangeFee::get();
		let fee = FixedU128::checked_from_rational(fee.0, fee.1).ok_or(ExecutorError::Error(Corruption))?;
		let fee_multipiler = FixedU128::from_rational(1, 1)
			.checked_sub(&fee)
			.ok_or(ExecutorError::Error(Corruption))?;

		let spot_price_without_fee =
			XYKSpotPrice::<T>::spot_price(asset_a, asset_b).ok_or(ExecutorError::Error(Corruption))?;

		spot_price_without_fee
			.checked_div(&fee_multipiler)
			.ok_or(ExecutorError::Error(Corruption))

		//XYKSpotPrice::<T>::spot_price(asset_a, asset_b).ok_or(ExecutorError::Error(Corruption))
	}
}
