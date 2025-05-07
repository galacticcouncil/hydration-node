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
//! ### Drifting peg
//! It is possible to create a pool with so called drifting peg.
//! Source of target peg for each asset must be provided. Either constant value or external oracle.
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
//! Maximum number of assets in pool is 5 (`MAX_ASSETS_IN_POOL` constant).
//!
//! A pool can be created only by allowed `AuthorityOrigin`.
//!
//! First LP to provide liquidity must add initial liquidity of all pool assets. Subsequent calls to add_liquidity, LP can provide only 1 asset.
//!
//! Initial liquidity is first liquidity added to the pool (that is first call of `add_assets_liquidity`).
//!
//! LP is given certain amount of shares by minting a pool's share token.
//!
//! When LP decides to withdraw liquidity, it receives selected asset or all assets proportionality.
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]

extern crate core;

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::{ensure, require_transactional, transactional, PalletId};
use frame_system::ensure_signed;
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
use hydradx_traits::{registry::Inspect, stableswap::StableswapAddLiquidity, AccountIdFor};
pub use pallet::*;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider, Zero};
use sp_runtime::{ArithmeticError, DispatchError, Permill, SaturatedConversion};
use sp_std::num::NonZeroU16;
use sp_std::prelude::*;
use sp_std::vec;
use types::OracleSource;

mod trade_execution;
pub mod types;
pub mod weights;

