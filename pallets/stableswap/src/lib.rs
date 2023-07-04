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
//! A pool can be created only by allowed `AuthorityOrigin`.
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
use frame_support::{ensure, require_transactional, transactional};
use hydradx_traits::{AccountIdFor, Registry};
use sp_runtime::traits::{BlockNumberProvider, Zero};
use sp_runtime::{ArithmeticError, DispatchError, Permill, SaturatedConversion};
use sp_std::num::NonZeroU16;
use sp_std::prelude::*;

pub use pallet::*;

mod trade_execution;
pub mod types;
pub mod weights;

pub use trade_execution::*;

use crate::types::{AssetLiquidity, Balance, PoolInfo, Tradability};
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
	use sp_runtime::traits::{BlockNumberProvider, Zero};
	use sp_runtime::Permill;
	use sp_runtime::{ArithmeticError, SaturatedConversion};
	use sp_std::num::NonZeroU16;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Provider for the current block number.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

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

		/// Account ID constructor - pool account are derived from unique pool id
		type ShareAccountId: AccountIdFor<Self::AssetId, AccountId = Self::AccountId>;

		/// Asset registry mechanism
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;

		/// The origin which can create a new pool
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Minimum pool liquidity
		#[pallet::constant]
		type MinPoolLiquidity: Get<Balance>;

		/// Minimum trading amount
		#[pallet::constant]
		type MinTradingLimit: Get<Balance>;

		/// Amplification inclusive range. Pool's amp can be selected from the range only.
		#[pallet::constant]
		type AmplificationRange: Get<RangeInclusive<NonZeroU16>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Existing pools
	#[pallet::storage]
	#[pallet::getter(fn pools)]
	pub type Pools<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, PoolInfo<T::AssetId, T::BlockNumber>>;

	/// Tradability state of pool assets.
	#[pallet::storage]
	#[pallet::getter(fn asset_tradability)]
	pub type AssetTradability<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AssetId, Blake2_128Concat, T::AssetId, Tradability, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was created.
		PoolCreated {
			pool_id: T::AssetId,
			assets: Vec<T::AssetId>,
			amplification: NonZeroU16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		},
		/// Pool parameters has been updated.
		PoolUpdated {
			pool_id: T::AssetId,
			amplification: NonZeroU16,
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

		/// Aseet's tradable state has been updated.
		TradableStateUpdated {
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			state: Tradability,
		},

		///
		AmplificationUpdated {
			pool_id: T::AssetId,
			amplification: NonZeroU16,
			block: T::BlockNumber,
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

		/// Not allowed to perform an operation on given asset.
		NotAllowed,
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
		/// - `origin`: Must be T::AuthorityOrigin
		/// - `share_asset`: Preregistered share asset identifier
		/// - `assets`: List of Asset ids
		/// - `amplification`: Pool amplification
		/// - `trade_fee`: trade fee to be applied in sell/buy trades
		/// - `withdraw_fee`: fee to be applied when removing liquidity
		///
		/// Emits `PoolCreated` event if successful.
		#[pallet::call_index(0)]
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
			T::AuthorityOrigin::ensure_origin(origin)?;

			let amplification = NonZeroU16::new(amplification).ok_or(Error::<T>::InvalidAmplification)?;

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
		/// - `origin`: Must be T::AuthorityOrigin
		/// - `pool_id`: pool to update
		/// - `amplification`: new pool amplification or None
		/// - `trade_fee`: new trade fee or None
		/// - `withdraw_fee`: new withdraw fee or None
		///
		/// Emits `PoolUpdated` event if successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::update_pool())]
		#[transactional]
		pub fn update_pool(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			amplification: Option<u16>,
			trade_fee: Option<Permill>,
			withdraw_fee: Option<Permill>,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(
				amplification.is_some() || trade_fee.is_some() || withdraw_fee.is_some(),
				Error::<T>::NothingToUpdate
			);

			Pools::<T>::try_mutate(pool_id, |maybe_pool| -> DispatchResult {
				let mut pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;

				pool.amplification = if let Some(ampl) = amplification {
					NonZeroU16::new(ampl).ok_or(Error::<T>::InvalidAmplification)?
				} else {
					pool.amplification
				};
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

		/// Update given stableswap pool's parameters.
		///
		/// Updates one or more parameters of stablesswap pool ( amplification, trade fee, withdraw fee).
		///
		/// If all parameters are none, `NothingToUpdate` error is returned.
		///
		/// if pool does not exist, `PoolNotFound` is returned.
		///
		/// Parameters:
		/// - `origin`: Must be T::AuthorityOrigin
		/// - `pool_id`: pool to update
		/// - `amplification`: new pool amplification or None
		/// - `trade_fee`: new trade fee or None
		/// - `withdraw_fee`: new withdraw fee or None
		///
		/// Emits `PoolUpdated` event if successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_pool())]
		#[transactional]
		pub fn update_amplification(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			future_amplification: u16,
			block: T::BlockNumber,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			Pools::<T>::try_mutate(pool_id, |maybe_pool| -> DispatchResult {
				let mut pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;

				pool.amplification = pool.future_amplification;
				pool.future_amplification =
					NonZeroU16::new(future_amplification).ok_or(Error::<T>::InvalidAmplification)?;

				let current_block = T::BlockNumberProvider::current_block_number();
				ensure!(
					block > current_block && block > pool.future_amp_timestamp,
					Error::<T>::InvalidAmplification
				);

				pool.amp_timestamp = current_block;
				pool.future_amp_timestamp = block;

				ensure!(
					T::AmplificationRange::get().contains(&pool.future_amplification),
					Error::<T>::InvalidAmplification
				);
				Self::deposit_event(Event::AmplificationUpdated {
					pool_id,
					amplification: pool.future_amplification,
					block: pool.future_amp_timestamp,
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
		#[pallet::call_index(3)]
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
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity_one_asset())]
		#[transactional]
		pub fn remove_liquidity_one_asset(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			share_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				Self::is_asset_allowed(pool_id, asset_id, Tradability::REMOVE_LIQUIDITY),
				Error::<T>::NotAllowed
			);

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
			let pool_account = Self::pool_account(pool_id);
			let balances = pool.balances::<T>(&pool_account);
			let share_issuance = T::Currency::total_issuance(pool_id);

			ensure!(
				share_issuance == share_amount
					|| share_issuance.saturating_sub(share_amount) >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidityRemaining
			);

			let amplification = hydra_dx_math::stableswap::calculate_amplification(
				pool.amplification.get().into(),
				pool.future_amplification.get().into(),
				pool.amp_timestamp.saturated_into(),
				pool.future_amp_timestamp.saturated_into(),
				T::BlockNumberProvider::current_block_number().saturated_into(),
			);

			let (amount, fee) = hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
				&balances,
				share_amount,
				asset_idx,
				share_issuance,
				amplification,
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
		#[pallet::call_index(5)]
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
				Self::is_asset_allowed(pool_id, asset_in, Tradability::SELL)
					&& Self::is_asset_allowed(pool_id, asset_out, Tradability::BUY),
				Error::<T>::NotAllowed
			);

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

			let pool_account = Self::pool_account(pool_id);

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
		#[pallet::call_index(6)]
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
				Self::is_asset_allowed(pool_id, asset_in, Tradability::SELL)
					&& Self::is_asset_allowed(pool_id, asset_out, Tradability::BUY),
				Error::<T>::NotAllowed
			);

			ensure!(
				amount_out >= T::MinTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			let (amount_in, fee_amount) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out)?;

			let pool_account = Self::pool_account(pool_id);

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

		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_tradable_state())]
		#[transactional]
		pub fn set_asset_tradable_state(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			state: Tradability,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			AssetTradability::<T>::mutate(pool_id, asset_id, |current_state| {
				*current_state = state;
			});

			Self::deposit_event(Event::TradableStateUpdated {
				pool_id,
				asset_id,
				state,
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

		let pool_account = Self::pool_account(pool_id);
		let balances = pool.balances::<T>(&pool_account);

		ensure!(balances[index_in] > Balance::zero(), Error::<T>::InsufficientLiquidity);
		ensure!(balances[index_out] > Balance::zero(), Error::<T>::InsufficientLiquidity);

		let amplification = hydra_dx_math::stableswap::calculate_amplification(
			pool.amplification.get().into(),
			pool.future_amplification.get().into(),
			pool.amp_timestamp.saturated_into(),
			pool.future_amp_timestamp.saturated_into(),
			T::BlockNumberProvider::current_block_number().saturated_into(),
		);

		hydra_dx_math::stableswap::calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&balances,
			index_in,
			index_out,
			amount_in,
			amplification,
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

		let pool_account = Self::pool_account(pool_id);
		let balances = pool.balances::<T>(&pool_account);

		ensure!(balances[index_out] > amount_out, Error::<T>::InsufficientLiquidity);
		ensure!(balances[index_in] > Balance::zero(), Error::<T>::InsufficientLiquidity);

		let amplification = hydra_dx_math::stableswap::calculate_amplification(
			pool.amplification.get().into(),
			pool.future_amplification.get().into(),
			pool.amp_timestamp.saturated_into(),
			pool.future_amp_timestamp.saturated_into(),
			T::BlockNumberProvider::current_block_number().saturated_into(),
		);

		hydra_dx_math::stableswap::calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&balances,
			index_in,
			index_out,
			amount_out,
			amplification,
			pool.trade_fee,
		)
		.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	#[require_transactional]
	fn do_create_pool(
		share_asset: T::AssetId,
		assets: &[T::AssetId],
		amplification: NonZeroU16,
		trade_fee: Permill,
		withdraw_fee: Permill,
	) -> Result<T::AssetId, DispatchError> {
		ensure!(!Pools::<T>::contains_key(share_asset), Error::<T>::PoolExists);
		ensure!(
			T::AssetRegistry::exists(share_asset),
			Error::<T>::ShareAssetNotRegistered
		);

		ensure!(!assets.contains(&share_asset), Error::<T>::ShareAssetInPoolAssets);

		let block_number = T::BlockNumberProvider::current_block_number();

		let mut pool_assets = assets.to_vec();
		pool_assets.sort();

		let pool = PoolInfo {
			assets: pool_assets
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::MaxAssetsExceeded)?,
			amplification,
			future_amplification: amplification,
			amp_timestamp: block_number,
			future_amp_timestamp: block_number,
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

		Pools::<T>::insert(share_asset, pool);

		Ok(share_asset)
	}

	#[require_transactional]
	fn do_add_liquidity(
		who: &T::AccountId,
		pool_id: T::AssetId,
		assets: &[AssetLiquidity<T::AssetId>],
	) -> Result<Balance, DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
		ensure!(assets.len() <= pool.assets.len(), Error::<T>::MaxAssetsExceeded);
		let mut added_assets = BTreeMap::<T::AssetId, Balance>::new();
		for asset in assets.iter() {
			ensure!(
				Self::is_asset_allowed(pool_id, asset.asset_id, Tradability::ADD_LIQUIDITY),
				Error::<T>::NotAllowed
			);
			ensure!(
				asset.amount >= T::MinTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);
			ensure!(
				T::Currency::free_balance(asset.asset_id, who) >= asset.amount,
				Error::<T>::InsufficientBalance
			);
			ensure!(pool.find_asset(asset.asset_id).is_some(), Error::<T>::AssetNotInPool);
			added_assets.insert(asset.asset_id, asset.amount);
		}

		let pool_account = Self::pool_account(pool_id);
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

		let amplification = hydra_dx_math::stableswap::calculate_amplification(
			pool.amplification.get().into(),
			pool.future_amplification.get().into(),
			pool.amp_timestamp.saturated_into(),
			pool.future_amp_timestamp.saturated_into(),
			T::BlockNumberProvider::current_block_number().saturated_into(),
		);

		let share_issuance = T::Currency::total_issuance(pool_id);
		let share_amount = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
			&initial_reserves,
			&updated_reserves,
			amplification,
			share_issuance,
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(!share_amount.is_zero(), Error::<T>::InvalidAssetAmount);
		let current_share_balance = T::Currency::free_balance(pool_id, who);

		ensure!(
			current_share_balance.saturating_add(share_amount) >= T::MinPoolLiquidity::get(),
			Error::<T>::InsufficientShareBalance
		);

		T::Currency::deposit(pool_id, who, share_amount)?;

		for asset in assets.iter() {
			T::Currency::transfer(asset.asset_id, who, &pool_account, asset.amount)?;
		}

		Ok(share_amount)
	}

	fn is_asset_allowed(pool_id: T::AssetId, asset_id: T::AssetId, operation: Tradability) -> bool {
		AssetTradability::<T>::get(pool_id, asset_id).contains(operation)
	}

	fn pool_account(pool_id: T::AssetId) -> T::AccountId {
		T::ShareAccountId::from_assets(&pool_id, Some(POOL_IDENTIFIER))
	}
}
