// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # XYK Pallet
//!
//! ## Overview
//!
//! XYK pallet provides functionality for managing liquidity pool and executing trades.
//!
//! This pallet implements AMM Api trait therefore it is possible to plug this pool implementation
//! into the exchange pallet.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::sp_runtime::{
	traits::{Hash, Zero},
	DispatchError,
};
use frame_support::{dispatch::DispatchResult, ensure, traits::Get, transactional};
use frame_system::ensure_signed;
use primitives::{asset::AssetPair, fee, traits::AMM, AssetId, Balance, Price, MAX_IN_RATIO, MAX_OUT_RATIO};
use sp_std::{marker::PhantomData, vec, vec::Vec};

use frame_support::sp_runtime::app_crypto::sp_core::crypto::UncheckedFrom;
use frame_support::sp_runtime::FixedPointNumber;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::fee::WithFee;
use primitives::traits::AMMTransfer;
use primitives::Amount;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_asset_registry::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Share token support
		type AssetPairAccountId: AssetPairAccountIdFor<AssetId, Self::AccountId>;

		/// Multi currency for transfer of currencies
		type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = Amount>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<AssetId>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		/// Trading fee rate
		#[pallet::constant]
		type GetExchangeFee: Get<fee::Fee>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// It is not allowed to create a pool between same assets.
		CannotCreatePoolWithSameAssets,

		/// It is not allowed to create a pool with zero initial liquidity.
		CannotCreatePoolWithZeroLiquidity,

		/// It is not allowed to create a pool with zero initial price.
		CannotCreatePoolWithZeroInitialPrice,

		/// Overflow
		CreatePoolAssetAmountInvalid,

		/// It is not allowed to remove zero liquidity.
		CannotRemoveLiquidityWithZero,

		/// It is not allowed to add zero liquidity.
		CannotAddZeroLiquidity,

		/// Overflow
		InvalidMintedLiquidity, // No tests - but it is currently not possible this error to occur due to previous checks in the code.

		/// Overflow
		InvalidLiquidityAmount, // no tests

		/// Asset amount has exceeded given limit.
		AssetAmountExceededLimit,

		/// Asset amount has not reached given limit.
		AssetAmountNotReachedLimit,

		/// Asset balance is not sufficient.
		InsufficientAssetBalance,

		/// Not enough asset liquidity in the pool.
		InsufficientPoolAssetBalance, // No tests

		/// Not enough core asset liquidity in the pool.
		InsufficientNativeCurrencyBalance, // No tests

		/// Liquidity pool for given assets does not exist.
		TokenPoolNotFound,

		/// Liquidity pool for given assets already exists.
		TokenPoolAlreadyExists,

		/// Overflow
		AddAssetAmountInvalid, // no tests
		/// Overflow
		RemoveAssetAmountInvalid, // no tests
		/// Overflow
		SellAssetAmountInvalid, // no tests
		/// Overflow
		BuyAssetAmountInvalid, // no tests
		/// Overflow
		FeeAmountInvalid, // no tests
		/// Overflow
		CannotApplyDiscount,

		/// Max fraction of pool to buy in single transaction has been exceeded.
		MaxOutRatioExceeded,
		/// Max fraction of pool to sell in single transaction has been exceeded.
		MaxInRatioExceeded,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New liquidity was provided to the pool. [who, asset a, asset b, amount a, amount b]
		LiquidityAdded(T::AccountId, AssetId, AssetId, Balance, Balance),

		/// Liquidity was removed from the pool. [who, asset a, asset b, shares]
		LiquidityRemoved(T::AccountId, AssetId, AssetId, Balance),

		/// Pool was created. [who, asset a, asset b, initial shares amount]
		PoolCreated(T::AccountId, AssetId, AssetId, Balance),

		/// Pool was destroyed. [who, asset a, asset b]
		PoolDestroyed(T::AccountId, AssetId, AssetId),

		/// Asset sale executed. [who, asset in, asset out, amount, sale price, fee asset, fee amount]
		SellExecuted(T::AccountId, AssetId, AssetId, Balance, Balance, AssetId, Balance),

		/// Asset purchase executed. [who, asset out, asset in, amount, buy price, fee asset, fee amount]
		BuyExecuted(T::AccountId, AssetId, AssetId, Balance, Balance, AssetId, Balance),
	}

	/// Asset id storage for shared pool tokens
	#[pallet::storage]
	#[pallet::getter(fn share_token)]
	pub type ShareToken<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, AssetId, ValueQuery>;

	/// Total liquidity in a pool.
	#[pallet::storage]
	#[pallet::getter(fn total_liquidity)]
	pub type TotalLiquidity<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Asset pair in a pool.
	#[pallet::storage]
	#[pallet::getter(fn pool_assets)]
	pub type PoolAssets<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (AssetId, AssetId), ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create new pool for given asset pair.
		///
		/// Registers new pool for given asset pair (`asset a` and `asset b`) in asset registry.
		/// Asset registry creates new id or returns previously created one if such pool existed before.
		///
		/// Pool is created with initial liquidity provided by `origin`.
		/// Shares are issued with specified initial price and represents proportion of asset in the pool.
		///
		/// Emits `PoolCreated` event when successful.
		#[pallet::weight(<T as Config>::WeightInfo::create_pool())]
		#[transactional]
		pub fn create_pool(
			origin: OriginFor<T>,
			asset_a: AssetId,
			asset_b: AssetId,
			amount: Balance,
			initial_price: Price,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure!(!amount.is_zero(), Error::<T>::CannotCreatePoolWithZeroLiquidity);
			ensure!(
				!(initial_price == Price::zero()),
				Error::<T>::CannotCreatePoolWithZeroInitialPrice
			);

			ensure!(asset_a != asset_b, Error::<T>::CannotCreatePoolWithSameAssets);

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(!Self::exists(asset_pair), Error::<T>::TokenPoolAlreadyExists);

			let asset_b_amount = initial_price
				.checked_mul_int(amount)
				.ok_or(Error::<T>::CreatePoolAssetAmountInvalid)?;

			let shares_added = if asset_a < asset_b { amount } else { asset_b_amount };

			ensure!(
				T::Currency::free_balance(asset_a, &who) >= amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(asset_b, &who) >= asset_b_amount,
				Error::<T>::InsufficientAssetBalance
			);

			let pair_account = Self::get_pair_id(asset_pair);

			let token_name = asset_pair.name();

			let share_token = <pallet_asset_registry::Pallet<T>>::get_or_create_asset(token_name)?.into();

			<ShareToken<T>>::insert(&pair_account, &share_token);
			<PoolAssets<T>>::insert(&pair_account, (asset_a, asset_b));

			T::Currency::transfer(asset_a, &who, &pair_account, amount)?;
			T::Currency::transfer(asset_b, &who, &pair_account, asset_b_amount)?;

			T::Currency::deposit(share_token, &who, shares_added)?;

			<TotalLiquidity<T>>::insert(&pair_account, shares_added);

			Self::deposit_event(Event::PoolCreated(who, asset_a, asset_b, shares_added));

			Ok(().into())
		}

		/// Add liquidity to previously created asset pair pool.
		///
		/// Shares are issued with current price.
		///
		/// Emits `LiquidityAdded` event when successful.
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity())]
		#[transactional]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			asset_a: AssetId,
			asset_b: AssetId,
			amount_a: Balance,
			amount_b_max_limit: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(Self::exists(asset_pair), Error::<T>::TokenPoolNotFound);

			ensure!(!amount_a.is_zero(), Error::<T>::CannotAddZeroLiquidity);

			ensure!(!amount_b_max_limit.is_zero(), Error::<T>::CannotAddZeroLiquidity);

			ensure!(
				T::Currency::free_balance(asset_a, &who) >= amount_a,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(asset_b, &who) >= amount_b_max_limit,
				Error::<T>::InsufficientAssetBalance
			);

			let pair_account = Self::get_pair_id(asset_pair);

			let share_token = Self::share_token(&pair_account);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);
			let total_liquidity = Self::total_liquidity(&pair_account);

			let amount_b_required = hydra_dx_math::calculate_liquidity_in(asset_a_reserve, asset_b_reserve, amount_a)
				.map_err(|_| Error::<T>::AddAssetAmountInvalid)?;

			let shares_added = if asset_a < asset_b { amount_a } else { amount_b_required };

			ensure!(
				amount_b_required <= amount_b_max_limit,
				Error::<T>::AssetAmountExceededLimit
			);

			ensure!(shares_added > 0_u128, Error::<T>::InvalidMintedLiquidity);

			let liquidity_amount = total_liquidity
				.checked_add(shares_added)
				.ok_or(Error::<T>::InvalidLiquidityAmount)?;

			let asset_b_balance = T::Currency::free_balance(asset_b, &who);

			ensure!(
				asset_b_balance >= amount_b_required,
				Error::<T>::InsufficientAssetBalance
			);

			T::Currency::transfer(asset_a, &who, &pair_account, amount_a)?;
			T::Currency::transfer(asset_b, &who, &pair_account, amount_b_required)?;

			T::Currency::deposit(share_token, &who, shares_added)?;

			<TotalLiquidity<T>>::insert(&pair_account, liquidity_amount);

			Self::deposit_event(Event::LiquidityAdded(
				who,
				asset_a,
				asset_b,
				amount_a,
				amount_b_required,
			));

			Ok(().into())
		}

		/// Remove liquidity from specific liquidity pool in the form of burning shares.
		///
		/// If liquidity in the pool reaches 0, it is destroyed.
		///
		/// Emits 'LiquidityRemoved' when successful.
		/// Emits 'PoolDestroyed' when pool is destroyed.
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity())]
		#[transactional]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			asset_a: AssetId,
			asset_b: AssetId,
			liquidity_amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(!liquidity_amount.is_zero(), Error::<T>::CannotRemoveLiquidityWithZero);

			ensure!(Self::exists(asset_pair), Error::<T>::TokenPoolNotFound);

			let pair_account = Self::get_pair_id(asset_pair);

			let share_token = Self::share_token(&pair_account);

			let total_shares = Self::total_liquidity(&pair_account);

			ensure!(total_shares >= liquidity_amount, Error::<T>::InsufficientAssetBalance);

			ensure!(
				T::Currency::free_balance(share_token, &who) >= liquidity_amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(!total_shares.is_zero(), Error::<T>::CannotRemoveLiquidityWithZero);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

			let liquidity_out = hydra_dx_math::calculate_liquidity_out(
				asset_a_reserve,
				asset_b_reserve,
				liquidity_amount,
				total_shares,
			)
			.map_err(|_| Error::<T>::RemoveAssetAmountInvalid)?;

			let (remove_amount_a, remove_amount_b) = liquidity_out;

			ensure!(
				T::Currency::free_balance(asset_a, &pair_account) >= remove_amount_a,
				Error::<T>::InsufficientPoolAssetBalance
			);
			ensure!(
				T::Currency::free_balance(asset_b, &pair_account) >= remove_amount_b,
				Error::<T>::InsufficientPoolAssetBalance
			);

			let liquidity_left = total_shares
				.checked_sub(liquidity_amount)
				.ok_or(Error::<T>::InvalidLiquidityAmount)?;

			T::Currency::transfer(asset_a, &pair_account, &who, remove_amount_a)?;
			T::Currency::transfer(asset_b, &pair_account, &who, remove_amount_b)?;

			T::Currency::withdraw(share_token, &who, liquidity_amount)?;

			<TotalLiquidity<T>>::insert(&pair_account, liquidity_left);

			Self::deposit_event(Event::LiquidityRemoved(who.clone(), asset_a, asset_b, liquidity_amount));

			if liquidity_left == 0 {
				<ShareToken<T>>::remove(&pair_account);
				<PoolAssets<T>>::remove(&pair_account);

				Self::deposit_event(Event::PoolDestroyed(who, asset_a, asset_b));
			}

			Ok(().into())
		}

		/// Trade asset in for asset out.
		///
		/// Executes a swap of `asset_in` for `asset_out`. Price is determined by the liquidity pool.
		///
		/// `max_limit` - minimum amount of `asset_out` / amount of asset_out to be obtained from the pool in exchange for `asset_in`.
		///
		/// Emits `SellExecuted` when successful.
		#[pallet::weight(<T as Config>::WeightInfo::sell())]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: AssetId,
			asset_out: AssetId,
			amount: Balance,
			max_limit: Balance,
			discount: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			<Self as AMM<_, _, _, _>>::sell(&who, AssetPair { asset_in, asset_out }, amount, max_limit, discount)?;

			Ok(().into())
		}

		/// Trade asset in for asset out.
		///
		/// Executes a swap of `asset_in` for `asset_out`. Price is determined by the liquidity pool.
		///
		/// `max_limit` - maximum amount of `asset_in` to be sold in exchange for `asset_out`.
		///
		/// Emits `BuyExecuted` when successful.
		#[pallet::weight(<T as Config>::WeightInfo::buy())]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: AssetId,
			asset_in: AssetId,
			amount: Balance,
			max_limit: Balance,
			discount: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			<Self as AMM<_, _, _, _>>::buy(&who, AssetPair { asset_in, asset_out }, amount, max_limit, discount)?;

			Ok(().into())
		}
	}
}

