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

//! # Omnipool pallet
//!
//! Omnipool implementation
//!
//! ## Overview
//!
//! Omnipool is type of AMM where all assets are pooled together into one single pool.
//!
//! Liquidity provider can provide any asset of their choice to the Omnipool and in return
//! they will receive pool shares for this single asset.
//!
//! The position is represented with a NFT token which saves the amount of shares distributed
//! and the price of the asset at the time of provision.
//!
//! For traders this means that they can benefit from the fill asset position
//! which can be used for trades with all other assets - there is no fragmented liquidity.
//! They can send any token to the pool using the swap mechanism
//! and in return they will receive the token of their choice in the appropriate quantity.
//!
//! Omnipool is implemented with concrete Balance type: u128.
//!
//! ### Terminology
//!
//! * **LP:**  liquidity provider
//! * **Position:**  a moment when LP added liquidity to the pool. It stores amount,shares and price at the time
//!  of provision
//! * **Hub Asset:** dedicated 'hub' token for trade executions (LRNA)
//! * **Native Asset:** governance token
//!
//! ## Assumptions
//!
//! Below are assumptions that must be held when using this pallet.
//!
//! * First two asset in pool must be Stable Asset and Native Asset. This must be achieved by calling
//!   `initialize_pool` dispatchable.
//! * Stable asset balance and native asset balance must be transferred to omnipool account manually.
//! * All tokens added to the pool must be first registered in Asset Registry.
//! * Initial liquidity of new token being added to Omnipool must be transferred manually to pool account prior to calling add_token.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `initialize_pool` - Initializes Omnipool with Stable and Native assets. This must be executed first.
//! * `set_asset_tradable_state` - Updates state of an asset in the pool to allow/disallow trading.
//! * `add_token` - Adds token to the pool. Initial liquidity must be transffered to pool account prior to calling add_token.
//! * `add_liquidity` - Adds liquidity of selected asset to the pool. Mints corresponding position NFT.
//! * `remove_liquidity` - Removes liquidity of selected position from the pool. Partial withdrawals are allowed.
//! * `sell` - Trades an asset in for asset out by selling given amount of asset in.
//! * `buy` - Trades an asset in for asset out by buying given amount of asset out.
//! * `set_asset_tradable_state` - Updates asset's tradable asset with new flags. This allows/forbids asset operation such SELL,BUY,ADD or  REMOVE liquidtityy.
//! * `refund_refused_asset` - Refunds the initial liquidity amount sent to pool account prior to add_token if the token has been refused to be added.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::require_transactional;
use frame_support::PalletId;
use frame_support::{ensure, transactional};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, One};
use sp_runtime::traits::{CheckedAdd, CheckedSub, Zero};
use sp_std::ops::{Add, Sub};
use sp_std::prelude::*;

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use hydra_dx_math::omnipool::types::BalanceUpdate;
use hydradx_traits::Registry;
use orml_traits::MultiCurrency;
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128, Permill};

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

#[cfg(test)]
mod tests;

mod types;
pub mod weights;

use crate::types::{AssetReserveState, AssetState, Balance, Price, SimpleImbalance, Tradability};
pub use pallet::*;
pub use weights::WeightInfo;

