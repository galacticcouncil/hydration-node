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
//! ## Overview
//!
//! Curve style AMM at is designed to provide highly efficient and low-slippage trades for stablecoins.
//!
//! ### Stableswap Hooks
//!
//! Stableswap pallet supports multiple hooks which are triggerred on certain operations:
//! - on_liquidity_changed - called when liquidity is added or removed from the pool
//! - on_trade - called when trade is executed
//!
//! This is currently used to update on-chain oracle.
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
use frame_system::pallet_prelude::BlockNumberFor;
use hydradx_traits::{registry::InspectRegistry, AccountIdFor};
pub use pallet::*;
use sp_runtime::traits::{BlockNumberProvider, Zero};
use sp_runtime::{ArithmeticError, DispatchError, Permill, SaturatedConversion};
use sp_std::num::NonZeroU16;
use sp_std::prelude::*;
use sp_std::vec;

mod trade_execution;
pub mod types;
pub mod weights;

pub use trade_execution::*;

use crate::types::{AssetAmount, Balance, PoolInfo, PoolState, StableswapHooks, Tradability};
use hydra_dx_math::stableswap::types::AssetReserve;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use orml_traits::MultiCurrency;
use sp_std::collections::btree_map::BTreeMap;
use weights::WeightInfo;

#[cfg(test)]
pub(crate) mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

#[cfg(feature = "runtime-benchmarks")]
pub use crate::types::BenchmarkHelper;

