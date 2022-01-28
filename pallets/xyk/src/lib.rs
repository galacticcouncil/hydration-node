// This file is part of Basilisk-node.

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

use frame_support::sp_runtime::{traits::Zero, DispatchError};
use frame_support::{dispatch::DispatchResult, ensure, traits::Get, transactional};
use frame_system::ensure_signed;
use hydradx_traits::{AMMTransfer, AssetPairAccountIdFor, CanCreatePool, OnCreatePoolHandler, OnTradeHandler, AMM};
use primitives::{asset::AssetPair, AssetId, Balance, Price};
use sp_std::{vec, vec::Vec};

use frame_support::sp_runtime::FixedPointNumber;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::Amount;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

mod impls;
pub mod weights;

pub use impls::XYKSpotPrice;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;
	use hydradx_traits::ShareTokenRegistry;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Registry support
		type AssetRegistry: ShareTokenRegistry<AssetId, Vec<u8>, Balance, DispatchError>;

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
		type GetExchangeFee: Get<(u32, u32)>;

		/// Minimum trading limit
		#[pallet::constant]
		type MinTradingLimit: Get<Balance>;

		/// Minimum pool liquidity
		#[pallet::constant]
		type MinPoolLiquidity: Get<Balance>;

		/// Max fraction of pool to sell in single transaction
		#[pallet::constant]
		type MaxInRatio: Get<u128>;

		/// Max fraction of pool to buy in single transaction
		#[pallet::constant]
		type MaxOutRatio: Get<u128>;

		/// Called to ensure that pool can be created
		type CanCreatePool: CanCreatePool<AssetId>;

		/// AMM handlers
		type AMMHandler: OnCreatePoolHandler<AssetId> + OnTradeHandler<AssetId, Balance>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// It is not allowed to create a pool between same assets.
		CannotCreatePoolWithSameAssets,

		/// Liquidity has not reached the required minimum.
		InsufficientLiquidity,

		/// Amount is less than min trading limit.
		InsufficientTradingAmount,

		/// Liquidity is zero.
		ZeroLiquidity,

		/// It is not allowed to create a pool with zero initial price.
		ZeroInitialPrice,

		/// Overflow
		CreatePoolAssetAmountInvalid,

		/// Overflow
		InvalidMintedLiquidity, // No tests - but it is currently not possible this error to occur due to previous checks in the code.

		/// Overflow
		InvalidLiquidityAmount, // no tests - it is currently not possible this error to occur due to previous checks in the code.

		/// Asset amount has exceeded given limit.
		AssetAmountExceededLimit,

		/// Asset amount has not reached given limit.
		AssetAmountNotReachedLimit,

		/// Asset balance is not sufficient.
		InsufficientAssetBalance,

		/// Not enough asset liquidity in the pool.
		InsufficientPoolAssetBalance,

		/// Not enough core asset liquidity in the pool.
		InsufficientNativeCurrencyBalance,

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
		FeeAmountInvalid,

		/// Overflow
		CannotApplyDiscount,

		/// Max fraction of pool to buy in single transaction has been exceeded.
		MaxOutRatioExceeded,
		/// Max fraction of pool to sell in single transaction has been exceeded.
		MaxInRatioExceeded,

		/// Overflow
		Overflow,

		/// Pool cannot be created due to outside factors.
		CannotCreatePool,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New liquidity was provided to the pool. [who, asset a, asset b, amount a, amount b]
		LiquidityAdded(T::AccountId, AssetId, AssetId, Balance, Balance),

		/// Liquidity was removed from the pool. [who, asset a, asset b, shares]
		LiquidityRemoved(T::AccountId, AssetId, AssetId, Balance),

		/// Pool was created. [who, asset a, asset b, initial shares amount, share token, pool account id]
		PoolCreated(T::AccountId, AssetId, AssetId, Balance, AssetId, T::AccountId),

		/// Pool was destroyed. [who, asset a, asset b, share token, pool account id]
		PoolDestroyed(T::AccountId, AssetId, AssetId, AssetId, T::AccountId),

		/// Asset sale executed. [who, asset in, asset out, amount, sale price, fee asset, fee amount, pool account id]
		SellExecuted(
			T::AccountId,
			AssetId,
			AssetId,
			Balance,
			Balance,
			AssetId,
			Balance,
			T::AccountId,
		),

		/// Asset purchase executed. [who, asset out, asset in, amount, buy price, fee asset, fee amount]
		BuyExecuted(
			T::AccountId,
			AssetId,
			AssetId,
			Balance,
			Balance,
			AssetId,
			Balance,
			T::AccountId,
		),
	}

	/// Asset id storage for shared pool tokens
	#[pallet::storage]
	#[pallet::getter(fn share_token)]
	pub(crate) type ShareToken<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, AssetId, ValueQuery>;

	/// Total liquidity in a pool.
	#[pallet::storage]
	#[pallet::getter(fn total_liquidity)]
	pub(crate) type TotalLiquidity<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Asset pair in a pool.
	#[pallet::storage]
	#[pallet::getter(fn pool_assets)]
	pub(crate) type PoolAssets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (AssetId, AssetId), OptionQuery>;

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
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::CanCreatePool::can_create(asset_a, asset_b),
				Error::<T>::CannotCreatePool
			);

			ensure!(amount >= T::MinPoolLiquidity::get(), Error::<T>::InsufficientLiquidity);

			ensure!(!(initial_price == Price::zero()), Error::<T>::ZeroInitialPrice);

			ensure!(asset_a != asset_b, Error::<T>::CannotCreatePoolWithSameAssets);

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(!Self::exists(asset_pair), Error::<T>::TokenPoolAlreadyExists);

			let asset_b_amount = initial_price
				.checked_mul_int(amount)
				.ok_or(Error::<T>::CreatePoolAssetAmountInvalid)?;

			ensure!(
				asset_b_amount >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidity
			);

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

			let share_token = T::AssetRegistry::get_or_create_shared_asset(
				token_name,
				vec![asset_a, asset_b],
				T::MinPoolLiquidity::get(),
			)?;

			let _ = T::AMMHandler::on_create_pool(asset_pair.asset_in, asset_pair.asset_out)?;

			<ShareToken<T>>::insert(&pair_account, &share_token);
			<PoolAssets<T>>::insert(&pair_account, (asset_a, asset_b));

			Self::deposit_event(Event::PoolCreated(
				who.clone(),
				asset_a,
				asset_b,
				shares_added,
				share_token,
				pair_account.clone(),
			));

			T::Currency::transfer(asset_a, &who, &pair_account, amount)?;
			T::Currency::transfer(asset_b, &who, &pair_account, asset_b_amount)?;

			T::Currency::deposit(share_token, &who, shares_added)?;

			<TotalLiquidity<T>>::insert(&pair_account, shares_added);

			Ok(())
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
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(Self::exists(asset_pair), Error::<T>::TokenPoolNotFound);

			ensure!(
				amount_a >= T::MinTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			ensure!(!amount_b_max_limit.is_zero(), Error::<T>::ZeroLiquidity);

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

			let account_shares = T::Currency::free_balance(share_token, &who);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);
			let total_liquidity = Self::total_liquidity(&pair_account);

			let amount_b_required =
				hydra_dx_math::xyk::calculate_liquidity_in(asset_a_reserve, asset_b_reserve, amount_a)
					.map_err(|_| Error::<T>::AddAssetAmountInvalid)?;

			let shares_added = if asset_a < asset_b { amount_a } else { amount_b_required };

			ensure!(
				amount_b_required <= amount_b_max_limit,
				Error::<T>::AssetAmountExceededLimit
			);

			ensure!(!shares_added.is_zero(), Error::<T>::InvalidMintedLiquidity);

			// Make sure that account share liquidity is at least MinPoolLiquidity
			ensure!(
				account_shares
					.checked_add(shares_added)
					.ok_or(Error::<T>::InvalidMintedLiquidity)?
					>= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidity
			);

			let liquidity_amount = total_liquidity
				.checked_add(shares_added)
				.ok_or(Error::<T>::InvalidLiquidityAmount)?;

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

			Ok(())
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
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(!liquidity_amount.is_zero(), Error::<T>::ZeroLiquidity);

			ensure!(Self::exists(asset_pair), Error::<T>::TokenPoolNotFound);

			let pair_account = Self::get_pair_id(asset_pair);

			let share_token = Self::share_token(&pair_account);

			let total_shares = Self::total_liquidity(&pair_account);

			let account_shares = T::Currency::free_balance(share_token, &who);

			ensure!(total_shares >= liquidity_amount, Error::<T>::InsufficientLiquidity);

			ensure!(account_shares >= liquidity_amount, Error::<T>::InsufficientAssetBalance);

			// Account's liquidity left should be either 0 or at least MinPoolLiquidity
			ensure!(
				(account_shares.saturating_sub(liquidity_amount)) >= T::MinPoolLiquidity::get()
					|| (account_shares == liquidity_amount),
				Error::<T>::InsufficientLiquidity
			);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

			let liquidity_out = hydra_dx_math::xyk::calculate_liquidity_out(
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
				<TotalLiquidity<T>>::remove(&pair_account);

				Self::deposit_event(Event::PoolDestroyed(who, asset_a, asset_b, share_token, pair_account));
			}

			Ok(())
		}

		/// Trade asset in for asset out.
		///
		/// Executes a swap of `asset_in` for `asset_out`. Price is determined by the liquidity pool.
		///
		/// `max_limit` - minimum amount of `asset_out` / amount of asset_out to be obtained from the pool in exchange for `asset_in`.
		///
		/// Emits `SellExecuted` when successful.
		#[pallet::weight(<T as Config>::WeightInfo::sell() + <T as Config>::AMMHandler::on_trade_weight())]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: AssetId,
			asset_out: AssetId,
			amount: Balance,
			max_limit: Balance,
			discount: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_, _, _, _>>::sell(&who, AssetPair { asset_in, asset_out }, amount, max_limit, discount)?;

			Ok(())
		}

		/// Trade asset in for asset out.
		///
		/// Executes a swap of `asset_in` for `asset_out`. Price is determined by the liquidity pool.
		///
		/// `max_limit` - maximum amount of `asset_in` to be sold in exchange for `asset_out`.
		///
		/// Emits `BuyExecuted` when successful.
		#[pallet::weight(<T as Config>::WeightInfo::buy() + <T as Config>::AMMHandler::on_trade_weight())]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: AssetId,
			asset_in: AssetId,
			amount: Balance,
			max_limit: Balance,
			discount: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_, _, _, _>>::buy(&who, AssetPair { asset_in, asset_out }, amount, max_limit, discount)?;

			Ok(())
		}
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
		Ok(
			hydra_dx_math::fee::calculate_pool_trade_fee(amount, (7, 10_000)) // 0.07%
				.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?,
		)
	}

	/// Calculate trade fee
	fn calculate_fee(amount: Balance) -> Result<Balance, DispatchError> {
		let fee = T::GetExchangeFee::get();
		Ok(hydra_dx_math::fee::calculate_pool_trade_fee(amount, (fee.0, fee.1))
			.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?)
	}

	pub fn pair_account_from_assets(asset_a: AssetId, asset_b: AssetId) -> T::AccountId {
		T::AssetPairAccountId::from_assets(asset_a, asset_b, "xyk")
	}
}

