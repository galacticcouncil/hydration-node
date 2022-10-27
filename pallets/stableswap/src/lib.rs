// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Stableswap pallet
//!
//! Curve/stableswap AMM implementation.
//!
//! ### Terminology
//!
//! * **LP** - liquidity provider
//! * **Share Token** - a token representing share asset of specific pool. Each pool has its own share token.
//! * **Amplification** - curve AMM pool amplification parameter
//!
//! ## Assumptions
//!
//! Maximum number of assets in pool is 5.
//!
//! A pool can be created only by allowed `CreatePoolOrigin`.
//!
//! First LP to provided liquidity must add initial liquidity of all pool assets. Subsequent calls to add_liquidity, LP can provide only 1 asset.
//!
//! Initial liquidity is first liquidity added to the pool (that is first call of `add_liquidity`).
//!
//! LP is given certain amount of shares by minting a pool's share token.
//!
//! When LP decides to withdraw liquidity, it receives selected asset.
//!

#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::{ensure, transactional};
use hydradx_traits::{AccountIdFor, Registry};
use sp_runtime::traits::Zero;
use sp_runtime::{ArithmeticError, DispatchError, Permill};
use sp_std::prelude::*;

pub use pallet::*;

mod trade_execution;
pub mod types;
pub mod weights;

pub use trade_execution::*;

use crate::types::{AssetLiquidity, Balance, PoolInfo};
use orml_traits::MultiCurrency;
use sp_std::collections::btree_map::BTreeMap;
use weights::WeightInfo;

#[cfg(test)]
pub(crate) mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

/// Stableswap share token and account id identifier.
/// Used as identifier to create share token unique names and account ids.
pub const POOL_IDENTIFIER: &[u8] = b"sts";

pub const MAX_ASSETS_IN_POOL: u32 = 5;