/// Stableswap account id identifier.
/// Used as identifier to create share token unique names and account id.
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
	use hydradx_traits::pools::DustRemovalAccountWhitelist;
	use sp_runtime::traits::{BlockNumberProvider, Zero};
	use sp_runtime::ArithmeticError;
	use sp_runtime::Permill;
	use sp_std::num::NonZeroU16;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Provider for the current block number.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

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

		/// Asset registry mechanism to check if asset is registered and retrieve asset decimals.
		type AssetInspection: InspectRegistry<Self::AssetId>;

		/// The origin which can create a new pool
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Account whitelist manager to exclude pool accounts from dusting mechanism.
		type DustAccountHandler: DustRemovalAccountWhitelist<Self::AccountId, Error = DispatchError>;

		/// Hooks are actions executed on add_liquidity, sell or buy.
		type Hooks: StableswapHooks<Self::AssetId>;

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

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self::AssetId>;
	}

	/// Existing pools
	#[pallet::storage]
	#[pallet::getter(fn pools)]
	pub type Pools<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, PoolInfo<T::AssetId, BlockNumberFor<T>>>;

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
			fee: Permill,
		},
		/// Pool fee has been updated.
		FeeUpdated { pool_id: T::AssetId, fee: Permill },
		/// Liquidity of an asset was added to a pool.
		LiquidityAdded {
			pool_id: T::AssetId,
			who: T::AccountId,
			shares: Balance,
			assets: Vec<AssetAmount<T::AssetId>>,
		},
		/// Liquidity removed.
		LiquidityRemoved {
			pool_id: T::AssetId,
			who: T::AccountId,
			shares: Balance,
			amounts: Vec<AssetAmount<T::AssetId>>,
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

		/// Asset's tradable state has been updated.
		TradableStateUpdated {
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			state: Tradability,
		},

		/// AAmplification of a pool has been scheduled to change.
		AmplificationChanging {
			pool_id: T::AssetId,
			current_amplification: NonZeroU16,
			final_amplification: NonZeroU16,
			start_block: BlockNumberFor<T>,
			end_block: BlockNumberFor<T>,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Creating a pool with same assets or less than 2 assets is not allowed.
		IncorrectAssets,

		/// Maximum number of assets has been exceeded.
		MaxAssetsExceeded,

		/// A pool with given assets does not exist.
		PoolNotFound,

		/// A pool with given assets already exists.
		PoolExists,

		/// Asset is not in the pool.
		AssetNotInPool,

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

		/// Amplification is outside configured range.
		InvalidAmplification,

		/// Remaining balance of share asset is below asset's existential deposit.
		InsufficientShareBalance,

		/// Not allowed to perform an operation on given asset.
		NotAllowed,

		/// Future block number is in the past.
		PastBlock,

		/// New amplification is equal to the previous value.
		SameAmplification,

		/// Slippage protection.
		SlippageLimit,

		/// Failed to retrieve asset decimals.
		UnknownDecimals,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a stable pool with given list of assets.
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
		/// - `fee`: fee to be applied on trade and liquidity operations
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
			fee: Permill,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let amplification = NonZeroU16::new(amplification).ok_or(Error::<T>::InvalidAmplification)?;

			let pool_id = Self::do_create_pool(share_asset, &assets, amplification, fee)?;

			Self::deposit_event(Event::PoolCreated {
				pool_id,
				assets,
				amplification,
				fee,
			});

			Self::deposit_event(Event::AmplificationChanging {
				pool_id,
				current_amplification: amplification,
				final_amplification: amplification,
				start_block: T::BlockNumberProvider::current_block_number(),
				end_block: T::BlockNumberProvider::current_block_number(),
			});
			Ok(())
		}

		/// Update pool's fee.
		///
		/// if pool does not exist, `PoolNotFound` is returned.
		///
		/// Parameters:
		/// - `origin`: Must be T::AuthorityOrigin
		/// - `pool_id`: pool to update
		/// - `fee`: new pool fee
		///
		/// Emits `FeeUpdated` event if successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::update_pool_fee())]
		#[transactional]
		pub fn update_pool_fee(origin: OriginFor<T>, pool_id: T::AssetId, fee: Permill) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			Pools::<T>::try_mutate(pool_id, |maybe_pool| -> DispatchResult {
				let pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;

				pool.fee = fee;
				Self::deposit_event(Event::FeeUpdated { pool_id, fee });
				Ok(())
			})
		}

		/// Update pool's amplification.
		///
		/// Parameters:
		/// - `origin`: Must be T::AuthorityOrigin
		/// - `pool_id`: pool to update
		/// - `future_amplification`: new desired pool amplification
		/// - `future_block`: future block number when the amplification is updated
		///
		/// Emits `AmplificationUpdated` event if successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_amplification())]
		#[transactional]
		pub fn update_amplification(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			final_amplification: u16,
			start_block: BlockNumberFor<T>,
			end_block: BlockNumberFor<T>,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let current_block = T::BlockNumberProvider::current_block_number();
			ensure!(
				end_block > start_block && start_block >= current_block,
				Error::<T>::PastBlock
			);

			Pools::<T>::try_mutate(pool_id, |maybe_pool| -> DispatchResult {
				let pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;

				let current_amplification = Self::get_amplification(pool);

				ensure!(
					current_amplification != final_amplification as u128,
					Error::<T>::SameAmplification
				);

				pool.initial_amplification =
					NonZeroU16::new(current_amplification.saturated_into()).ok_or(Error::<T>::InvalidAmplification)?;
				pool.final_amplification =
					NonZeroU16::new(final_amplification).ok_or(Error::<T>::InvalidAmplification)?;
				pool.initial_block = start_block;
				pool.final_block = end_block;

				ensure!(
					T::AmplificationRange::get().contains(&pool.final_amplification),
					Error::<T>::InvalidAmplification
				);
				Self::deposit_event(Event::AmplificationChanging {
					pool_id,
					current_amplification: pool.initial_amplification,
					final_amplification: pool.final_amplification,
					start_block: pool.initial_block,
					end_block: pool.final_block,
				});
				Ok(())
			})
		}

		/// Add liquidity to selected pool.
		///
		/// First call of `add_liquidity` must provide "initial liquidity" of all assets.
		///
		/// If there is liquidity already in the pool, LP can provide liquidity of any number of pool assets.
		///
		/// LP must have sufficient amount of each asset.
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
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity()
							.saturating_add(T::Hooks::on_liquidity_changed_weight(MAX_ASSETS_IN_POOL as usize)))]
		#[transactional]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			assets: Vec<AssetAmount<T::AssetId>>,
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

		/// Add liquidity to selected pool given exact amount of shares to receive.
		///
		/// Similar to `add_liquidity` but LP specifies exact amount of shares to receive.
		///
		/// This functionality is used mainly by on-chain routing when a swap between Omnipool asset and stable asset is performed.
		///
		/// Parameters:
		/// - `origin`: liquidity provider
		/// - `pool_id`: Pool Id
		/// - `shares`: amount of shares to receive
		/// - `asset_id`: asset id of an asset to provide as liquidity
		/// - `max_asset_amount`: slippage limit. Max amount of asset.
		///
		/// Emits `LiquidityAdded` event when successful.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity_shares()
							.saturating_add(T::Hooks::on_liquidity_changed_weight(MAX_ASSETS_IN_POOL as usize)))]
		#[transactional]
		pub fn add_liquidity_shares(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			shares: Balance,
			asset_id: T::AssetId,
			max_asset_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let amount_in = Self::do_add_liquidity_shares(&who, pool_id, shares, asset_id, max_asset_amount)?;
			Self::deposit_event(Event::LiquidityAdded {
				pool_id,
				who,
				shares,
				assets: vec![AssetAmount::new(asset_id, amount_in)],
			});

			Ok(())
		}

		/// Remove liquidity from selected pool.
		///
		/// Withdraws liquidity of selected asset from a pool.
		///
		/// Share amount is burned and LP receives corresponding amount of chosen asset.
		///
		/// Withdraw fee is applied to the asset amount.
		///
		/// Parameters:
		/// - `origin`: liquidity provider
		/// - `pool_id`: Pool Id
		/// - `asset_id`: id of asset to receive
		/// - 'share_amount': amount of shares to withdraw
		/// - 'min_amount_out': minimum amount to receive
		///
		/// Emits `LiquidityRemoved` event when successful.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity_one_asset()
							.saturating_add(T::Hooks::on_liquidity_changed_weight(MAX_ASSETS_IN_POOL as usize)))]
		#[transactional]
		pub fn remove_liquidity_one_asset(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			share_amount: Balance,
			min_amount_out: Balance,
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

			// Retrive pool state.
			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let asset_idx = pool.find_asset(asset_id).ok_or(Error::<T>::AssetNotInPool)?;
			let pool_account = Self::pool_account(pool_id);
			let initial_reserves = pool
				.reserves_with_decimals::<T>(&pool_account)
				.ok_or(Error::<T>::UnknownDecimals)?;
			let share_issuance = T::Currency::total_issuance(pool_id);

			ensure!(
				share_issuance == share_amount
					|| share_issuance.saturating_sub(share_amount) >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidityRemaining
			);

			let amplification = Self::get_amplification(&pool);

			//Calculate how much asset user will receive. Note that the fee is already subtracted from the amount.
			let (amount, fee) = hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
				&initial_reserves,
				share_amount,
				asset_idx,
				share_issuance,
				amplification,
				pool.fee,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(amount >= min_amount_out, Error::<T>::SlippageLimit);

			// Burn shares and transfer asset to user.
			T::Currency::withdraw(pool_id, &who, share_amount)?;
			T::Currency::transfer(asset_id, &pool_account, &who, amount)?;

			// All done and updated. let's call the on_liquidity_changed hook.
			Self::call_on_liquidity_change_hook(pool_id, &initial_reserves, share_issuance)?;

			Self::deposit_event(Event::LiquidityRemoved {
				pool_id,
				who,
				shares: share_amount,
				amounts: vec![AssetAmount { asset_id, amount }],
				fee,
			});

			Ok(())
		}

		/// Remove liquidity from selected pool by specifying exact amount of asset to receive.
		///
		/// Similar to `remove_liquidity_one_asset` but LP specifies exact amount of asset to receive instead of share amount.
		///
		/// Parameters:
		/// - `origin`: liquidity provider
		/// - `pool_id`: Pool Id
		/// - `asset_id`: id of asset to receive
		/// - 'amount': amount of asset to receive
		/// - 'max_share_amount': Slippage limit. Max amount of shares to burn.
		///
		/// Emits `LiquidityRemoved` event when successful.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_asset_amount()
							.saturating_add(T::Hooks::on_liquidity_changed_weight(MAX_ASSETS_IN_POOL as usize)))]
		#[transactional]
		pub fn withdraw_asset_amount(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			amount: Balance,
			max_share_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				Self::is_asset_allowed(pool_id, asset_id, Tradability::REMOVE_LIQUIDITY),
				Error::<T>::NotAllowed
			);
			ensure!(amount > Balance::zero(), Error::<T>::InvalidAssetAmount);

			// Retrieve pool state.
			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let asset_idx = pool.find_asset(asset_id).ok_or(Error::<T>::AssetNotInPool)?;
			let pool_account = Self::pool_account(pool_id);
			let initial_reserves = pool
				.reserves_with_decimals::<T>(&pool_account)
				.ok_or(Error::<T>::UnknownDecimals)?;
			let share_issuance = T::Currency::total_issuance(pool_id);
			let amplification = Self::get_amplification(&pool);

			// Calculate how much shares user needs to provide to receive `amount` of asset.
			let shares = hydra_dx_math::stableswap::calculate_shares_for_amount::<D_ITERATIONS>(
				&initial_reserves,
				asset_idx,
				amount,
				amplification,
				share_issuance,
				pool.fee,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(shares <= max_share_amount, Error::<T>::SlippageLimit);

			let current_share_balance = T::Currency::free_balance(pool_id, &who);
			ensure!(
				current_share_balance == shares
					|| current_share_balance.saturating_sub(shares) >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientShareBalance
			);

			// Burn shares and transfer asset to user.
			T::Currency::withdraw(pool_id, &who, shares)?;
			T::Currency::transfer(asset_id, &pool_account, &who, amount)?;

			// All done and updated. let's call the on_liquidity_changed hook.
			Self::call_on_liquidity_change_hook(pool_id, &initial_reserves, share_issuance)?;

			Self::deposit_event(Event::LiquidityRemoved {
				pool_id,
				who,
				shares,
				amounts: vec![AssetAmount { asset_id, amount }],
				fee: 0u128, // dev note: figure out the actual fee amount in this case. For now, we dont need it.
			});

			Ok(())
		}

		/// Execute a swap of `asset_in` for `asset_out`.
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
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::sell()
							.saturating_add(T::Hooks::on_trade_weight(MAX_ASSETS_IN_POOL as usize)))]
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

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let pool_account = Self::pool_account(pool_id);
			let initial_reserves = pool
				.reserves_with_decimals::<T>(&pool_account)
				.ok_or(Error::<T>::UnknownDecimals)?;

			let (amount_out, fee_amount) = Self::calculate_out_amount(pool_id, asset_in, asset_out, amount_in)?;
			ensure!(amount_out >= min_buy_amount, Error::<T>::BuyLimitNotReached);

			T::Currency::transfer(asset_in, &who, &pool_account, amount_in)?;
			T::Currency::transfer(asset_out, &pool_account, &who, amount_out)?;

			//All done and updated. Let's call on_trade hook.
			Self::call_on_trade_hook(pool_id, asset_in, asset_out, &initial_reserves)?;

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

		/// Execute a swap of `asset_in` for `asset_out`.
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
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::buy()
							.saturating_add(T::Hooks::on_trade_weight(MAX_ASSETS_IN_POOL as usize)))]
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

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let pool_account = Self::pool_account(pool_id);
			let initial_reserves = pool
				.reserves_with_decimals::<T>(&pool_account)
				.ok_or(Error::<T>::UnknownDecimals)?;

			let (amount_in, fee_amount) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out)?;

			let pool_account = Self::pool_account(pool_id);

			ensure!(amount_in <= max_sell_amount, Error::<T>::SellLimitExceeded);

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= amount_in,
				Error::<T>::InsufficientBalance
			);

			T::Currency::transfer(asset_in, &who, &pool_account, amount_in)?;
			T::Currency::transfer(asset_out, &pool_account, &who, amount_out)?;

			//All done and updated. Let's call on_trade_hook.
			Self::call_on_trade_hook(pool_id, asset_in, asset_out, &initial_reserves)?;

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

		#[pallet::call_index(9)]
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_tradable_state())]
		#[transactional]
		pub fn set_asset_tradable_state(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			state: Tradability,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let _ = pool.find_asset(asset_id).ok_or(Error::<T>::AssetNotInPool)?;

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
	/// Calculates out amount given in amount.
	/// Returns (out_amount, fee_amount) on success. Note that fee amount is already subtracted from the out amount.
	fn calculate_out_amount(
		pool_id: T::AssetId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
	) -> Result<(Balance, Balance), DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		let index_in = pool.find_asset(asset_in).ok_or(Error::<T>::AssetNotInPool)?;
		let index_out = pool.find_asset(asset_out).ok_or(Error::<T>::AssetNotInPool)?;

		let pool_account = Self::pool_account(pool_id);
		let initial_reserves = pool
			.reserves_with_decimals::<T>(&pool_account)
			.ok_or(Error::<T>::UnknownDecimals)?;

		ensure!(!initial_reserves[index_in].is_zero(), Error::<T>::InsufficientLiquidity);
		ensure!(
			!initial_reserves[index_out].is_zero(),
			Error::<T>::InsufficientLiquidity
		);

		let amplification = Self::get_amplification(&pool);
		hydra_dx_math::stableswap::calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&initial_reserves,
			index_in,
			index_out,
			amount_in,
			amplification,
			pool.fee,
		)
		.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	/// Calculates in amount given out amount.
	/// Returns (in_amount, fee_amount) on success. Note that fee amount is already added to the in amount.
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
		let initial_reserves = pool
			.reserves_with_decimals::<T>(&pool_account)
			.ok_or(Error::<T>::UnknownDecimals)?;

		ensure!(
			initial_reserves[index_out].amount > amount_out,
			Error::<T>::InsufficientLiquidity
		);
		ensure!(!initial_reserves[index_in].is_zero(), Error::<T>::InsufficientLiquidity);

		let amplification = Self::get_amplification(&pool);
		hydra_dx_math::stableswap::calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&initial_reserves,
			index_in,
			index_out,
			amount_out,
			amplification,
			pool.fee,
		)
		.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	#[require_transactional]
	fn do_create_pool(
		share_asset: T::AssetId,
		assets: &[T::AssetId],
		amplification: NonZeroU16,
		fee: Permill,
	) -> Result<T::AssetId, DispatchError> {
		ensure!(!Pools::<T>::contains_key(share_asset), Error::<T>::PoolExists);
		ensure!(
			T::AssetInspection::exists(share_asset),
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
			initial_amplification: amplification,
			final_amplification: amplification,
			initial_block: block_number,
			final_block: block_number,
			fee,
		};
		ensure!(pool.is_valid(), Error::<T>::IncorrectAssets);
		ensure!(
			T::AmplificationRange::get().contains(&amplification),
			Error::<T>::InvalidAmplification
		);
		for asset in pool.assets.iter() {
			ensure!(T::AssetInspection::exists(*asset), Error::<T>::AssetNotRegistered);
		}

		Pools::<T>::insert(share_asset, pool);
		T::DustAccountHandler::add_account(&Self::pool_account(share_asset))?;
		Ok(share_asset)
	}

	#[require_transactional]
	fn do_add_liquidity(
		who: &T::AccountId,
		pool_id: T::AssetId,
		assets: &[AssetAmount<T::AssetId>],
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
			if added_assets.insert(asset.asset_id, asset.amount).is_some() {
				return Err(Error::<T>::IncorrectAssets.into());
			}
		}

		let pool_account = Self::pool_account(pool_id);
		let mut initial_reserves = Vec::with_capacity(pool.assets.len());
		let mut updated_reserves = Vec::with_capacity(pool.assets.len());
		let mut added_amounts = Vec::with_capacity(pool.assets.len());
		for pool_asset in pool.assets.iter() {
			let decimals = Self::retrieve_decimals(*pool_asset).ok_or(Error::<T>::UnknownDecimals)?;
			let reserve = T::Currency::free_balance(*pool_asset, &pool_account);
			initial_reserves.push(AssetReserve {
				amount: reserve,
				decimals,
			});
			if let Some(liq_added) = added_assets.remove(pool_asset) {
				let inc_reserve = reserve.checked_add(liq_added).ok_or(ArithmeticError::Overflow)?;
				updated_reserves.push(AssetReserve {
					amount: inc_reserve,
					decimals,
				});
				added_amounts.push(liq_added);
			} else {
				ensure!(!reserve.is_zero(), Error::<T>::InvalidInitialLiquidity);
				updated_reserves.push(AssetReserve {
					amount: reserve,
					decimals,
				});
				added_amounts.push(0);
			}
		}
		// If something is left in added_assets, it means that user provided liquidity for asset that is not in the pool.
		ensure!(added_assets.is_empty(), Error::<T>::AssetNotInPool);

		let amplification = Self::get_amplification(&pool);
		let share_issuance = T::Currency::total_issuance(pool_id);
		let share_amount = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
			&initial_reserves,
			&updated_reserves,
			amplification,
			share_issuance,
			pool.fee,
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

		// All done and updated. let's call the on_liquidity_changed hook.
		Self::call_on_liquidity_change_hook(pool_id, &initial_reserves, share_issuance)?;

		Ok(share_amount)
	}

	#[require_transactional]
	fn do_add_liquidity_shares(
		who: &T::AccountId,
		pool_id: T::AssetId,
		shares: Balance,
		asset_id: T::AssetId,
		max_asset_amount: Balance,
	) -> Result<Balance, DispatchError> {
		ensure!(
			Self::is_asset_allowed(pool_id, asset_id, Tradability::ADD_LIQUIDITY),
			Error::<T>::NotAllowed
		);
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
		let asset_idx = pool.find_asset(asset_id).ok_or(Error::<T>::AssetNotInPool)?;
		let share_issuance = T::Currency::total_issuance(pool_id);
		let amplification = Self::get_amplification(&pool);
		let pool_account = Self::pool_account(pool_id);
		let initial_reserves = pool
			.reserves_with_decimals::<T>(&pool_account)
			.ok_or(Error::<T>::UnknownDecimals)?;

		// Ensure that initial liquidity has been already provided
		for reserve in initial_reserves.iter() {
			ensure!(!reserve.amount.is_zero(), Error::<T>::InvalidInitialLiquidity);
		}

		let (amount_in, _) = hydra_dx_math::stableswap::calculate_add_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
			&initial_reserves,
			shares,
			asset_idx,
			share_issuance,
			amplification,
			pool.fee,
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(amount_in <= max_asset_amount, Error::<T>::SlippageLimit);

		ensure!(!amount_in.is_zero(), Error::<T>::InvalidAssetAmount);
		let current_share_balance = T::Currency::free_balance(pool_id, who);

		ensure!(
			current_share_balance.saturating_add(shares) >= T::MinPoolLiquidity::get(),
			Error::<T>::InsufficientShareBalance
		);

		T::Currency::deposit(pool_id, who, shares)?;
		T::Currency::transfer(asset_id, who, &pool_account, amount_in)?;

		//All done and update. let's call the on_liquidity_changed hook.
		Self::call_on_liquidity_change_hook(pool_id, &initial_reserves, share_issuance)?;

		Ok(amount_in)
	}

	#[inline]
	fn is_asset_allowed(pool_id: T::AssetId, asset_id: T::AssetId, operation: Tradability) -> bool {
		AssetTradability::<T>::get(pool_id, asset_id).contains(operation)
	}

	#[inline]
	fn pool_account(pool_id: T::AssetId) -> T::AccountId {
		T::ShareAccountId::from_assets(&pool_id, Some(POOL_IDENTIFIER))
	}

	#[inline]
	pub(crate) fn get_amplification(pool: &PoolInfo<T::AssetId, BlockNumberFor<T>>) -> u128 {
		hydra_dx_math::stableswap::calculate_amplification(
			pool.initial_amplification.get().into(),
			pool.final_amplification.get().into(),
			pool.initial_block.saturated_into(),
			pool.final_block.saturated_into(),
			T::BlockNumberProvider::current_block_number().saturated_into(),
		)
	}

	#[inline]
	pub(crate) fn retrieve_decimals(asset_id: T::AssetId) -> Option<u8> {
		T::AssetInspection::decimals(asset_id)
	}
}