pub trait AssetPairAccountIdFor<AssetId: Sized, AccountId: Sized> {
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> AccountId;
}

pub struct AssetPairAccountId<T: Config>(PhantomData<T>);

impl<T: Config> AssetPairAccountIdFor<AssetId, T::AccountId> for AssetPairAccountId<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> T::AccountId {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"hydradx");
		if asset_a < asset_b {
			buf.extend_from_slice(&asset_a.to_le_bytes());
			buf.extend_from_slice(&asset_b.to_le_bytes());
		} else {
			buf.extend_from_slice(&asset_b.to_le_bytes());
			buf.extend_from_slice(&asset_a.to_le_bytes());
		}
		T::AccountId::unchecked_from(T::Hashing::hash(&buf[..]))
	}
}

impl<T: Config> Pallet<T> {
	/// Return balance of each asset in selected liquidity pool.
	pub fn get_pool_balances(pool_address: T::AccountId) -> Option<Vec<(AssetId, Balance)>> {
		let mut balances = Vec::new();

		if let Some(assets) = Self::get_pool_assets(&pool_address) {
			for item in &assets {
				let reserve = T::Currency::free_balance(*item, &pool_address);
				balances.push((*item, reserve));
			}
		}
		Some(balances)
	}
	/// Calculate discounted trade fee
	fn calculate_discounted_fee(amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount
			.discounted_fee()
			.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?)
	}

	/// Calculate trade fee
	fn calculate_fee(amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount
			.just_fee(T::GetExchangeFee::get())
			.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?)
	}
}