// Implementation of AMM API which makes possible to plug the AMM pool into the exchange pallet.
impl<T: Config> AMM<T::AccountId, AssetId, AssetPair, Balance> for Pallet<T> {
	fn exists(assets: AssetPair) -> bool {
		<ShareToken<T>>::contains_key(&Self::get_pair_id(assets))
	}

	fn get_pair_id(assets: AssetPair) -> T::AccountId {
		Self::pair_account_from_assets(assets.asset_in, assets.asset_out)
	}

	fn get_share_token(assets: AssetPair) -> AssetId {
		let pair_account = Self::get_pair_id(assets);
		Self::share_token(&pair_account)
	}

	fn get_pool_assets(pool_account_id: &T::AccountId) -> Option<Vec<AssetId>> {
		let maybe_assets = <PoolAssets<T>>::get(pool_account_id);
		maybe_assets.map(|assets| vec![assets.0, assets.1])
	}

	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Balance) -> Balance {
		let pair_account = Self::get_pair_id(AssetPair {
			asset_out: asset_a,
			asset_in: asset_b,
		});

		let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

		hydra_dx_math::xyk::calculate_spot_price(asset_a_reserve, asset_b_reserve, amount)
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
		ensure!(
			amount >= T::MinTradingLimit::get(),
			Error::<T>::InsufficientTradingAmount
		);

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
			amount
				<= asset_in_reserve
					.checked_div(T::MaxInRatio::get())
					.ok_or(Error::<T>::Overflow)?,
			Error::<T>::MaxInRatioExceeded
		);

		let amount_out = hydra_dx_math::xyk::calculate_out_given_in(asset_in_reserve, asset_out_reserve, amount)
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

		ensure!(
			min_bought <= amount_out_without_fee,
			Error::<T>::AssetAmountNotReachedLimit
		);

		let discount_fee = if discount {
			let native_asset = T::NativeAssetId::get();

			let native_pair_account = Self::get_pair_id(AssetPair {
				asset_in: assets.asset_in,
				asset_out: native_asset,
			});

			let native_reserve = T::Currency::free_balance(native_asset, &native_pair_account);
			let asset_reserve = T::Currency::free_balance(assets.asset_in, &native_pair_account);

			let native_fee_spot_price =
				hydra_dx_math::xyk::calculate_spot_price(asset_reserve, native_reserve, transfer_fee)
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

		let total_liquidity = Self::total_liquidity(&pair_account);
		T::AMMHandler::on_trade(
			transfer.assets.asset_in,
			transfer.assets.asset_out,
			transfer.amount,
			transfer.amount_out,
			total_liquidity,
		);

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
			pair_account,
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
		ensure!(
			amount >= T::MinTradingLimit::get(),
			Error::<T>::InsufficientTradingAmount
		);

		ensure!(Self::exists(assets), Error::<T>::TokenPoolNotFound);

		let pair_account = Self::get_pair_id(assets);

		let asset_out_reserve = T::Currency::free_balance(assets.asset_out, &pair_account);
		let asset_in_reserve = T::Currency::free_balance(assets.asset_in, &pair_account);

		ensure!(asset_out_reserve > amount, Error::<T>::InsufficientPoolAssetBalance);

		ensure!(
			amount
				<= asset_out_reserve
					.checked_div(T::MaxOutRatio::get())
					.ok_or(Error::<T>::Overflow)?,
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

		let buy_price = hydra_dx_math::xyk::calculate_in_given_out(asset_out_reserve, asset_in_reserve, amount)
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
				hydra_dx_math::xyk::calculate_spot_price(asset_reserve, native_reserve, transfer_fee)
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

		let total_liquidity = Self::total_liquidity(&pair_account);
		T::AMMHandler::on_trade(
			transfer.assets.asset_in,
			transfer.assets.asset_out,
			transfer.amount,
			transfer.amount_out,
			total_liquidity,
		);

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
			pair_account,
		));

		Ok(())
	}

	fn get_min_trading_limit() -> Balance {
		T::MinTradingLimit::get()
	}

	fn get_min_pool_liquidity() -> Balance {
		T::MinPoolLiquidity::get()
	}

	fn get_max_in_ratio() -> u128 {
		T::MaxInRatio::get()
	}

	fn get_max_out_ratio() -> u128 {
		T::MaxOutRatio::get()
	}

	fn get_fee(_pool_account_id: &T::AccountId) -> (u32, u32) {
		T::GetExchangeFee::get()
	}
}

pub struct AllowAllPools();

impl CanCreatePool<AssetId> for AllowAllPools {
	fn can_create(_asset_a: AssetId, _asset_b: AssetId) -> bool {
		true
	}
}