use crate::types::{
	Balance, BoundedPegs, PegSource, PegType, PoolInfo, PoolPegInfo, PoolState, RawOracle, StableswapHooks, Tradability,
};
use hydra_dx_math::stableswap::types::AssetReserve;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::MultiCurrency;
use pallet_broadcast::types::{Asset, Destination, Fee};
use sp_std::collections::btree_map::BTreeMap;
pub use weights::WeightInfo;

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
	use crate::types::{BoundedPegSources, PoolPegInfo};
	use codec::HasCompact;
	use core::ops::RangeInclusive;
	use frame_support::pallet_prelude::*;
	use hydradx_traits::pools::DustRemovalAccountWhitelist;
	use pallet_broadcast::types::Fee;
	use sp_runtime::traits::{BlockNumberProvider, Zero};
	use sp_runtime::ArithmeticError;
	use sp_runtime::Permill;
	use sp_std::num::NonZeroU16;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_broadcast::Config {
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
			+ TypeInfo
			+ Into<u32>;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Account ID constructor - pool account are derived from unique pool id
		type ShareAccountId: AccountIdFor<Self::AssetId, AccountId = Self::AccountId>;

		/// Asset registry mechanism to check if asset is registered and retrieve asset decimals.
		type AssetInspection: Inspect<AssetId = Self::AssetId>;

		/// The origin which can create a new pool
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Security origin which can set the asset tradable state
		type UpdateTradabilityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

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

		/// Oracle providing prices for asset pegs (if configured for pool)
		/// Raw oracle is required because it needs the values that are not delayed.
		/// It is how the mechanism is designed.
		type TargetPegOracle: RawOracle<Self::AssetId, Balance, BlockNumberFor<Self>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self::AssetId>;
	}

	/// Existing pools
	#[pallet::storage]
	#[pallet::getter(fn pools)]
	pub type Pools<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, PoolInfo<T::AssetId, BlockNumberFor<T>>>;

	/// Pool peg info.
	#[pallet::storage]
	#[pallet::getter(fn pool_peg_info)]
	pub type PoolPegs<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, PoolPegInfo<T::AssetId>>;

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
			peg: Option<PoolPegInfo<T::AssetId>>,
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
		/// Deprecated. Replaced by pallet_broadcast::Swapped
		// TODO: remove once we migrated completely to pallet_amm::Event::Swapped
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
		/// Deprecated. Replaced by pallet_broadcast::Swapped
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

		/// Amplification of a pool has been scheduled to change.
		AmplificationChanging {
			pool_id: T::AssetId,
			current_amplification: NonZeroU16,
			final_amplification: NonZeroU16,
			start_block: BlockNumberFor<T>,
			end_block: BlockNumberFor<T>,
		},
		/// A pool has been destroyed.
		PoolDestroyed { pool_id: T::AssetId },
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

		/// List of provided pegs is incorrect.
		IncorrectInitialPegs,

		/// Failed to retrieve oracle entry.
		MissingTargetPegOracle,

		/// Creating pool with pegs is not allowed for asset with different decimals.
		IncorrectAssetDecimals,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a stable pool with given list of assets.
		///
		/// All assets must be correctly registered in `T::AssetRegistry`.
		/// Note that this does not seed the pool with liquidity. Use `add_assets_liquidity` to provide
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
			assets: BoundedVec<T::AssetId, ConstU32<MAX_ASSETS_IN_POOL>>,
			amplification: u16,
			fee: Permill,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let amplification = NonZeroU16::new(amplification).ok_or(Error::<T>::InvalidAmplification)?;

			let pool_id = Self::do_create_pool(share_asset, &assets, amplification, fee, None)?;

			Self::deposit_event(Event::PoolCreated {
				pool_id,
				assets: assets.to_vec(),
				amplification,
				fee,
				peg: None,
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
		/// - `final_amplification`: new desired pool amplification
		/// - `start_block`: block number when the amplification starts to move towards final_amplication
		/// - `end_block`: block number when the amplification reaches final_amplification
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
		/// Use `add_assets_liquidity` instead.
		/// This extrinsics will be removed in the future.
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
		/// Emits `pallet_broadcast::Swapped` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity()
							.saturating_add(T::Hooks::on_liquidity_changed_weight(MAX_ASSETS_IN_POOL as usize)))]
		#[transactional]
		#[deprecated(note = "Use add_assets_liquidity instead")]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			assets: BoundedVec<AssetAmount<T::AssetId>, ConstU32<MAX_ASSETS_IN_POOL>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::do_add_liquidity(&who, pool_id, &assets, Balance::zero())?;

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
		/// Emits `pallet_broadcast::Swapped` event when successful.
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
		/// Emits `pallet_broadcast::Swapped` event when successful.
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
			let (trade_fee, asset_pegs) = Self::update_and_return_pegs_and_trade_fee(pool_id, &pool)?;

			//Calculate how much asset user will receive. Note that the fee is already subtracted from the amount.
			let (amount, fee) = hydra_dx_math::stableswap::calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
				&initial_reserves,
				share_amount,
				asset_idx,
				share_issuance,
				amplification,
				trade_fee,
				&asset_pegs,
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
				who: who.clone(),
				shares: share_amount,
				amounts: vec![AssetAmount { asset_id, amount }],
				fee,
			});

			pallet_broadcast::Pallet::<T>::deposit_trade_event(
				who,
				pool_account.clone(),
				pallet_broadcast::types::Filler::Stableswap(pool_id.into()),
				pallet_broadcast::types::TradeOperation::LiquidityRemove,
				vec![Asset::new(pool_id.into(), share_amount)],
				vec![Asset::new(asset_id.into(), amount)],
				vec![Fee {
					asset: pool_id.into(),
					amount: fee,
					destination: Destination::Account(pool_account),
				}],
			);

			#[cfg(any(feature = "try-runtime", test))]
			Self::ensure_remove_liquidity_invariant(pool_id, &initial_reserves);

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
		/// Emits `pallet_broadcast::Swapped` event when successful.
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
			let (trade_fee, asset_pegs) = Self::update_and_return_pegs_and_trade_fee(pool_id, &pool)?;

			// Calculate how much shares user needs to provide to receive `amount` of asset.
			let (shares, fees) = hydra_dx_math::stableswap::calculate_shares_for_amount::<D_ITERATIONS>(
				&initial_reserves,
				asset_idx,
				amount,
				amplification,
				share_issuance,
				trade_fee,
				&asset_pegs,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(shares <= max_share_amount, Error::<T>::SlippageLimit);

			// Burn shares and transfer asset to user.
			T::Currency::withdraw(pool_id, &who, shares)?;
			T::Currency::transfer(asset_id, &pool_account, &who, amount)?;

			// All done and updated. let's call the on_liquidity_changed hook.
			Self::call_on_liquidity_change_hook(pool_id, &initial_reserves, share_issuance)?;

			Self::deposit_event(Event::LiquidityRemoved {
				pool_id,
				who: who.clone(),
				shares,
				amounts: vec![AssetAmount { asset_id, amount }],
				fee: 0u128,
			});

			let fees = fees
				.iter()
				.zip(pool.assets.iter())
				.map(|(balance, asset_id)| {
					Fee::new((*asset_id).into(), *balance, Destination::Account(pool_account.clone()))
				})
				.collect::<Vec<_>>();
			pallet_broadcast::Pallet::<T>::deposit_trade_event(
				who,
				pool_account.clone(),
				pallet_broadcast::types::Filler::Stableswap(pool_id.into()),
				pallet_broadcast::types::TradeOperation::LiquidityRemove,
				vec![Asset::new(pool_id.into(), shares)],
				vec![Asset::new(asset_id.into(), amount)],
				fees,
			);

			#[cfg(any(feature = "try-runtime", test))]
			Self::ensure_remove_liquidity_invariant(pool_id, &initial_reserves);

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
		/// Emits `SellExecuted` event when successful. Deprecated.
		/// Emits `pallet_broadcast::Swapped` event when successful.
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

			ensure!(asset_in != asset_out, Error::<T>::NotAllowed);

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

			let (amount_out, fee_amount) = Self::calculate_out_amount(pool_id, asset_in, asset_out, amount_in, true)?;
			ensure!(amount_out >= min_buy_amount, Error::<T>::BuyLimitNotReached);

			T::Currency::transfer(asset_in, &who, &pool_account, amount_in)?;
			T::Currency::transfer(asset_out, &pool_account, &who, amount_out)?;

			//All done and updated. Let's call on_trade hook.
			Self::call_on_trade_hook(pool_id, asset_in, asset_out, &initial_reserves)?;

			Self::deposit_event(Event::SellExecuted {
				who: who.clone(),
				pool_id,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				fee: fee_amount,
			});

			pallet_broadcast::Pallet::<T>::deposit_trade_event(
				who,
				pool_account.clone(),
				pallet_broadcast::types::Filler::Stableswap(pool_id.into()),
				pallet_broadcast::types::TradeOperation::ExactIn,
				vec![Asset::new(asset_in.into(), amount_in)],
				vec![Asset::new(asset_out.into(), amount_out)],
				vec![Fee {
					asset: asset_out.into(),
					amount: fee_amount,
					destination: Destination::Account(pool_account),
				}],
			);

			#[cfg(any(feature = "try-runtime", test))]
			Self::ensure_trade_invariant(pool_id, &initial_reserves, pool.fee);

			Ok(())
		}

		/// Execute a swap of `asset_out` for `asset_in`.
		///
		/// Parameters:
		/// - `origin`:
		/// - `pool_id`: Id of a pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `asset_in`: ID of asset sold to the pool
		/// - `amount_out`: Amount of asset to receive from the pool
		/// - `max_sell_amount`: Maximum amount allowed to be sold
		///
		/// Emits `BuyExecuted` event when successful. Deprecated.
		/// Emits `pallet_broadcast::Swapped` event when successful.
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

			ensure!(asset_in != asset_out, Error::<T>::NotAllowed);

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

			let (amount_in, fee_amount) = Self::calculate_in_amount(pool_id, asset_in, asset_out, amount_out, true)?;

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
				who: who.clone(),
				pool_id,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				fee: fee_amount,
			});

			pallet_broadcast::Pallet::<T>::deposit_trade_event(
				who,
				pool_account.clone(),
				pallet_broadcast::types::Filler::Stableswap(pool_id.into()),
				pallet_broadcast::types::TradeOperation::ExactOut,
				vec![Asset::new(asset_in.into(), amount_in)],
				vec![Asset::new(asset_out.into(), amount_out)],
				vec![Fee {
					asset: asset_in.into(),
					amount: fee_amount,
					destination: Destination::Account(pool_account),
				}],
			);

			#[cfg(any(feature = "try-runtime", test))]
			Self::ensure_trade_invariant(pool_id, &initial_reserves, pool.fee);

			Ok(())
		}

		/// Update the tradable state of a specific asset in a pool.
		///
		/// This function allows updating the tradability state of an asset within a pool. The tradability state determines whether the asset can be used for specific operations such as adding liquidity, removing liquidity, buying, or selling.
		///
		/// Parameters:
		/// - `origin`: Must be `T::UpdateTradabilityOrigin`.
		/// - `pool_id`: The ID of the pool containing the asset.
		/// - `asset_id`: The ID of the asset whose tradability state is to be updated.
		/// - `state`: The new tradability state of the asset.
		///
		/// Emits `TradableStateUpdated` event when successful.
		///
		/// # Errors
		/// - `PoolNotFound`: If the specified pool does not exist.
		/// - `AssetNotInPool`: If the specified asset is not part of the pool.
		///
		#[pallet::call_index(9)]
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_tradable_state())]
		#[transactional]
		pub fn set_asset_tradable_state(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			asset_id: T::AssetId,
			state: Tradability,
		) -> DispatchResult {
			T::UpdateTradabilityOrigin::ensure_origin(origin)?;

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

		/// Remove liquidity from a selected pool uniformly.
		///
		/// This function allows a liquidity provider to withdraw liquidity from a pool.
		/// The provider specifies the amount of shares to burn and the minimum amounts of each asset to receive.
		///
		/// Parameters:
		/// - `origin`: The liquidity provider.
		/// - `pool_id`: The ID of the pool from which to remove liquidity.
		/// - `share_amount`: The amount of shares to burn.
		/// - `min_amounts_out`: A bounded vector specifying the minimum amounts of each asset to receive.
		///
		/// Emits `LiquidityRemoved` event when successful.
		/// Emits `pallet_broadcast::Swapped` event when successful.
		///
		/// # Errors
		/// - `InvalidAssetAmount`: If the `share_amount` is zero.
		/// - `InsufficientShares`: If the provider does not have enough shares.
		/// - `PoolNotFound`: If the specified pool does not exist.
		/// - `UnknownDecimals`: If the asset decimals cannot be retrieved.
		/// - `IncorrectAssets`: If the provided `min_amounts_out` does not match the pool assets.
		/// - `NotAllowed`: If the asset is not allowed for the operation.
		/// - `SlippageLimit`: If the amount received is less than the specified minimum amount.
		/// - `InsufficientLiquidityRemaining`: If the remaining liquidity in the pool is below the minimum required.
		///
		/// # Invariants
		/// - Ensures that the pool's reserves are updated correctly after liquidity removal.
		/// - Ensures that the pool's invariant is maintained.
		#[pallet::call_index(10)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity())]
		#[transactional]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			share_amount: Balance,
			min_amounts_out: BoundedVec<AssetAmount<T::AssetId>, ConstU32<MAX_ASSETS_IN_POOL>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(share_amount > Balance::zero(), Error::<T>::InvalidAssetAmount);

			let current_share_balance = T::Currency::free_balance(pool_id, &who);
			ensure!(current_share_balance >= share_amount, Error::<T>::InsufficientShares);

			let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
			let pool_account = Self::pool_account(pool_id);
			let initial_reserves = pool
				.reserves_with_decimals::<T>(&pool_account)
				.ok_or(Error::<T>::UnknownDecimals)?;
			let share_issuance = T::Currency::total_issuance(pool_id);

			// We want to ensure that given min amounts are correct. It must contain all pool assets.
			// We convert vec of min amounts to a map.
			// We first ensure the length , and if any asset is not found later on, we can return an error.
			ensure!(min_amounts_out.len() == pool.assets.len(), Error::<T>::IncorrectAssets);
			let mut min_amounts_out_map = BTreeMap::new();
			for v in min_amounts_out.into_iter() {
				let r = min_amounts_out_map.insert(v.asset_id, v.amount);
				ensure!(r.is_none(), Error::<T>::IncorrectAssets);
			}

			// Store the amount of each asset that is transferred. Used as info in the event.
			let mut amounts = Vec::with_capacity(pool.assets.len());

			// 1. Calculate amount of each asset
			// 2. ensure min amount is respected
			// 3. transfer amount to user
			for asset_id in pool.assets.iter() {
				ensure!(
					Self::is_asset_allowed(pool_id, *asset_id, Tradability::REMOVE_LIQUIDITY),
					Error::<T>::NotAllowed
				);
				let min_amount = min_amounts_out_map
					.remove(asset_id)
					.ok_or(Error::<T>::IncorrectAssets)?;
				let reserve = T::Currency::free_balance(*asset_id, &pool_account);

				// Special case when withdrawing all remaining pool shares, so we can directly send all the remaining assets to the user.
				let amount = if share_amount == share_issuance {
					ensure!(reserve >= min_amount, Error::<T>::SlippageLimit);
					reserve
				} else {
					let amount =
						hydra_dx_math::stableswap::calculate_liquidity_out(reserve, share_amount, share_issuance)
							.ok_or(ArithmeticError::Overflow)?;
					ensure!(amount >= min_amount, Error::<T>::SlippageLimit);
					amount
				};

				T::Currency::transfer(*asset_id, &pool_account, &who, amount)?;
				amounts.push(AssetAmount {
					asset_id: *asset_id,
					amount,
				});
			}

			// Burn shares
			T::Currency::withdraw(pool_id, &who, share_amount)?;

			// All done and updated. let's call the on_liquidity_changed hook.
			if share_amount != share_issuance {
				Self::call_on_liquidity_change_hook(pool_id, &initial_reserves, share_issuance)?;
			} else {
				// Remove the pool.
				Pools::<T>::remove(pool_id);
				PoolPegs::<T>::remove(pool_id);
				let _ = AssetTradability::<T>::clear_prefix(pool_id, MAX_ASSETS_IN_POOL, None);
				T::DustAccountHandler::remove_account(&Self::pool_account(pool_id))?;
				Self::deposit_event(Event::PoolDestroyed { pool_id });
			}

			Self::deposit_event(Event::LiquidityRemoved {
				pool_id,
				who,
				shares: share_amount,
				amounts,
				fee: Balance::zero(),
			});

			#[cfg(any(feature = "try-runtime", test))]
			Self::ensure_remove_liquidity_invariant(pool_id, &initial_reserves);

			Ok(())
		}

		/// Create a stable pool with a given list of assets and pegs.
		///
		/// This function allows the creation of a new stable pool with specified assets, amplification, fee, and peg sources. The pool is identified by a share asset.
		///
		/// Peg target price is determined by retrieving the target peg from the oracle - it is the price of the asset from the peg sourcedenominated in the other pool assets.
		///
		/// Parameters:
		/// - `origin`: Must be `T::AuthorityOrigin`.
		/// - `share_asset`: Preregistered share asset identifier.
		/// - `assets`: List of asset IDs to be included in the pool.
		/// - `amplification`: Pool amplification parameter.
		/// - `fee`: Fee to be applied on trade and liquidity operations.
		/// - `peg_source`: Bounded vector specifying the source of the peg for each asset.
		/// - `max_peg_update`: Maximum allowed peg update per block.
		///
		/// Emits `PoolCreated` event if successful.
		/// Emits `AmplificationChanging` event if successful.
		///
		/// # Errors
		/// - `IncorrectAssets`: If the assets are the same or less than 2 assets are provided.
		/// - `MaxAssetsExceeded`: If the maximum number of assets is exceeded.
		/// - `PoolExists`: If a pool with the given assets already exists.
		/// - `ShareAssetInPoolAssets`: If the share asset is among the pool assets.
		/// - `AssetNotRegistered`: If one or more assets are not registered in the AssetRegistry.
		/// - `InvalidAmplification`: If the amplification parameter is invalid.
		/// - `IncorrectInitialPegs`: If the initial pegs are incorrect.
		/// - `MissingTargetPegOracle`: If the target peg oracle entry is missing.
		/// - `IncorrectAssetDecimals`: If the assets have different decimals.
		///
		#[pallet::call_index(11)]
		#[pallet::weight(<T as Config>::WeightInfo::create_pool_with_pegs())]
		#[transactional]
		pub fn create_pool_with_pegs(
			origin: OriginFor<T>,
			share_asset: T::AssetId,
			assets: BoundedVec<T::AssetId, ConstU32<MAX_ASSETS_IN_POOL>>,
			amplification: u16,
			fee: Permill,
			peg_source: BoundedPegSources<T::AssetId>,
			max_peg_update: Permill,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let amplification = NonZeroU16::new(amplification).ok_or(Error::<T>::InvalidAmplification)?;

			let current_block: u128 = T::BlockNumberProvider::current_block_number().saturated_into();
			let initial_pegs = Self::get_target_pegs(current_block, &assets, &peg_source)?;

			let peg_info = PoolPegInfo {
				source: peg_source,
				max_peg_update,
				current: BoundedPegs::truncate_from(initial_pegs.into_iter().map(|(v, _)| v).collect()),
			};

			let pool_id = Self::do_create_pool(share_asset, &assets, amplification, fee, Some(&peg_info))?;

			Self::deposit_event(Event::PoolCreated {
				pool_id,
				assets: assets.to_vec(),
				amplification,
				fee,
				peg: Some(peg_info),
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

		/// Add liquidity to selected pool.
		///
		/// First call of `add_assets_liquidity` must provide "initial liquidity" of all assets.
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
		/// - `min_shares`: minimum amount of shares to receive
		///
		/// Emits `LiquidityAdded` event when successful.
		/// Emits `pallet_broadcast::Swapped` event when successful.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as Config>::WeightInfo::add_assets_liquidity()
							.saturating_add(T::Hooks::on_liquidity_changed_weight(MAX_ASSETS_IN_POOL as usize)))]
		#[transactional]
		pub fn add_assets_liquidity(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			assets: BoundedVec<AssetAmount<T::AssetId>, ConstU32<MAX_ASSETS_IN_POOL>>,
			min_shares: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::do_add_liquidity(&who, pool_id, &assets, min_shares)?;

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	/// Account address to be used to dry-run sell for determining spot price of stable assets
	pub fn pallet_account() -> T::AccountId {
		PalletId(*b"stblpool").into_account_truncating()
	}

	/// Calculates out amount given in amount.
	/// Returns (out_amount, fee_amount) on success. Note that fee amount is already subtracted from the out amount.
	fn calculate_out_amount(
		pool_id: T::AssetId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
		persist_peg: bool,
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
		let (trade_fee, asset_pegs) = if persist_peg {
			Self::update_and_return_pegs_and_trade_fee(pool_id, &pool)?
		} else {
			// Only recalculate, do not store
			Self::get_updated_pegs(pool_id, &pool)?
		};
		hydra_dx_math::stableswap::calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&initial_reserves,
			index_in,
			index_out,
			amount_in,
			amplification,
			trade_fee,
			&asset_pegs,
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
		persist_peg: bool,
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
		let (trade_fee, asset_pegs) = if persist_peg {
			Self::update_and_return_pegs_and_trade_fee(pool_id, &pool)?
		} else {
			Self::get_updated_pegs(pool_id, &pool)?
		};
		hydra_dx_math::stableswap::calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
			&initial_reserves,
			index_in,
			index_out,
			amount_out,
			amplification,
			trade_fee,
			&asset_pegs,
		)
		.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	#[require_transactional]
	fn do_create_pool(
		share_asset: T::AssetId,
		assets: &[T::AssetId],
		amplification: NonZeroU16,
		fee: Permill,
		peg_info: Option<&PoolPegInfo<T::AssetId>>,
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

		if let Some(p) = peg_info {
			ensure!(p.current.len() == pool.assets.len(), Error::<T>::IncorrectInitialPegs);

			let asset_decimals: Vec<Option<u8>> = pool
				.assets
				.iter()
				.map(|asset| Self::retrieve_decimals(*asset))
				.collect();
			let asset_decimals: Option<Vec<u8>> = asset_decimals.into_iter().collect();
			if let Some(decimals_info) = asset_decimals {
				ensure!(
					decimals_info.iter().all(|&x| x == decimals_info[0]),
					Error::<T>::IncorrectAssetDecimals
				);
			} else {
				return Err(Error::<T>::UnknownDecimals.into());
			}
			PoolPegs::<T>::insert(share_asset, p);
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
		min_shares: Balance,
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
		let (trade_fee, asset_pegs) = Self::update_and_return_pegs_and_trade_fee(pool_id, &pool)?;
		let (share_amount, fees) = hydra_dx_math::stableswap::calculate_shares::<D_ITERATIONS>(
			&initial_reserves,
			&updated_reserves,
			amplification,
			share_issuance,
			trade_fee,
			&asset_pegs,
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(!share_amount.is_zero(), Error::<T>::InvalidAssetAmount);
		ensure!(share_amount >= min_shares, Error::<T>::SlippageLimit);

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

		#[cfg(any(feature = "try-runtime", test))]
		Self::ensure_add_liquidity_invariant(pool_id, &initial_reserves);

		Self::deposit_event(Event::LiquidityAdded {
			pool_id,
			who: who.clone(),
			shares: share_amount,
			assets: assets.to_vec(),
		});

		let inputs = assets
			.iter()
			.map(|asset| Asset::new(asset.asset_id.into(), asset.amount))
			.collect();
		let fees = fees
			.iter()
			.zip(pool.assets.iter())
			.map(|(balance, asset_id)| {
				Fee::new((*asset_id).into(), *balance, Destination::Account(pool_account.clone()))
			})
			.collect::<Vec<_>>();
		pallet_broadcast::Pallet::<T>::deposit_trade_event(
			who.clone(),
			pool_account.clone(),
			pallet_broadcast::types::Filler::Stableswap(pool_id.into()),
			pallet_broadcast::types::TradeOperation::LiquidityAdd,
			inputs,
			vec![Asset::new(pool_id.into(), share_amount)],
			fees,
		);

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

		let (trade_fee, asset_pegs) = Self::update_and_return_pegs_and_trade_fee(pool_id, &pool)?;
		let (amount_in, fee) = hydra_dx_math::stableswap::calculate_add_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
			&initial_reserves,
			shares,
			asset_idx,
			share_issuance,
			amplification,
			trade_fee,
			&asset_pegs,
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

		#[cfg(any(feature = "try-runtime", test))]
		Self::ensure_add_liquidity_invariant(pool_id, &initial_reserves);

		pallet_broadcast::Pallet::<T>::deposit_trade_event(
			who.clone(),
			pool_account.clone(),
			pallet_broadcast::types::Filler::Stableswap(pool_id.into()),
			pallet_broadcast::types::TradeOperation::LiquidityAdd,
			vec![Asset::new(asset_id.into(), amount_in)],
			vec![Asset::new(pool_id.into(), shares)],
			vec![Fee {
				asset: pool_id.into(),
				amount: fee,
				destination: Destination::Account(pool_account),
			}],
		);

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
		let (_, asset_pegs) = Self::get_updated_pegs(pool_id, &pool)?;
		let share_prices = hydra_dx_math::stableswap::calculate_share_prices::<D_ITERATIONS>(
			&updated_reserves,
			amplification,
			share_issuance,
			&asset_pegs,
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

	#[cfg(any(feature = "try-runtime", test))]
	fn ensure_add_liquidity_invariant(pool_id: T::AssetId, initial_reserves: &[AssetReserve]) {
		let pool = Pools::<T>::get(pool_id).unwrap();
		let (_, asset_pegs) = Self::get_updated_pegs(pool_id, &pool).unwrap();
		let final_reserves = pool.reserves_with_decimals::<T>(&Self::pool_account(pool_id)).unwrap();
		debug_assert_ne!(
			initial_reserves.iter().map(|v| v.amount).collect::<Vec<u128>>(),
			final_reserves.iter().map(|v| v.amount).collect::<Vec<u128>>(),
			"Reserves are not changed"
		);
		let amplification = Self::get_amplification(&pool);
		let initial_d =
			hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(initial_reserves, amplification, &asset_pegs)
				.unwrap();
		let final_d =
			hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(&final_reserves, amplification, &asset_pegs)
				.unwrap();
		assert!(
			final_d >= initial_d,
			"Add liquidity Invariant broken: D+ is less than initial D; {:?} <= {:?}",
			initial_d,
			final_d
		);
	}

	#[cfg(any(feature = "try-runtime", test))]
	fn ensure_remove_liquidity_invariant(pool_id: T::AssetId, initial_reserves: &[AssetReserve]) {
		let Some(pool) = Pools::<T>::get(pool_id) else {
			return;
		};
		let (_, asset_pegs) = Self::get_updated_pegs(pool_id, &pool).unwrap();
		let final_reserves = pool.reserves_with_decimals::<T>(&Self::pool_account(pool_id)).unwrap();
		debug_assert_ne!(
			initial_reserves.iter().map(|v| v.amount).collect::<Vec<u128>>(),
			final_reserves.iter().map(|v| v.amount).collect::<Vec<u128>>(),
			"Reserves are not changed"
		);
		let amplification = Self::get_amplification(&pool);
		let initial_d =
			hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(initial_reserves, amplification, &asset_pegs)
				.unwrap();
		let final_d =
			hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(&final_reserves, amplification, &asset_pegs)
				.unwrap();
		assert!(
			final_d <= initial_d,
			"Remove liquidity Invariant broken: D+ is more than initial D; {:?} >= {:?}",
			initial_d,
			final_d
		);
	}
	#[cfg(any(feature = "try-runtime", test))]
	fn ensure_trade_invariant(pool_id: T::AssetId, initial_reserves: &[AssetReserve], _fee: Permill) {
		let pool = Pools::<T>::get(pool_id).unwrap();
		let (_, asset_pegs) = Self::get_updated_pegs(pool_id, &pool).unwrap();
		let final_reserves = pool.reserves_with_decimals::<T>(&Self::pool_account(pool_id)).unwrap();
		debug_assert_ne!(
			initial_reserves.iter().map(|v| v.amount).collect::<Vec<u128>>(),
			final_reserves.iter().map(|v| v.amount).collect::<Vec<u128>>(),
			"Reserves are not changed"
		);
		let amplification = Self::get_amplification(&pool);
		let initial_d =
			hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(initial_reserves, amplification, &asset_pegs)
				.unwrap();
		let final_d =
			hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(&final_reserves, amplification, &asset_pegs)
				.unwrap();
		assert!(
			final_d >= initial_d,
			"Trade Invariant broken: D+ is less than initial D; {:?} <= {:?}",
			initial_d,
			final_d
		);
	}
}

impl<T: Config> StableswapAddLiquidity<T::AccountId, T::AssetId, Balance> for Pallet<T> {
	fn add_liquidity(
		who: T::AccountId,
		pool_id: T::AssetId,
		assets: Vec<AssetAmount<T::AssetId>>,
	) -> Result<Balance, DispatchError> {
		Self::do_add_liquidity(&who, pool_id, &assets, Balance::zero())
	}
}

// Peg support
impl<T: Config> Pallet<T> {
	// Recalculate pegs and trade fee - moving current pegs to target pegs
	fn get_updated_pegs(
		pool_id: T::AssetId,
		pool: &PoolInfo<T::AssetId, BlockNumberFor<T>>,
	) -> Result<(Permill, Vec<PegType>), DispatchError> {
		let Some(peg_info) = PoolPegs::<T>::get(pool_id) else {
			// No pegs for this pool, return default pegs
			return Ok((pool.fee, vec![(1, 1); pool.assets.len()]));
		};
		// Move pegs to target pegs if necessary
		let current_block: u128 = T::BlockNumberProvider::current_block_number().saturated_into();
		let target_pegs = Self::get_target_pegs(current_block, &pool.assets, &peg_info.source)?;

		hydra_dx_math::stableswap::recalculate_pegs(
			&peg_info.current,
			&target_pegs,
			current_block,
			peg_info.max_peg_update,
			pool.fee,
		)
		.ok_or(ArithmeticError::Overflow.into())
	}

	// Same as get_current_pegs but it stores new pegs as well
	#[require_transactional]
	fn update_and_return_pegs_and_trade_fee(
		pool_id: T::AssetId,
		pool: &PoolInfo<T::AssetId, BlockNumberFor<T>>,
	) -> Result<(Permill, Vec<PegType>), DispatchError> {
		let (trade_fee, new_pegs) = Self::get_updated_pegs(pool_id, pool)?;

		// Store new pegs if pool has pegs configured
		if let Some(peg_info) = PoolPegs::<T>::get(pool_id) {
			let new_info = peg_info.with_new_pegs(&new_pegs);
			PoolPegs::<T>::insert(pool_id, new_info);
		};

		Ok((trade_fee, new_pegs))
	}

	/// Retrieve new target pegs
	fn get_target_pegs(
		_block_no: u128, //TODO: remove & refactor
		pool_assets: &[T::AssetId],
		peg_sources: &[PegSource<T::AssetId>],
	) -> Result<Vec<(PegType, u128)>, DispatchError> {
		debug_assert_eq!(
			pool_assets.len(),
			peg_sources.len(),
			"Pool assets and peg sources must have the same length"
		);

		if pool_assets.is_empty() {
			// Should never happen
			debug_assert!(false, "Missing pool info");
			return Err(Error::<T>::IncorrectAssets.into());
		}

		let mut r = vec![];
		for (asset_id, source) in pool_assets.iter().zip(peg_sources.iter()) {
			let p = T::TargetPegOracle::get_raw_entry(OracleSource::from((source.clone(), *asset_id)))
				.map_err(|_| Error::<T>::MissingTargetPegOracle)?;

			r.push((p.peg, p.updated_at.saturated_into()));
		}
		Ok(r)
	}
}