// Implementation of AMM API which makes possible to plug the AMM pool into the exchange pallet.
impl<T: Config> AMM<T::AccountId, AssetId, AssetPair, Balance> for Pallet<T> {
	fn exists(assets: AssetPair) -> bool {
		let pair_account = T::AssetPairAccountId::from_assets(assets.asset_in, assets.asset_out);
		<ShareToken<T>>::contains_key(&pair_account)
	}

	fn get_pair_id(assets: AssetPair) -> T::AccountId {
		T::AssetPairAccountId::from_assets(assets.asset_in, assets.asset_out)
	}

	fn get_pool_assets(pool_account_id: &T::AccountId) -> Option<Vec<AssetId>> {
		match <PoolAssets<T>>::contains_key(pool_account_id) {
			true => {
				let assets = Self::pool_assets(pool_account_id);
				Some(vec![assets.0, assets.1])
			}
			false => None,
		}
	}

	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Balance) -> Balance {
		let pair_account = Self::get_pair_id(AssetPair {
			asset_out: asset_a,
			asset_in: asset_b,
		});

		let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

		hydra_dx_math::calculate_spot_price(asset_a_reserve, asset_b_reserve, amount)
			.unwrap_or_else(|_| Balance::zero())
	}

	/// Validate a sell. Perform all necessary checks and calculations.
	/// No storage changes are performed yet.
	///
	/// Return `AMMTransfer` with all info needed to execute the transaction.
	fn validate_sell(
		who: &T::AccountId,
		assets: AssetPair,
		amount: Balance,
		min_bought: Balance,
		discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>, sp_runtime::DispatchError> {
		ensure!(Self::exists(assets), Error::<T>::TokenPoolNotFound);

		ensure!(
			T::Currency::free_balance(assets.asset_in, who) >= amount,
			Error::<T>::InsufficientAssetBalance
		);

		// If discount, pool for Sell asset and native asset must exist
		if discount {
			ensure!(
				Self::exists(AssetPair {
					asset_in: assets.asset_in,
					asset_out: T::NativeAssetId::get()
				}),
				Error::<T>::CannotApplyDiscount
			);
		}

		let pair_account = Self::get_pair_id(assets);

		let asset_in_reserve = T::Currency::free_balance(assets.asset_in, &pair_account);
		let asset_out_reserve = T::Currency::free_balance(assets.asset_out, &pair_account);

		ensure!(
			amount <= asset_in_reserve / MAX_IN_RATIO,
			Error::<T>::MaxInRatioExceeded
		);

		let amount_out = hydra_dx_math::calculate_out_given_in(asset_in_reserve, asset_out_reserve, amount)
			.map_err(|_| Error::<T>::SellAssetAmountInvalid)?;

		let transfer_fee = if discount {
			Self::calculate_discounted_fee(amount_out)?
		} else {
			Self::calculate_fee(amount_out)?
		};

		let amount_out_without_fee = amount_out
			.checked_sub(transfer_fee)
			.ok_or(Error::<T>::SellAssetAmountInvalid)?;

		ensure!(asset_out_reserve > amount_out, Error::<T>::InsufficientAssetBalance);

		ensure!(min_bought <= amount_out_without_fee, Error::<T>::AssetAmountNotReachedLimit);

		let discount_fee = if discount {
			let native_asset = T::NativeAssetId::get();

			let native_pair_account = Self::get_pair_id(AssetPair {
				asset_in: assets.asset_in,
				asset_out: native_asset,
			});

			let native_reserve = T::Currency::free_balance(native_asset, &native_pair_account);
			let asset_reserve = T::Currency::free_balance(assets.asset_in, &native_pair_account);

			let native_fee_spot_price =
				hydra_dx_math::calculate_spot_price(asset_reserve, native_reserve, transfer_fee)
					.map_err(|_| Error::<T>::CannotApplyDiscount)?;

			ensure!(
				T::Currency::free_balance(native_asset, who) >= native_fee_spot_price,
				Error::<T>::InsufficientNativeCurrencyBalance
			);

			native_fee_spot_price
		} else {
			Balance::zero()
		};

		let transfer = AMMTransfer {
			origin: who.clone(),
			assets,
			amount,
			amount_out: amount_out_without_fee,
			discount,
			discount_amount: discount_fee,
			fee: (assets.asset_out, transfer_fee),
		};

		Ok(transfer)
	}

	/// Execute sell. validate_sell must be called first.
	/// Perform necessary storage/state changes.
	/// Note : the execution should not return error as everything was previously verified and validated.
	#[transactional]
	fn execute_sell(transfer: &AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>) -> DispatchResult {
		let pair_account = Self::get_pair_id(transfer.assets);

		if transfer.discount && transfer.discount_amount > 0u128 {
			let native_asset = T::NativeAssetId::get();
			T::Currency::withdraw(native_asset, &transfer.origin, transfer.discount_amount)?;
		}

		T::Currency::transfer(
			transfer.assets.asset_in,
			&transfer.origin,
			&pair_account,
			transfer.amount,
		)?;
		T::Currency::transfer(
			transfer.assets.asset_out,
			&pair_account,
			&transfer.origin,
			transfer.amount_out,
		)?;

		Self::deposit_event(Event::<T>::SellExecuted(
			transfer.origin.clone(),
			transfer.assets.asset_in,
			transfer.assets.asset_out,
			transfer.amount,
			transfer.amount_out,
			transfer.fee.0,
			transfer.fee.1,
		));

		Ok(())
	}

	/// Validate a buy. Perform all necessary checks and calculations.
	/// No storage changes are performed yet.
	///
	/// Return `AMMTransfer` with all info needed to execute the transaction.
	fn validate_buy(
		who: &T::AccountId,
		assets: AssetPair,
		amount: Balance,
		max_limit: Balance,
		discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>, DispatchError> {
		ensure!(Self::exists(assets), Error::<T>::TokenPoolNotFound);

		let pair_account = Self::get_pair_id(assets);

		let asset_out_reserve = T::Currency::free_balance(assets.asset_out, &pair_account);
		let asset_in_reserve = T::Currency::free_balance(assets.asset_in, &pair_account);

		ensure!(asset_out_reserve > amount, Error::<T>::InsufficientPoolAssetBalance);

		ensure!(
			amount <= asset_out_reserve / MAX_OUT_RATIO,
			Error::<T>::MaxOutRatioExceeded
		);

		// If discount, pool for Sell asset and native asset must exist
		if discount {
			ensure!(
				Self::exists(AssetPair {
					asset_in: assets.asset_out,
					asset_out: T::NativeAssetId::get()
				}),
				Error::<T>::CannotApplyDiscount
			);
		}

		let buy_price = hydra_dx_math::calculate_in_given_out(asset_out_reserve, asset_in_reserve, amount)
			.map_err(|_| Error::<T>::BuyAssetAmountInvalid)?;

		let transfer_fee = if discount {
			Self::calculate_discounted_fee(buy_price)?
		} else {
			Self::calculate_fee(buy_price)?
		};

		let buy_price_with_fee = buy_price
			.checked_add(transfer_fee)
			.ok_or(Error::<T>::BuyAssetAmountInvalid)?;

		ensure!(max_limit >= buy_price_with_fee, Error::<T>::AssetAmountExceededLimit);

		ensure!(
			T::Currency::free_balance(assets.asset_in, who) >= buy_price_with_fee,
			Error::<T>::InsufficientAssetBalance
		);

		let discount_fee = if discount {
			let native_asset = T::NativeAssetId::get();

			let native_pair_account = Self::get_pair_id(AssetPair {
				asset_in: assets.asset_out,
				asset_out: native_asset,
			});

			let native_reserve = T::Currency::free_balance(native_asset, &native_pair_account);
			let asset_reserve = T::Currency::free_balance(assets.asset_out, &native_pair_account);

			let native_fee_spot_price =
				hydra_dx_math::calculate_spot_price(asset_reserve, native_reserve, transfer_fee)
					.map_err(|_| Error::<T>::CannotApplyDiscount)?;

			ensure!(
				T::Currency::free_balance(native_asset, who) >= native_fee_spot_price,
				Error::<T>::InsufficientNativeCurrencyBalance
			);
			native_fee_spot_price
		} else {
			Balance::zero()
		};

		let transfer = AMMTransfer {
			origin: who.clone(),
			assets,
			amount,
			amount_out: buy_price,
			discount,
			discount_amount: discount_fee,
			fee: (assets.asset_in, transfer_fee),
		};

		Ok(transfer)
	}

	/// Execute buy. validate_buy must be called first.
	/// Perform necessary storage/state changes.
	/// Note : the execution should not return error as everything was previously verified and validated.
	#[transactional]
	fn execute_buy(transfer: &AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>) -> DispatchResult {
		let pair_account = Self::get_pair_id(transfer.assets);

		if transfer.discount && transfer.discount_amount > 0 {
			let native_asset = T::NativeAssetId::get();
			T::Currency::withdraw(native_asset, &transfer.origin, transfer.discount_amount)?;
		}

		T::Currency::transfer(
			transfer.assets.asset_out,
			&pair_account,
			&transfer.origin,
			transfer.amount,
		)?;
		T::Currency::transfer(
			transfer.assets.asset_in,
			&transfer.origin,
			&pair_account,
			transfer.amount_out + transfer.fee.1,
		)?;

		Self::deposit_event(Event::<T>::BuyExecuted(
			transfer.origin.clone(),
			transfer.assets.asset_out,
			transfer.assets.asset_in,
			transfer.amount,
			transfer.amount_out,
			transfer.fee.0,
			transfer.fee.1,
		));

		Ok(())
	}
}