const D_ITERATIONS: u8 = hydra_dx_math::stableswap::MAX_D_ITERATIONS;
const Y_ITERATIONS: u8 = hydra_dx_math::stableswap::MAX_Y_ITERATIONS;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use core::ops::RangeInclusive;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Zero;
	use sp_runtime::ArithmeticError;
	use sp_runtime::Permill;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Ord
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Account ID constructor
		type ShareAccountId: AccountIdFor<Vec<Self::AssetId>, AccountId = Self::AccountId>;

		/// Asset registry mechanism
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;

		/// The origin which can create a new pool
		type CreatePoolOrigin: EnsureOrigin<Self::Origin>;

		/// Minimum pool liquidity
		#[pallet::constant]
		type MinPoolLiquidity: Get<Balance>;

		/// Minimum trading amount
		#[pallet::constant]
		type MinTradingLimit: Get<Balance>;

		/// Amplification inclusive range. Pool's amp can be selected from the range only.
		#[pallet::constant]
		type AmplificationRange: Get<RangeInclusive<u16>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Existing pools
	#[pallet::storage]
	#[pallet::getter(fn pools)]
	pub type Pools<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, PoolInfo<T::AssetId>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was created.
		PoolCreated {
			pool_id: T::AssetId,
			assets: Vec<T::AssetId>,
			amplification: u16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		},
		/// Pool parameters has been updated.
		PoolUpdated {
			pool_id: T::AssetId,
			amplification: u16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		},
		/// Liquidity of an asset was added to a pool.
		LiquidityAdded {
			pool_id: T::AssetId,
			who: T::AccountId,
			shares: Balance,
			assets: Vec<AssetLiquidity<T::AssetId>>,
		},
		/// Liquidity removed.
		LiquidityRemoved {
			pool_id: T::AssetId,
			who: T::AccountId,
			shares: Balance,
			asset: T::AssetId,
			amount: Balance,
			fee: Balance,
		},
		/// Sell trade executed. Trade fee paid in asset leaving the pool (already subtracted from amount_out).
		SellExecuted {
			who: T::AccountId,
			pool_id: T::AssetId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
			fee: Balance,
		},
		/// Buy trade executed. Trade fee paid in asset entering the pool (already included in amount_in).
		BuyExecuted {
			who: T::AccountId,
			pool_id: T::AssetId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
			fee: Balance,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Creating a pool with same assets is not allowed.
		SameAssets,

		/// Maximum number of assets has been exceeded.
		MaxAssetsExceeded,

		/// A pool with given assets does not exist.
		PoolNotFound,

		/// A pool with given assets already exists.
		PoolExists,

		/// Asset is not in the pool.
		AssetNotInPool,

		/// Asset is already in the pool.
		AssetInPool,

		/// Share asset is not registered in Registry.
		ShareAssetNotRegistered,

		/// Share asset is amount assets when creating a pool.
		ShareAssetInPoolAssets,

		/// One or more assets are not registered in AssetRegistry
		AssetNotRegistered,

		/// Invalid asset amount provided. Amount must be greater than zero.
		InvalidAssetAmount,

		/// Balance of an asset is not sufficient to perform a trade.
		InsufficientBalance,

		/// Balance of a share asset is not sufficient to withdraw liquidity.
		InsufficientShares,

		/// Liquidity has not reached the required minimum.
		InsufficientLiquidity,

		/// Insufficient liquidity left in the pool after withdrawal.
		InsufficientLiquidityRemaining,

		/// Amount is less than the minimum trading amount configured.
		InsufficientTradingAmount,

		/// Minimum limit has not been reached during trade.
		BuyLimitNotReached,

		/// Maximum limit has been exceeded during trade.
		SellLimitExceeded,

		/// Initial liquidity of asset must be > 0.
		InvalidInitialLiquidity,

		/// Account balance is too low.
		BalanceTooLow,

		/// Amplification is outside configured range.
		InvalidAmplification,

		/// Remaining balance of share asset is below asset's existential deposit.
		InsufficientShareBalance,

		/// No pool parameters to update are provided.
		NothingToUpdate,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a stableswap pool with given list of asset
		///
		/// All assets must be correctly registered in `T::AssetRegistry`.
		/// Note that this does not seed the pool with liquidity. Use `add_liquidity` to provide
		/// initial liquidity.
		///
		/// Parameters:
		/// - `origin`: Must be T::CreatePoolOrigin
		/// - `assets`: List of Asset ids
		/// - `amplification`: Pool amplification
		/// - `trade_fee`: trade fee to be applied in sell/buy trades
		/// - `withdraw_fee`: fee to be applied when removing liquidity
		///
		/// Emits `PoolCreated` event if successful.
		#[pallet::weight(<T as Config>::WeightInfo::create_pool())]
		#[transactional]
		pub fn create_pool(
			origin: OriginFor<T>,
			share_asset: T::AssetId,
			assets: Vec<T::AssetId>,
			amplification: u16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		) -> DispatchResult {
			T::CreatePoolOrigin::ensure_origin(origin)?;

			let pool_id = Self::do_create_pool(share_asset, &assets, amplification, trade_fee, withdraw_fee)?;

			Self::deposit_event(Event::PoolCreated {
				pool_id,
				assets,
				amplification,
				trade_fee,
				withdraw_fee,
			});

			Ok(())
		}

		/// Update given stableswap pool's parameters.
		///
		/// Updates one or more parameters of stablesswap pool ( amplification, trade fee, withdraw fee).
		///
		/// If all parameters are none, `NothingToUpdate` error is returned.
		///
		/// if pool does not exist, `PoolNotFound` is returned.
		///
		/// Parameters:
		/// - `origin`: Must be T::CreatePoolOrigin
		/// - `pool_id`: pool to update
		/// - `amplification`: new pool amplification or None
		/// - `trade_fee`: new trade fee or None
		/// - `withdraw_fee`: new withdraw fee or None
		///
		/// Emits `PoolUpdated` event if successful.
		#[pallet::weight(<T as Config>::WeightInfo::update_pool())]
		#[transactional]
		pub fn update_pool(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			amplification: Option<u16>,
			trade_fee: Option<Permill>,
			withdraw_fee: Option<Permill>,
		) -> DispatchResult {
			T::CreatePoolOrigin::ensure_origin(origin)?;

			ensure!(
				amplification.is_some() || trade_fee.is_some() || withdraw_fee.is_some(),
				Error::<T>::NothingToUpdate
			);

			Pools::<T>::try_mutate(&pool_id, |maybe_pool| -> DispatchResult {
				let mut pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;

				pool.amplification = amplification.unwrap_or(pool.amplification);
				ensure!(
					T::AmplificationRange::get().contains(&pool.amplification),
					Error::<T>::InvalidAmplification
				);
				pool.trade_fee = trade_fee.unwrap_or(pool.trade_fee);
				pool.withdraw_fee = withdraw_fee.unwrap_or(pool.withdraw_fee);
				Self::deposit_event(Event::PoolUpdated {
					pool_id,
					amplification: pool.amplification,
					trade_fee: pool.trade_fee,
					withdraw_fee: pool.withdraw_fee,
				});
				Ok(())
			})
		}

		/// Add liquidity to selected pool.
		///
		/// First call of `add_liquidity` adds "initial liquidity" of all assets.
		///
		/// If there is liquidity already in the pool, LP can provide liquidity of any number of pool assets.
		///
		/// LP must have sufficient amount of each assets.
		///
		/// Origin is given corresponding amount of shares.
		///
		/// Parameters:
		/// - `origin`: liquidity provider
		/// - `pool_id`: Pool Id
		/// - `assets`: asset id and liquidity amount provided
		///
		/// Emits `LiquidityAdded` event when successful.
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity())]
		#[transactional]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			assets: Vec<AssetLiquidity<T::AssetId>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let shares = Self::do_add_liquidity(&who, pool_id, &assets)?;

			Self::deposit_event(Event::LiquidityAdded {
				pool_id,
				who,
				shares,
				assets,
			});

			Ok(())
		}

		/// Remove liquidity from selected pool.
		///
		/// Withdraws liquidity of selected asset from a pool.
		///
		/// Share amount is burn and LP receives corresponding amount of chosen asset.
		///
		/// Withdraw fee is applied to the asset amount.
		///
		/// Parameters:
		/// - `origin`: liquidity provider
		/// - `pool_id`: Pool Id
		/// - `asset_id`: id of asset to receive
		/// - 'share_amount': amount of shares to withdraw
		///
		/// Emits `LiquidityRemoved` event when successful.
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity_one_asset())]
		#[transactional]
		pub fn remove_liquidity_one_asset(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			share_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(share_amount > Balance::zero(), Error::<T>::InvalidAssetAmount);

			let current_share_balance = T::Currency::free_balance(pool_id, &who);

			ensure!(current_share_balance >= share_amount, Error::<T>::InsufficientShares);

			ensure!(
				current_share_balance == share_amount
					|| current_share_balance.saturating_sub(share_amount) >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientShareBalance
			);

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let asset_idx = pool.find_asset(asset_id).ok_or(Error::<T>::AssetNotInPool)?;
			let pool_account = pool.pool_account::<T>();
			let balances = pool.balances::<T>();
			let share_issuance = T::Currency::total_issuance(pool_id);

			ensure!(
				share_issuance == share_amount
					|| share_issuance.saturating_sub(share_amount) >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidityRemaining
			);

			let (amount, fee) = hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
				&balances,
				share_amount,
				asset_idx,
				share_issuance,
				pool.amplification.into(),
				pool.withdraw_fee,
			)
			.ok_or(ArithmeticError::Overflow)?;

			T::Currency::withdraw(pool_id, &who, share_amount)?;
			T::Currency::transfer(asset_id, &pool_account, &who, amount)?;

			Self::deposit_event(Event::LiquidityRemoved {
				pool_id,
				who,
				shares: share_amount,
				asset: asset_id,
				amount,
				fee,
			});

			Ok(())
		}

		/// Execute a swap of `asset_in` for `asset_out` by specifying how much to put in.
		///
		/// Parameters:
		/// - `origin`: origin of the caller
		/// - `pool_id`: Id of a pool
		/// - `asset_in`: ID of asset sold to the pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `amount_in`: Amount of asset to be sold to the pool
		/// - `min_buy_amount`: Minimum amount required to receive
		///
		/// Emits `SellExecuted` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::sell())]
		#[transactional]
		pub fn sell(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			min_buy_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				amount_in >= T::MinTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= amount_in,
				Error::<T>::InsufficientBalance
			);

			let (amount_out, fee_amount) = Self::calculate_out_amount(pool_id, asset_in, asset_out, amount_in)?;

			ensure!(amount_out >= min_buy_amount, Error::<T>::BuyLimitNotReached);

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let pool_account = pool.pool_account::<T>();

			T::Currency::transfer(asset_in, &who, &pool_account, amount_in)?;
			T::Currency::transfer(asset_out, &pool_account, &who, amount_out)?;

			Self::deposit_event(Event::SellExecuted {
				who,
				pool_id,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				fee: fee_amount,
			});

			Ok(())
		}

		/// Execute a swap of `asset_in` for `asset_out` by specifying how much to get out.
		///
		/// Parameters:
		/// - `origin`:
		/// - `pool_id`: Id of a pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `asset_in`: ID of asset sold to the pool
		/// - `amount_out`: Amount of asset to receive from the pool
		/// - `max_sell_amount`: Maximum amount allowed to be sold
		///
		/// Emits `BuyExecuted` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::buy())]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_out: T::AssetId,
			asset_in: T::AssetId,
			amount_out: Balance,
			max_sell_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				amount_out >= T::MinTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			let (amount_in, fee_amount) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out)?;

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let pool_account = pool.pool_account::<T>();

			ensure!(amount_in <= max_sell_amount, Error::<T>::SellLimitExceeded);

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= amount_in,
				Error::<T>::InsufficientBalance
			);

			T::Currency::transfer(asset_in, &who, &pool_account, amount_in)?;
			T::Currency::transfer(asset_out, &pool_account, &who, amount_out)?;

			Self::deposit_event(Event::BuyExecuted {
				who,
				pool_id,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				fee: fee_amount,
			});

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	fn calculate_out_amount(
		pool_id: T::AssetId,
		asset_out: T::AssetId,
		asset_in: T::AssetId,
		amount_in: Balance,
	) -> Result<(Balance, Balance), DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		let index_in = pool.find_asset(asset_in).ok_or(Error::<T>::AssetNotInPool)?;
		let index_out = pool.find_asset(asset_out).ok_or(Error::<T>::AssetNotInPool)?;

		let balances = pool.balances::<T>();

		ensure!(balances[index_in] > Balance::zero(), Error::<T>::InsufficientLiquidity);
		ensure!(balances[index_out] > Balance::zero(), Error::<T>::InsufficientLiquidity);

		hydra_dx_math::stableswap::calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&balances,
			index_in,
			index_out,
			amount_in,
			pool.amplification.into(),
			pool.trade_fee,
		)
		.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	fn calculate_in_amount(
		pool_id: T::AssetId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
	) -> Result<(Balance, Balance), DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		let index_in = pool.find_asset(asset_in).ok_or(Error::<T>::AssetNotInPool)?;
		let index_out = pool.find_asset(asset_out).ok_or(Error::<T>::AssetNotInPool)?;

		let balances = pool.balances::<T>();

		ensure!(balances[index_out] > amount_out, Error::<T>::InsufficientLiquidity);
		ensure!(balances[index_in] > Balance::zero(), Error::<T>::InsufficientLiquidity);

		hydra_dx_math::stableswap::calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&balances,
			index_in,
			index_out,
			amount_out,
			pool.amplification.into(),
			pool.trade_fee,
		)
		.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	pub fn get_pool(pool_id: T::AssetId) -> Result<PoolInfo<T::AssetId>, DispatchError> {
		Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound.into())
	}

	pub fn do_create_pool(
		share_asset: T::AssetId,
		assets: &[T::AssetId],
		amplification: u16,
		trade_fee: Permill,
		withdraw_fee: Permill,
	) -> Result<T::AssetId, DispatchError> {
		ensure!(!Pools::<T>::contains_key(&share_asset), Error::<T>::PoolExists);
		ensure!(
			T::AssetRegistry::exists(share_asset),
			Error::<T>::ShareAssetNotRegistered
		);

		ensure!(!assets.contains(&share_asset), Error::<T>::ShareAssetInPoolAssets);

		let mut pool_assets = assets.to_vec();
		pool_assets.sort();

		let pool = PoolInfo {
			assets: pool_assets
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::MaxAssetsExceeded)?,
			amplification,
			trade_fee,
			withdraw_fee,
		};
		ensure!(pool.is_valid(), Error::<T>::SameAssets);
		ensure!(
			T::AmplificationRange::get().contains(&amplification),
			Error::<T>::InvalidAmplification
		);
		for asset in pool.assets.iter() {
			ensure!(T::AssetRegistry::exists(*asset), Error::<T>::AssetNotRegistered);
		}

		Pools::<T>::insert(&share_asset, pool);

		Ok(share_asset)
	}

	pub fn add_asset_to_existing_pool(pool_id: T::AssetId, asset_id: T::AssetId) -> DispatchResult {
		ensure!(T::AssetRegistry::exists(asset_id), Error::<T>::AssetNotRegistered);

		Pools::<T>::try_mutate(&pool_id, |maybe_pool| -> DispatchResult {
			let pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;
			ensure!(pool.find_asset(asset_id).is_none(), Error::<T>::AssetInPool);

			let mut assets = pool.assets.to_vec();

			assets.push(asset_id);
			assets.sort();

			pool.assets = assets.try_into().map_err(|_| Error::<T>::MaxAssetsExceeded)?;

			//TODO: we might need to transfer to new pool account if account of the pool changes - depends how it is constructed in T::AccountIdFor

			Ok(())
		})
	}

	pub fn move_liquidity_to_pool(
		from: &T::AccountId,
		pool_id: T::AssetId,
		assets: &[AssetLiquidity<T::AssetId>],
	) -> DispatchResult {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		let pool_account = pool.pool_account::<T>();

		for asset in assets.iter() {
			ensure!(pool.find_asset(asset.asset_id).is_some(), Error::<T>::AssetNotInPool);
			T::Currency::transfer(asset.asset_id, &from, &pool_account, asset.amount)?;
		}

		Ok(())
	}

	pub fn deposit_shares(who: &T::AccountId, pool_id: T::AssetId, amount: Balance) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidAssetAmount);
		let current_share_balance = T::Currency::free_balance(pool_id, &who);

		ensure!(
			current_share_balance.saturating_add(amount) >= T::MinPoolLiquidity::get(),
			Error::<T>::InsufficientShareBalance
		);

		T::Currency::deposit(pool_id, &who, amount)
	}

	pub fn do_add_liquidity(
		who: &T::AccountId,
		pool_id: T::AssetId,
		assets: &[AssetLiquidity<T::AssetId>],
	) -> Result<Balance, DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
		ensure!(assets.len() <= pool.assets.len(), Error::<T>::MaxAssetsExceeded);
		let mut added_assets = BTreeMap::<T::AssetId, Balance>::new();
		for asset in assets.iter() {
			ensure!(
				asset.amount >= T::MinTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);
			ensure!(
				T::Currency::free_balance(asset.asset_id, &who) >= asset.amount,
				Error::<T>::InsufficientBalance
			);
			ensure!(pool.find_asset(asset.asset_id).is_some(), Error::<T>::AssetNotInPool);
			added_assets.insert(asset.asset_id, asset.amount);
		}

		let pool_account = pool.pool_account::<T>();
		let mut initial_reserves = Vec::new();
		let mut updated_reserves = Vec::new();
		for pool_asset in pool.assets.iter() {
			let reserve = T::Currency::free_balance(*pool_asset, &pool_account);
			initial_reserves.push(reserve);
			if let Some(liq_added) = added_assets.get(pool_asset) {
				updated_reserves.push(reserve.checked_add(*liq_added).ok_or(ArithmeticError::Overflow)?);
			} else {
				ensure!(!reserve.is_zero(), Error::<T>::InvalidInitialLiquidity);
				updated_reserves.push(reserve);
			}
		}

		let share_issuance = T::Currency::total_issuance(pool_id);
		let share_amount = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
			&initial_reserves,
			&updated_reserves,
			pool.amplification.into(),
			share_issuance,
		)
		.ok_or(ArithmeticError::Overflow)?;

		Self::deposit_shares(&who, pool_id, share_amount)?;

		Self::move_liquidity_to_pool(&who, pool_id, &assets)?;

		Ok(share_amount)
	}
}