impl<T: Config> Pallet<T> {
	fn calculate_shares(pool_id: T::AssetId, assets: &[AssetAmount<T::AssetId>]) -> Result<Balance, DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
		let pool_account = Self::pool_account(pool_id);

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

			ensure!(pool.find_asset(asset.asset_id).is_some(), Error::<T>::AssetNotInPool);

			if added_assets.insert(asset.asset_id, asset.amount).is_some() {
				return Err(Error::<T>::IncorrectAssets.into());
			}
		}

		let mut initial_reserves = Vec::with_capacity(pool.assets.len());
		let mut updated_reserves = Vec::with_capacity(pool.assets.len());
		for pool_asset in pool.assets.iter() {
			let decimals = Self::retrieve_decimals(*pool_asset).ok_or(Error::<T>::UnknownDecimals)?;
			let reserve = T::Currency::free_balance(*pool_asset, &pool_account);
			initial_reserves.push(AssetReserve {
				amount: reserve,
				decimals,
			});
			if let Some(liq_added) = added_assets.remove(pool_asset) {
				let inc_reserve = reserve.checked_add(liq_added).ok_or(ArithmeticError::Overflow)?;
				updated_reserves.push(AssetReserve {
					amount: inc_reserve,
					decimals,
				});
			} else {
				ensure!(!reserve.is_zero(), Error::<T>::InvalidInitialLiquidity);
				updated_reserves.push(AssetReserve {
					amount: reserve,
					decimals,
				});
			}
		}

		let amplification = Self::get_amplification(&pool);
		let share_issuance = T::Currency::total_issuance(pool_id);
		let share_amount = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
			&initial_reserves,
			&updated_reserves,
			amplification,
			share_issuance,
			pool.fee,
		)
		.ok_or(ArithmeticError::Overflow)?;

		Ok(share_amount)
	}

	// Trigger on_liquidity_changed hook. Initial reserves and issuance are required to calculate delta.
	// We need new updated reserves and new share price of each asset in pool, so for this, we can simply query the storage after the update.
	fn call_on_liquidity_change_hook(
		pool_id: T::AssetId,
		initial_reserves: &[AssetReserve],
		initial_issuance: Balance,
	) -> DispatchResult {
		let state = Self::get_pool_state(pool_id, initial_reserves, Some(initial_issuance))?;
		T::Hooks::on_liquidity_changed(pool_id, state)
	}

	// Trigger on_trade hook. Initial reserves are required to calculate delta.
	// We need new updated reserves and new share price of each asset in pool, so for this, we can simply query the storage after the update.
	fn call_on_trade_hook(
		pool_id: T::AssetId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		initial_reserves: &[AssetReserve],
	) -> DispatchResult {
		let state = Self::get_pool_state(pool_id, initial_reserves, None)?;
		T::Hooks::on_trade(pool_id, asset_in, asset_out, state)
	}

	// Get pool state info for on_liquidity_changed and on_trade hooks.
	fn get_pool_state(
		pool_id: T::AssetId,
		initial_reserves: &[AssetReserve],
		initial_issuance: Option<Balance>,
	) -> Result<PoolState<T::AssetId>, DispatchError> {
		let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
		let pool_account = Self::pool_account(pool_id);
		let amplification = Self::get_amplification(&pool);
		let share_issuance = T::Currency::total_issuance(pool_id);
		let updated_reserves = pool
			.reserves_with_decimals::<T>(&pool_account)
			.ok_or(Error::<T>::UnknownDecimals)?;
		let share_prices = hydra_dx_math::stableswap::calculate_share_prices::<D_ITERATIONS>(
			&updated_reserves,
			amplification,
			share_issuance,
		)
		.ok_or(ArithmeticError::Overflow)?;

		let deltas: Vec<Balance> = initial_reserves
			.iter()
			.zip(updated_reserves.iter())
			.map(|(initial, updated)| initial.amount.abs_diff(updated.amount))
			.collect();

		let state = PoolState {
			assets: pool.assets.into_inner(),
			before: initial_reserves.iter().map(|v| v.into()).collect(),
			after: updated_reserves.iter().map(|v| v.into()).collect(),
			delta: deltas,
			issuance_before: initial_issuance.unwrap_or(share_issuance),
			issuance_after: share_issuance,
			share_prices,
		};

		Ok(state)
	}
}