/// NFT class id type of provided nft implementation
type NFTClassIdOf<T> = <<T as Config>::NFTHandler as Inspect<<T as frame_system::Config>::AccountId>>::ClassId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::types::{Position, Price, SimpleImbalance, Tradability};
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use hydra_dx_math::omnipool::types::BalanceUpdate;
	use sp_runtime::ArithmeticError;

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
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Add token origin
		type AddTokenOrigin: EnsureOrigin<Self::Origin>;

		/// Origin to be able to suspend asset trades and initialize Omnipool.
		type TechnicalOrigin: EnsureOrigin<Self::Origin>;

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;

		/// Native Asset ID
		#[pallet::constant]
		type HdxAssetId: Get<Self::AssetId>;

		/// Hub Asset ID
		#[pallet::constant]
		type HubAssetId: Get<Self::AssetId>;

		/// Preferred stable Asset ID
		#[pallet::constant]
		type StableCoinAssetId: Get<Self::AssetId>;

		/// Protocol fee
		#[pallet::constant]
		type ProtocolFee: Get<Permill>;

		/// Asset fee
		#[pallet::constant]
		type AssetFee: Get<Permill>;

		/// Asset weight cap
		#[pallet::constant]
		type AssetWeightCap: Get<(u32, u32)>;

		/// TVL cap
		#[pallet::constant]
		type TVLCap: Get<Balance>;

		/// Minimum trading limit
		#[pallet::constant]
		type MinimumTradingLimit: Get<Balance>;

		/// Minimum pool liquidity which can be added
		#[pallet::constant]
		type MinimumPoolLiquidity: Get<Balance>;

		/// Position identifier type
		type PositionInstanceId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;

		/// Non fungible class id
		type NFTClassId: Get<NFTClassIdOf<Self>>;

		/// Non fungible handling - mint,burn, check owner
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ Inspect<Self::AccountId, InstanceId = Self::PositionInstanceId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	/// State of an asset in the omnipool
	pub(super) type Assets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, AssetState<Balance>>;

	#[pallet::storage]
	/// Imbalance of hub asset
	pub(super) type HubAssetImbalance<T: Config> = StorageValue<_, SimpleImbalance<Balance>, ValueQuery>;

	#[pallet::storage]
	/// Total TVL. It equals to sum of each asset's tvl in omnipool
	pub(super) type TotalTVL<T: Config> = StorageValue<_, Balance, ValueQuery>;

	#[pallet::storage]
	/// Tradable state of hub asset.
	pub(super) type HubAssetTradability<T: Config> = StorageValue<_, Tradability, ValueQuery>;

	#[pallet::storage]
	/// LP positions. Maps NFT instance id to corresponding position
	pub(super) type Positions<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PositionInstanceId, Position<Balance, T::AssetId>>;

	#[pallet::storage]
	/// Position ids sequencer
	pub(super) type NextPositionId<T: Config> = StorageValue<_, T::PositionInstanceId, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An asset was added to Omnipool
		TokenAdded {
			asset_id: T::AssetId,
			initial_amount: Balance,
			initial_price: Price,
		},
		/// Liquidity of an asset was added to Omnipool.
		LiquidityAdded {
			who: T::AccountId,
			asset_id: T::AssetId,
			amount: Balance,
			position_id: T::PositionInstanceId,
		},
		/// Liquidity of an asset was removed to Omnipool.
		LiquidityRemoved {
			who: T::AccountId,
			position_id: T::PositionInstanceId,
			asset_id: T::AssetId,
			shares_removed: Balance,
		},
		/// Sell trade executed.
		SellExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
		},
		/// Buy trade executed.
		BuyExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
		},
		/// LP Position was created and NFT instance minted.
		PositionCreated {
			position_id: T::PositionInstanceId,
			owner: T::AccountId,
			asset: T::AssetId,
			amount: Balance,
			shares: Balance,
			price: Price,
		},
		/// LP Position was destroyed and NFT instance burned.
		PositionDestroyed {
			position_id: T::PositionInstanceId,
			owner: T::AccountId,
		},
		/// LP Position was created and NFT instance minted.
		PositionUpdated {
			position_id: T::PositionInstanceId,
			owner: T::AccountId,
			asset: T::AssetId,
			amount: Balance,
			shares: Balance,
			price: Price,
		},
		/// Aseet's tradable state has been updated.
		TradableStateUpdated { asset_id: T::AssetId, state: Tradability },

		/// Amount has been refunded for asset which has not been accepted to add to omnipool.
		AssetRefunded {
			asset_id: T::AssetId,
			amount: Balance,
			recipient: T::AccountId,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq))]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,
		/// Asset is already in omnipool
		AssetAlreadyAdded,
		/// Asset is not in omnipool
		AssetNotFound,
		/// No stable asset in the pool
		NoStableAssetInPool,
		/// No native asset in the pool yet.
		NoNativeAssetInPool,
		/// Adding token as protocol ( root ), token balance has not been updated prior to add token.
		MissingBalance,
		/// Invalid initial asset price. Price must be non-zero.
		InvalidInitialAssetPrice,
		/// Minimum limit has not been reached during trade.
		BuyLimitNotReached,
		/// Maximum limit has been exceeded during trade.
		SellLimitExceeded,
		/// Position has not been found.
		PositionNotFound,
		/// Insufficient shares in position
		InsufficientShares,
		/// Asset is not allowed to be bought or sold
		NotAllowed,
		/// Signed account is not owner of position instance.
		Forbidden,
		/// Asset weight cap has been exceeded.
		AssetWeightCapExceeded,
		/// TVL cap has been exceeded
		TVLCapExceeded,
		/// Asset is not registered in asset registry
		AssetNotRegistered,
		/// Provided liquidity is below minimum allowed limit
		InsufficientLiquidity,
		/// Traded amount is below minimum allowed limit
		InsufficientTradingAmount,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Initialize Omnipool with stable asset and native asset.
		///
		/// First added assets must be:
		/// - preferred stable coin asset set as `StableCoinAssetId` pallet parameter
		/// - native asset
		///
		/// Omnipool account must already have correct balances of stable and native asset.
		///
		/// Parameters:
		/// - `stable_asset_price`: Initial price of stable asset
		/// - `native_asset_price`: Initial price of stable asset
		///
		/// Emits two `TokenAdded` events when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::initialize_pool())]
		#[transactional]
		pub fn initialize_pool(
			origin: OriginFor<T>,
			stable_asset_price: Price,
			native_asset_price: Price,
		) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			ensure!(
				!Assets::<T>::contains_key(T::StableCoinAssetId::get()),
				Error::<T>::AssetAlreadyAdded
			);
			ensure!(
				!Assets::<T>::contains_key(T::HdxAssetId::get()),
				Error::<T>::AssetAlreadyAdded
			);

			ensure!(
				stable_asset_price > FixedU128::zero(),
				Error::<T>::InvalidInitialAssetPrice
			);
			ensure!(
				native_asset_price > FixedU128::zero(),
				Error::<T>::InvalidInitialAssetPrice
			);

			let native_asset_reserve = T::Currency::free_balance(T::HdxAssetId::get(), &Self::protocol_account());
			let stable_asset_reserve =
				T::Currency::free_balance(T::StableCoinAssetId::get(), &Self::protocol_account());

			// Ensure that stable asset has been transferred to protocol account
			ensure!(stable_asset_reserve > Balance::zero(), Error::<T>::MissingBalance);

			// Ensure that native asset has been transferred to protocol account
			ensure!(native_asset_reserve > Balance::zero(), Error::<T>::MissingBalance);

			let stable_asset_hub_reserve = stable_asset_price
				.checked_mul_int(stable_asset_reserve)
				.ok_or(ArithmeticError::Overflow)?;

			let native_asset_hub_reserve = native_asset_price
				.checked_mul_int(native_asset_reserve)
				.ok_or(ArithmeticError::Overflow)?;

			let native_asset_tvl = hydra_dx_math::omnipool::calculate_asset_tvl(
				native_asset_hub_reserve,
				(stable_asset_reserve, stable_asset_hub_reserve),
			)
			.ok_or(ArithmeticError::Overflow)?;

			// Create NFT class
			T::NFTHandler::create_class(
				&T::NFTClassId::get(),
				&Self::protocol_account(),
				&Self::protocol_account(),
			)?;

			// Initial stale of native and stable assets
			let stable_asset_state = AssetState::<Balance> {
				hub_reserve: stable_asset_hub_reserve,
				shares: stable_asset_reserve,
				protocol_shares: stable_asset_reserve,
				tvl: stable_asset_reserve,
				tradable: Tradability::default(),
			};

			let native_asset_state = AssetState::<Balance> {
				hub_reserve: native_asset_hub_reserve,
				shares: native_asset_reserve,
				protocol_shares: native_asset_reserve,
				tvl: native_asset_tvl,
				tradable: Tradability::default(),
			};

			// Imbalance update and total hub asset liquidity for stable asset first
			// Note: cannot be merged with native, because the calculations depend on updated values
			let delta_imbalance = Self::recalculate_imbalance(
				&((&stable_asset_state, stable_asset_reserve).into()),
				BalanceUpdate::Decrease(stable_asset_reserve),
			)
			.ok_or(ArithmeticError::Overflow)?;

			// No imbalance yet, use default value
			Self::update_imbalance(SimpleImbalance::default(), delta_imbalance)?;

			Self::update_hub_asset_liquidity(&BalanceUpdate::Increase(stable_asset_hub_reserve))?;

			// Imbalance update total hub asset with native asset next
			Self::recalculate_imbalance(
				&((&native_asset_state, native_asset_reserve).into()),
				BalanceUpdate::Decrease(native_asset_reserve),
			)
			.ok_or(ArithmeticError::Overflow)?;
			Self::update_imbalance(<HubAssetImbalance<T>>::get(), delta_imbalance)?;

			Self::update_hub_asset_liquidity(&BalanceUpdate::Increase(native_asset_hub_reserve))?;

			Self::update_tvl(&BalanceUpdate::Increase(
				native_asset_tvl
					.checked_add(stable_asset_reserve)
					.ok_or(ArithmeticError::Overflow)?,
			))?;

			<Assets<T>>::insert(T::StableCoinAssetId::get(), stable_asset_state);
			<Assets<T>>::insert(T::HdxAssetId::get(), native_asset_state);

			// Hub asset is not allowed to be bought from the pool
			<HubAssetTradability<T>>::put(Tradability::SELL);

			Self::deposit_event(Event::TokenAdded {
				asset_id: T::StableCoinAssetId::get(),
				initial_amount: stable_asset_reserve,
				initial_price: stable_asset_price,
			});

			Self::deposit_event(Event::TokenAdded {
				asset_id: T::HdxAssetId::get(),
				initial_amount: native_asset_reserve,
				initial_price: native_asset_price,
			});

			Ok(())
		}

		/// Add new token to omnipool in quantity `amount` at price `initial_price`
		///
		/// Can be called only after pool is initialized, otherwise it returns `NoStableAssetInPool`
		///
		/// Initial liquidity must be transferred to pool's account for this new token manually prior to calling `add_token`.
		///
		/// Initial liquidity is pool's account balance of the token.
		///
		/// Position NFT token is minted for `position_owner`.
		///
		/// Parameters:
		/// - `asset`: The identifier of the new asset added to the pool. Must be registered in Asset registry
		/// - `initial_price`: Initial price
		/// - `position_owner`: account id for which share are distributed in form on NFT
		///
		/// Emits `TokenAdded` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::add_token())]
		#[transactional]
		pub fn add_token(
			origin: OriginFor<T>,
			asset: T::AssetId,
			initial_price: Price,
			position_owner: T::AccountId,
		) -> DispatchResult {
			//
			// Preconditions
			//
			T::AddTokenOrigin::ensure_origin(origin)?;

			ensure!(!Assets::<T>::contains_key(asset), Error::<T>::AssetAlreadyAdded);

			ensure!(T::AssetRegistry::exists(asset), Error::<T>::AssetNotRegistered);

			ensure!(initial_price > FixedU128::zero(), Error::<T>::InvalidInitialAssetPrice);

			let (stable_asset_reserve, stable_asset_hub_reserve) = Self::stable_asset()?;

			let amount = T::Currency::free_balance(asset, &Self::protocol_account());

			ensure!(
				amount >= T::MinimumPoolLiquidity::get() && amount > 0,
				Error::<T>::MissingBalance
			);

			//
			// Calculate state changes of add token
			//

			let hub_reserve = initial_price.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;

			let asset_tvl = hydra_dx_math::omnipool::calculate_asset_tvl(
				hub_reserve,
				(stable_asset_reserve, stable_asset_hub_reserve),
			)
			.ok_or(ArithmeticError::Overflow)?;

			//
			// Post - update states
			//

			// Initial state of asset
			let state = AssetState::<Balance> {
				hub_reserve,
				shares: amount,
				protocol_shares: Balance::zero(),
				tvl: asset_tvl,
				tradable: Tradability::default(),
			};

			let lp_position = Position::<Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: amount,
				price: initial_price.into_inner(),
			};

			let instance_id = Self::create_and_mint_position_instance(&position_owner)?;

			<Positions<T>>::insert(instance_id, lp_position);

			Self::deposit_event(Event::PositionCreated {
				position_id: instance_id,
				owner: position_owner,
				asset,
				amount,
				shares: amount,
				price: initial_price,
			});

			let delta_imbalance =
				Self::recalculate_imbalance(&((&state, amount).into()), BalanceUpdate::Decrease(amount))
					.ok_or(ArithmeticError::Overflow)?;

			Self::update_imbalance(<HubAssetImbalance<T>>::get(), delta_imbalance)?;

			Self::update_hub_asset_liquidity(&BalanceUpdate::Increase(hub_reserve))?;

			Self::update_tvl(&BalanceUpdate::Increase(asset_tvl))?;

			<Assets<T>>::insert(asset, state);

			Self::deposit_event(Event::TokenAdded {
				asset_id: asset,
				initial_amount: amount,
				initial_price,
			});

			Ok(())
		}

		/// Add liquidity of asset `asset` in quantity `amount` to Omnipool
		///
		/// `add_liquidity` adds specified asset amount to pool and in exchange gives the origin
		/// corresponding shares amount in form of NFT at current price.
		///
		/// Asset's tradable state must contain ADD_LIQUIDITY flag, otherwise `NotAllowed` error is returned.
		///
		/// NFT is minted using NTFHandler which implements non-fungibles traits from frame_support.
		///
		/// Parameters:
		/// - `asset`: The identifier of the new asset added to the pool. Must be already in the pool
		/// - `amount`: Amount of asset added to omnipool
		///
		/// Emits `LiquidityAdded` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity())]
		#[transactional]
		pub fn add_liquidity(origin: OriginFor<T>, asset: T::AssetId, amount: Balance) -> DispatchResult {
			//
			// Precondtions
			//
			let who = ensure_signed(origin)?;

			ensure!(
				amount >= T::MinimumPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidity
			);

			ensure!(
				T::Currency::free_balance(asset, &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			let stable_asset = Self::stable_asset()?;

			let asset_state = Self::load_asset_state(asset)?;

			ensure!(
				asset_state.tradable.contains(Tradability::ADD_LIQUIDITY),
				Error::<T>::NotAllowed
			);

			//
			// Calculate add liquidity state changes
			//
			let state_changes = hydra_dx_math::omnipool::calculate_add_liquidity_state_changes(
				&(&asset_state).into(),
				amount,
				stable_asset,
				asset == T::StableCoinAssetId::get(),
			)
			.ok_or(ArithmeticError::Overflow)?;

			let new_asset_state = asset_state
				.delta_update(&state_changes.asset)
				.ok_or(ArithmeticError::Overflow)?;

			let hub_reserve_ratio = FixedU128::checked_from_rational(
				new_asset_state.hub_reserve,
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account())
					.checked_add(*state_changes.asset.delta_hub_reserve)
					.ok_or(ArithmeticError::Overflow)?,
			)
			.ok_or(ArithmeticError::DivisionByZero)?;

			ensure!(
				hub_reserve_ratio <= Self::asset_weight_cap(),
				Error::<T>::AssetWeightCapExceeded
			);

			let updated_asset_price = new_asset_state.price().ok_or(ArithmeticError::DivisionByZero)?;

			//
			// Post - update states
			//

			// Create LP position with given shares
			let lp_position = Position::<Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: *state_changes.asset.delta_shares,
				// Note: position needs price after asset state is updated.
				price: updated_asset_price.into_inner(),
			};

			let instance_id = Self::create_and_mint_position_instance(&who)?;

			<Positions<T>>::insert(instance_id, lp_position);

			Self::deposit_event(Event::PositionCreated {
				position_id: instance_id,
				owner: who.clone(),
				asset,
				amount,
				shares: *state_changes.asset.delta_shares,
				price: updated_asset_price,
			});

			T::Currency::transfer(
				asset,
				&who,
				&Self::protocol_account(),
				*state_changes.asset.delta_reserve,
			)?;

			let delta_imbalance = Self::recalculate_imbalance(&new_asset_state, state_changes.delta_imbalance)
				.ok_or(ArithmeticError::Overflow)?;
			Self::update_imbalance(<HubAssetImbalance<T>>::get(), delta_imbalance)?;

			Self::update_tvl(&state_changes.asset.delta_tvl)?;

			Self::update_hub_asset_liquidity(&state_changes.asset.delta_hub_reserve)?;

			Self::set_asset_state(asset, new_asset_state);

			Self::deposit_event(Event::LiquidityAdded {
				who,
				asset_id: asset,
				amount,
				position_id: instance_id,
			});
			Ok(())
		}

		/// Remove liquidity of asset `asset` in quantity `amount` from Omnipool
		///
		/// `remove_liquidity` removes specified shares amount from given PositionId (NFT instance).
		///
		/// Asset's tradable state must contain REMOVE_LIQUIDITY flag, otherwise `NotAllowed` error is returned.
		///
		/// if all shares from given position are removed, NFT is burned.
		///
		/// Parameters:
		/// - `position_id`: The identifier of position which liquidity is removed from.
		/// - `amount`: Amount of shares removed from omnipool
		///
		/// Emits `LiquidityRemoved` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity())]
		#[transactional]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			position_id: T::PositionInstanceId,
			amount: Balance,
		) -> DispatchResult {
			//
			// Preconditions
			//
			let who = ensure_signed(origin)?;

			ensure!(
				T::NFTHandler::owner(&T::NFTClassId::get(), &position_id) == Some(who.clone()),
				Error::<T>::Forbidden
			);

			let position = Positions::<T>::get(position_id).ok_or(Error::<T>::PositionNotFound)?;

			ensure!(position.shares >= amount, Error::<T>::InsufficientShares);

			let stable_asset = Self::stable_asset()?;

			let asset_id = position.asset_id;

			let asset_state = Self::load_asset_state(asset_id)?;

			ensure!(
				asset_state.tradable.contains(Tradability::REMOVE_LIQUIDITY),
				Error::<T>::NotAllowed
			);

			//
			// calculate state changes of remove liquidity
			//

			let state_changes = hydra_dx_math::omnipool::calculate_remove_liquidity_state_changes(
				&(&asset_state).into(),
				amount,
				&(&position).into(),
				stable_asset,
				asset_id == T::StableCoinAssetId::get(),
			)
			.ok_or(ArithmeticError::Overflow)?;

			let new_asset_state = asset_state
				.delta_update(&state_changes.asset)
				.ok_or(ArithmeticError::Overflow)?;

			// Update position state
			let updated_position = position
				.delta_update(
					&state_changes.delta_position_reserve,
					&state_changes.delta_position_shares,
				)
				.ok_or(ArithmeticError::Overflow)?;

			//
			// Post - update states
			//

			T::Currency::transfer(
				asset_id,
				&Self::protocol_account(),
				&who,
				*state_changes.asset.delta_reserve,
			)?;

			let delta_imbalance = Self::recalculate_imbalance(&new_asset_state, state_changes.delta_imbalance)
				.ok_or(ArithmeticError::Overflow)?;
			Self::update_imbalance(<HubAssetImbalance<T>>::get(), delta_imbalance)?;

			Self::update_tvl(&state_changes.asset.delta_tvl)?;

			Self::update_hub_asset_liquidity(&state_changes.asset.delta_hub_reserve)?;

			// LP receives some hub asset
			if state_changes.lp_hub_amount > Balance::zero() {
				T::Currency::transfer(
					T::HubAssetId::get(),
					&Self::protocol_account(),
					&who,
					state_changes.lp_hub_amount,
				)?;
			}

			if updated_position.shares == Balance::zero() {
				// All liquidity removed, remove position and burn NFT instance

				<Positions<T>>::remove(position_id);
				T::NFTHandler::burn_from(&T::NFTClassId::get(), &position_id)?;

				Self::deposit_event(Event::PositionDestroyed {
					position_id,
					owner: who.clone(),
				});
			} else {
				Self::deposit_event(Event::PositionUpdated {
					position_id,
					owner: who.clone(),
					asset: asset_id,
					amount: updated_position.amount,
					shares: updated_position.shares,
					price: FixedU128::from_inner(updated_position.price.into()),
				});

				<Positions<T>>::insert(position_id, updated_position);
			}

			Self::set_asset_state(asset_id, new_asset_state);

			Self::deposit_event(Event::LiquidityRemoved {
				who,
				position_id,
				asset_id,
				shares_removed: amount,
			});

			Ok(())
		}

		/// Sacrifice LP position in favor of pool.
		///
		/// A position is destroyed and liquidity owned by LP becomes pool owned liquidity.
		///
		/// Only owner of position can perform this action.
		///
		/// Emits `PositionDestroyed`.
		#[pallet::weight(<T as Config>::WeightInfo::sacrifice_position())]
		#[transactional]
		pub fn sacrifice_position(origin: OriginFor<T>, position_id: T::PositionInstanceId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let position = Positions::<T>::get(position_id).ok_or(Error::<T>::PositionNotFound)?;

			ensure!(
				T::NFTHandler::owner(&T::NFTClassId::get(), &position_id) == Some(who.clone()),
				Error::<T>::Forbidden
			);

			Assets::<T>::try_mutate(position.asset_id, |maybe_asset| -> DispatchResult {
				let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				asset_state.protocol_shares = asset_state
					.protocol_shares
					.checked_add(position.shares)
					.ok_or(ArithmeticError::Overflow)?;

				Ok(())
			})?;

			// Desotry position and burn NFT
			<Positions<T>>::remove(position_id);
			T::NFTHandler::burn_from(&T::NFTClassId::get(), &position_id)?;

			Self::deposit_event(Event::PositionDestroyed {
				position_id,
				owner: who,
			});

			Ok(())
		}

		/// Execute a swap of `asset_in` for `asset_out`.
		///
		/// Price is determined by the Omnipool.
		///
		/// Hub asset is traded separately.
		///
		/// Asset's tradable states must contain SELL flag for asset_in and BUY flag for asset_out, otherwise `NotAllowed` error is returned.
		///
		/// Parameters:
		/// - `asset_in`: ID of asset sold to the pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `amount`: Amount of asset sold
		/// - `min_buy_amount`: Minimum amount required to receive
		///
		/// Emits `SellExecuted` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::sell())]
		#[transactional]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount: Balance,
			min_buy_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				amount >= T::MinimumTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			// Special handling when one of the asset is Hub Asset
			// Math is simplified and asset_in is actually part of asset_out state in this case
			if asset_in == T::HubAssetId::get() {
				return Self::sell_hub_asset(&who, asset_out, amount, min_buy_amount);
			}

			if asset_out == T::HubAssetId::get() {
				return Self::sell_asset_for_hub_asset(&who, asset_in, amount, min_buy_amount);
			}

			let asset_in_state = Self::load_asset_state(asset_in)?;
			let asset_out_state = Self::load_asset_state(asset_out)?;

			ensure!(
				Self::allow_assets(&asset_in_state, &asset_out_state),
				Error::<T>::NotAllowed
			);

			let current_imbalance = <HubAssetImbalance<T>>::get();

			let state_changes = hydra_dx_math::omnipool::calculate_sell_state_changes(
				&(&asset_in_state).into(),
				&(&asset_out_state).into(),
				amount,
				T::AssetFee::get(),
				T::ProtocolFee::get(),
				current_imbalance.value,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(
				*state_changes.asset_out.delta_reserve >= min_buy_amount,
				Error::<T>::BuyLimitNotReached
			);

			let new_asset_in_state = asset_in_state
				.delta_update(&state_changes.asset_in)
				.ok_or(ArithmeticError::Overflow)?;
			let new_asset_out_state = asset_out_state
				.delta_update(&state_changes.asset_out)
				.ok_or(ArithmeticError::Overflow)?;

			T::Currency::transfer(
				asset_in,
				&who,
				&Self::protocol_account(),
				*state_changes.asset_in.delta_reserve,
			)?;
			T::Currency::transfer(
				asset_out,
				&Self::protocol_account(),
				&who,
				*state_changes.asset_out.delta_reserve,
			)?;

			// Hub liquidity update - work out difference between in and amount so only one update needed.
			let delta_hub_asset = state_changes
				.asset_in
				.delta_hub_reserve
				.merge(
					state_changes
						.asset_out
						.delta_hub_reserve
						.merge(BalanceUpdate::Increase(state_changes.hdx_hub_amount))
						.ok_or(ArithmeticError::Overflow)?,
				)
				.ok_or(ArithmeticError::Overflow)?;

			Self::update_hub_asset_liquidity(&delta_hub_asset)?;

			Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

			Self::update_hdx_subpool_hub_asset(state_changes.hdx_hub_amount)?;

			Self::set_asset_state(asset_in, new_asset_in_state);
			Self::set_asset_state(asset_out, new_asset_out_state);

			Self::deposit_event(Event::SellExecuted {
				who,
				asset_in,
				asset_out,
				amount_in: amount,
				amount_out: *state_changes.asset_out.delta_reserve,
			});

			Ok(())
		}

		/// Execute a swap of `asset_out` for `asset_in`.
		///
		/// Price is determined by the Omnipool.
		///
		/// Hub asset is traded separately.
		///
		/// Asset's tradable states must contain SELL flag for asset_in and BUY flag for asset_out, otherwise `NotAllowed` error is returned.
		///
		/// Parameters:
		/// - `asset_in`: ID of asset sold to the pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `amount`: Amount of asset sold
		/// - `max_sell_amount`: Maximum amount to be sold.
		///
		/// Emits `BuyExecuted` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::buy())]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: T::AssetId,
			asset_in: T::AssetId,
			amount: Balance,
			max_sell_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				amount >= T::MinimumTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			// Special handling when one of the asset is Hub Asset
			if asset_out == T::HubAssetId::get() {
				return Self::buy_hub_asset(&who, asset_in, amount, max_sell_amount);
			}

			if asset_in == T::HubAssetId::get() {
				return Self::buy_asset_for_hub_asset(&who, asset_out, amount, max_sell_amount);
			}

			let asset_in_state = Self::load_asset_state(asset_in)?;
			let asset_out_state = Self::load_asset_state(asset_out)?;

			ensure!(
				Self::allow_assets(&asset_in_state, &asset_out_state),
				Error::<T>::NotAllowed
			);

			ensure!(asset_out_state.reserve >= amount, Error::<T>::InsufficientLiquidity);

			let current_imbalance = <HubAssetImbalance<T>>::get();

			let state_changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
				&(&asset_in_state).into(),
				&(&asset_out_state).into(),
				amount,
				T::AssetFee::get(),
				T::ProtocolFee::get(),
				current_imbalance.value,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= *state_changes.asset_in.delta_reserve,
				Error::<T>::InsufficientBalance
			);

			ensure!(
				*state_changes.asset_in.delta_reserve <= max_sell_amount,
				Error::<T>::SellLimitExceeded
			);

			let new_asset_in_state = asset_in_state
				.delta_update(&state_changes.asset_in)
				.ok_or(ArithmeticError::Overflow)?;
			let new_asset_out_state = asset_out_state
				.delta_update(&state_changes.asset_out)
				.ok_or(ArithmeticError::Overflow)?;

			T::Currency::transfer(
				asset_in,
				&who,
				&Self::protocol_account(),
				*state_changes.asset_in.delta_reserve,
			)?;
			T::Currency::transfer(
				asset_out,
				&Self::protocol_account(),
				&who,
				*state_changes.asset_out.delta_reserve,
			)?;

			// Hub liquidity update - work out difference between in and amount so only one update needed.
			let delta_hub_asset = state_changes
				.asset_in
				.delta_hub_reserve
				.merge(
					state_changes
						.asset_out
						.delta_hub_reserve
						.merge(BalanceUpdate::Increase(state_changes.hdx_hub_amount))
						.ok_or(ArithmeticError::Overflow)?,
				)
				.ok_or(ArithmeticError::Overflow)?;
			Self::update_hub_asset_liquidity(&delta_hub_asset)?;

			Self::update_hdx_subpool_hub_asset(state_changes.hdx_hub_amount)?;

			Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

			Self::set_asset_state(asset_in, new_asset_in_state);
			Self::set_asset_state(asset_out, new_asset_out_state);

			Self::deposit_event(Event::BuyExecuted {
				who,
				asset_in,
				asset_out,
				amount_in: *state_changes.asset_in.delta_reserve,
				amount_out: *state_changes.asset_out.delta_reserve,
			});

			Ok(())
		}

		/// Update asset's tradable state.
		///
		/// Parameters:
		/// - `asset_id`: asset id
		/// - `state`: new state
		///
		/// Emits `TradableStateUpdated` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_tradable_state())]
		#[transactional]
		pub fn set_asset_tradable_state(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			state: Tradability,
		) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			if asset_id == T::HubAssetId::get() {
				HubAssetTradability::<T>::mutate(|value| -> DispatchResult {
					*value = state;
					Self::deposit_event(Event::TradableStateUpdated { asset_id, state });
					Ok(())
				})
			} else {
				Assets::<T>::try_mutate(asset_id, |maybe_asset| -> DispatchResult {
					let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

					asset_state.tradable = state;
					Self::deposit_event(Event::TradableStateUpdated { asset_id, state });

					Ok(())
				})
			}
		}

		/// Refund given amount of asset to a recipient.
		///
		/// A refund is needed when a token is refused to be added to Omnipool, and initial liquidity of the asset has been already transferred to pool's account.
		///
		/// Transfer is performed only when asset is not in Omnipool and pool's balance has sufficient amount.
		///
		/// Only `AddTokenOrigin` can perform this operition -same as `add_token`o
		///
		/// Emits `AssetRefunded`
		#[pallet::weight(<T as Config>::WeightInfo::refund_refused_token())]
		#[transactional]
		pub fn refund_refused_asset(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			amount: Balance,
			recipient: T::AccountId,
		) -> DispatchResult {
			T::AddTokenOrigin::ensure_origin(origin)?;

			// Make sure that asset is not in the pool
			ensure!(!Assets::<T>::contains_key(asset_id), Error::<T>::AssetAlreadyAdded);

			let pool_balance = T::Currency::free_balance(asset_id, &Self::protocol_account());

			ensure!(pool_balance >= amount, Error::<T>::InsufficientBalance);

			T::Currency::transfer(asset_id, &Self::protocol_account(), &recipient, amount)?;

			Self::deposit_event(Event::AssetRefunded {
				asset_id,
				amount,
				recipient,
			});

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	/// Protocol account address
	fn protocol_account() -> T::AccountId {
		PalletId(*b"omnipool").into_account()
	}

	/// Convert asset weight cap to FixedU128
	fn asset_weight_cap() -> Price {
		let fee = T::AssetWeightCap::get();
		match fee {
			(_, 0) => FixedU128::zero(),
			(a, b) => FixedU128::from((a, b)),
		}
	}

	/// Retrieve stable asset detail from the pool.
	/// Return NoStableCoinInPool if stable asset is not yet in the pool.
	fn stable_asset() -> Result<(Balance, Balance), DispatchError> {
		let stable_asset = <Assets<T>>::get(T::StableCoinAssetId::get()).ok_or(Error::<T>::NoStableAssetInPool)?;
		let stable_reserve = T::Currency::free_balance(T::StableCoinAssetId::get(), &Self::protocol_account());
		Ok((stable_reserve, stable_asset.hub_reserve))
	}

	/// Retrieve state of asset from the pool and its pool balance
	fn load_asset_state(asset_id: T::AssetId) -> Result<AssetReserveState<Balance>, DispatchError> {
		let state = <Assets<T>>::get(asset_id).ok_or(Error::<T>::AssetNotFound)?;
		let reserve = T::Currency::free_balance(asset_id, &Self::protocol_account());
		Ok((state, reserve).into())
	}

	/// Set new state of asset.
	/// This converts the new state into correct state type ) by removing the reserve)
	fn set_asset_state(asset_id: T::AssetId, new_state: AssetReserveState<Balance>) {
		<Assets<T>>::insert(asset_id, Into::<AssetState<Balance>>::into(new_state));
	}

	/// Generate an nft instance id and mint NFT into the class and instance.
	#[require_transactional]
	fn create_and_mint_position_instance(owner: &T::AccountId) -> Result<T::PositionInstanceId, DispatchError> {
		<NextPositionId<T>>::try_mutate(|current_value| -> Result<T::PositionInstanceId, DispatchError> {
			let next_position_id = *current_value;

			T::NFTHandler::mint_into(&T::NFTClassId::get(), &next_position_id, owner)?;

			*current_value = current_value
				.checked_add(&T::PositionInstanceId::one())
				.ok_or(ArithmeticError::Overflow)?;

			Ok(next_position_id)
		})
	}

	/// Update Hub asset side of HDX subpool annd add given amount to hub_asset_reserve
	fn update_hdx_subpool_hub_asset(hub_asset_amount: Balance) -> DispatchResult {
		if hub_asset_amount > Balance::zero() {
			let mut native_subpool = Assets::<T>::get(T::HdxAssetId::get()).ok_or(Error::<T>::AssetNotFound)?;
			native_subpool.hub_reserve = native_subpool
				.hub_reserve
				.checked_add(hub_asset_amount)
				.ok_or(ArithmeticError::Overflow)?;
			<Assets<T>>::insert(T::HdxAssetId::get(), native_subpool);
		}
		Ok(())
	}

	/// Update total hub asset liquidity and write new value to storage.
	/// Update total issueance if AdjustSupply is specified.
	#[require_transactional]
	fn update_hub_asset_liquidity(delta_amount: &BalanceUpdate<Balance>) -> DispatchResult {
		match delta_amount {
			BalanceUpdate::Increase(amount) => {
				T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), *amount)
			}
			BalanceUpdate::Decrease(amount) => {
				T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), *amount)
			}
		}
	}

	/// Update imbalance with given delta_imbalance - increase or decrease
	fn update_imbalance(
		current_imbalance: SimpleImbalance<Balance>,
		delta_imbalance: BalanceUpdate<Balance>,
	) -> DispatchResult {
		let imbalance = match delta_imbalance {
			BalanceUpdate::Decrease(amount) => current_imbalance.sub(amount).ok_or(ArithmeticError::Overflow)?,
			BalanceUpdate::Increase(amount) => current_imbalance.add(amount).ok_or(ArithmeticError::Overflow)?,
		};
		<HubAssetImbalance<T>>::put(imbalance);

		Ok(())
	}

	/// Recalculate imbalance based on current imbalance and hub liquidity
	fn recalculate_imbalance(
		asset_state: &AssetReserveState<Balance>,
		delta_amount: BalanceUpdate<Balance>,
	) -> Option<BalanceUpdate<Balance>> {
		let current_imbalance = <HubAssetImbalance<T>>::get();
		let current_hub_asset_liquidity = T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

		let delta_imbalance = hydra_dx_math::omnipool::calculate_delta_imbalance(
			&(asset_state.into()),
			*delta_amount,
			current_imbalance.value,
			current_hub_asset_liquidity,
		)?;

		match delta_amount {
			BalanceUpdate::Increase(_) => Some(BalanceUpdate::Increase(delta_imbalance)),
			BalanceUpdate::Decrease(_) => Some(BalanceUpdate::Decrease(delta_imbalance)),
		}
	}

	/// Update total tvl balance and check TVL cap if TVL increased.
	#[require_transactional]
	fn update_tvl(delta_tvl: &BalanceUpdate<Balance>) -> DispatchResult {
		<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
			match delta_tvl {
				BalanceUpdate::Increase(amount) => {
					*tvl = tvl.checked_add(*amount).ok_or(ArithmeticError::Overflow)?;
					ensure!(*tvl <= T::TVLCap::get(), Error::<T>::TVLCapExceeded);
				}
				BalanceUpdate::Decrease(amount) => *tvl = tvl.checked_sub(*amount).ok_or(ArithmeticError::Underflow)?,
			}
			Ok(())
		})
	}

	/// Check if assets can be traded - asset_in must be allowed to be sold and asset_out allowed to be bought.
	fn allow_assets(asset_in: &AssetReserveState<Balance>, asset_out: &AssetReserveState<Balance>) -> bool {
		asset_in.tradable.contains(Tradability::SELL) && asset_out.tradable.contains(Tradability::BUY)
	}

	/// Swap hub asset for asset_out.
	/// Special handling of sell trade where asset in is Hub Asset.
	fn sell_hub_asset(who: &T::AccountId, asset_out: T::AssetId, amount: Balance, limit: Balance) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::SELL),
			Error::<T>::NotAllowed
		);

		let asset_out_state = Self::load_asset_state(asset_out)?;

		ensure!(
			asset_out_state.tradable.contains(Tradability::BUY),
			Error::<T>::NotAllowed
		);

		let state_changes = hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
			&(&asset_out_state).into(),
			amount,
			T::AssetFee::get(),
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(
			*state_changes.asset.delta_reserve >= limit,
			Error::<T>::BuyLimitNotReached
		);

		let new_asset_out_state = asset_out_state
			.delta_update(&state_changes.asset)
			.ok_or(ArithmeticError::Overflow)?;

		// Token updates
		T::Currency::transfer(
			T::HubAssetId::get(),
			who,
			&Self::protocol_account(),
			*state_changes.asset.delta_hub_reserve,
		)?;
		T::Currency::transfer(
			asset_out,
			&Self::protocol_account(),
			who,
			*state_changes.asset.delta_reserve,
		)?;

		// Imbalance update
		let current_imbalance = <HubAssetImbalance<T>>::get();
		Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

		Self::set_asset_state(asset_out, new_asset_out_state);

		Self::deposit_event(Event::SellExecuted {
			who: who.clone(),
			asset_in: T::HubAssetId::get(),
			asset_out,
			amount_in: *state_changes.asset.delta_hub_reserve,
			amount_out: *state_changes.asset.delta_reserve,
		});

		Ok(())
	}

	/// Swap asset for Hub Asset
	/// Special handling of buy trade where asset in is Hub Asset.
	fn buy_asset_for_hub_asset(
		who: &T::AccountId,
		asset_out: T::AssetId,
		amount: Balance,
		limit: Balance,
	) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::SELL),
			Error::<T>::NotAllowed
		);

		let asset_out_state = Self::load_asset_state(asset_out)?;

		ensure!(
			asset_out_state.tradable.contains(Tradability::BUY),
			Error::<T>::NotAllowed
		);

		let state_changes = hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
			&(&asset_out_state).into(),
			amount,
			T::AssetFee::get(),
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(
			*state_changes.asset.delta_reserve <= limit,
			Error::<T>::SellLimitExceeded
		);

		let new_asset_out_state = asset_out_state
			.delta_update(&state_changes.asset)
			.ok_or(ArithmeticError::Overflow)?;

		T::Currency::transfer(
			T::HubAssetId::get(),
			who,
			&Self::protocol_account(),
			*state_changes.asset.delta_hub_reserve,
		)?;
		T::Currency::transfer(
			asset_out,
			&Self::protocol_account(),
			who,
			*state_changes.asset.delta_reserve,
		)?;

		let current_imbalance = <HubAssetImbalance<T>>::get();
		Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

		Self::set_asset_state(asset_out, new_asset_out_state);

		Self::deposit_event(Event::BuyExecuted {
			who: who.clone(),
			asset_in: T::HubAssetId::get(),
			asset_out,
			amount_in: *state_changes.asset.delta_hub_reserve,
			amount_out: *state_changes.asset.delta_reserve,
		});

		Ok(())
	}

	/// Buy hub asset from the pool
	/// Special handling of buy trade where asset out is Hub Asset.
	fn buy_hub_asset(_who: &T::AccountId, _asset_in: T::AssetId, _amount: Balance, _limit: Balance) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::BUY),
			Error::<T>::NotAllowed
		);

		// Note: Currently not allowed at all, neither math is done for this case
		// this is already ready when hub asset will be allowed to be bought from the pool

		Err(Error::<T>::NotAllowed.into())
	}

	/// Swap asset for Hub Asset
	/// Special handling of sell trade where asset out is Hub Asset.
	fn sell_asset_for_hub_asset(
		_who: &T::AccountId,
		_asset_in: T::AssetId,
		_amount: Balance,
		_limit: Balance,
	) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::BUY),
			Error::<T>::NotAllowed
		);

		// Note: Currently not allowed at all, neither math is done for this case
		// this is already ready when hub asset will be allowed to be bought from the pool

		Err(Error::<T>::NotAllowed.into())
	}
}
