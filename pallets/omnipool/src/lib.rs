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

//! # Omnipool Pallet
//!
//! TBD

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::require_transactional;
use frame_support::sp_runtime::FixedPointOperand;
use frame_support::PalletId;
use frame_support::{ensure, transactional};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned};
use sp_runtime::traits::{CheckedAdd, CheckedMul, CheckedSub, Zero};
use sp_std::prelude::*;
use std::ops::{Add, Sub};

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use hydradx_traits::Registry;
use orml_traits::MultiCurrency;
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128};

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod math;
mod types;
pub mod weights;

use crate::math::calculate_sell_hub_state_changes;
use crate::types::{AssetState, BalanceUpdate, HubAssetIssuanceUpdate, Price, SimpleImbalance, Tradable};
use math::calculate_sell_state_changes;
pub use pallet::*;
pub use weights::WeightInfo;

/// NFT class id type of provided nft implementation
type NFTClassIdOf<T> = <<T as Config>::NFTHandler as Inspect<<T as frame_system::Config>::AccountId>>::ClassId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::math::{
		calculate_add_liquidity_state_changes, calculate_buy_state_changes, calculate_remove_liquidity_state_changes,
	};
	use crate::types::{AssetState, Position, Price, SimpleImbalance, Tradable};
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::{ArithmeticError, FixedPointNumber};

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The units in which we handle balances.
		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ FixedPointOperand
			// FixedU128 is missing MaxEncodedLen impl, which is added in 0.9.17
			// When upgraded to 0.9.17, the position price will be stored as FixedU128, therefore
			// there will no longer the need for conversion.
			+ From<u128>
			+ Into<u128>;

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
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Self::Balance>;

		/// Add token origin
		type AddTokenOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Self::Balance, DispatchError>;

		/// Native Asset ID
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Hub Asset ID
		#[pallet::constant]
		type HubAssetId: Get<Self::AssetId>;

		/// Preferred stable Asset ID
		#[pallet::constant]
		type StableCoinAssetId: Get<Self::AssetId>;

		/// Protocol fee
		#[pallet::constant]
		type ProtocolFee: Get<(u32, u32)>;

		/// Asset fee
		#[pallet::constant]
		type AssetFee: Get<(u32, u32)>;

		/// Asset weight cap
		#[pallet::constant]
		type AssetWeightCap: Get<(u32, u32)>;

		/// TVL cap
		#[pallet::constant]
		type TVLCap: Get<Self::Balance>;

		/// Minimum trading limit
		#[pallet::constant]
		type MinimumTradingLimit: Get<Self::Balance>;

		/// Minimum pool liquidity which can be added
		#[pallet::constant]
		type MinimumPoolLiquidity: Get<Self::Balance>;

		/// Position identifier type
		type PositionInstanceId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;

		/// Non fungible class id
		type NFTClassId: Get<NFTClassIdOf<Self>>;

		/// Non fungible handling - mint,burn, check owner
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ Inspect<Self::AccountId, InstanceId = Self::PositionInstanceId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: weights::WeightInfo;
	}

	#[pallet::storage]
	/// State of an asset in the omnipool
	pub(super) type Assets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, AssetState<T::Balance>>;

	#[pallet::storage]
	/// Imbalance of hub asset
	pub(super) type HubAssetImbalance<T: Config> = StorageValue<_, SimpleImbalance<T::Balance>, ValueQuery>;

	#[pallet::storage]
	/// Total TVL. It equals to sum of each asset's tvl in omnipool
	pub(super) type TotalTVL<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::storage]
	/// Total amount of hub asset reserve. It equals to sum of hub_reserve of each asset in omnipool
	pub(super) type HubAssetLiquidity<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::storage]
	/// LP positions. Maps NFT instance id to corresponding position
	pub(super) type Positions<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PositionInstanceId, Position<T::Balance, T::AssetId>>;

	#[pallet::storage]
	/// Position ids sequencer
	pub(super) type PositionInstanceSequencer<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An asset was added to Omnipool
		TokenAdded {
			asset_id: T::AssetId,
			initial_amount: T::Balance,
			initial_price: Price,
		},
		/// Liquidity of an asset was added to Omnipool.
		LiquidityAdded {
			from: T::AccountId,
			asset_id: T::AssetId,
			amount: T::Balance,
			position_id: T::PositionInstanceId,
		},
		/// Liquidity of an asset was removed to Omnipool.
		LiquidityRemoved {
			who: T::AccountId,
			position_id: T::PositionInstanceId,
			asset_id: T::AssetId,
			shares_removed: T::Balance,
		},
		/// Sell trade executed.
		SellExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: T::Balance,
			amount_out: T::Balance,
		},
		/// Buy trade executed.
		BuyExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: T::Balance,
			amount_out: T::Balance,
		},
		/// LP Position was created and NFT instance minted.
		PositionCreated {
			position_id: T::PositionInstanceId,
			owner: T::AccountId,
			asset: T::AssetId,
			amount: T::Balance,
			shares: T::Balance,
			price: Price,
		},
		/// LP Position was destroyed and NFT instance burned.
		PositionDestroyed {
			position_id: T::PositionInstanceId,
			owner: T::AccountId,
		},
		/// Aseet's tradable state has been updated.
		TradableStateUpdated { asset_id: T::AssetId, state: Tradable },
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
		/// - `stable_asset_amount`: Amount of stable asset
		/// - `native_asset_amount`: Amount of native asset
		/// - `stable_asset_price`: Initial price of stable asset
		/// - `native_asset_price`: Initial price of stable asset
		///
		/// Emits two `TokenAdded` events when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::add_token())]
		#[transactional]
		pub fn initialize_pool(
			origin: OriginFor<T>,
			stable_asset_amount: T::Balance,
			native_asset_amount: T::Balance,
			stable_asset_price: Price,
			native_asset_price: Price,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				!Assets::<T>::contains_key(T::StableCoinAssetId::get()),
				Error::<T>::AssetAlreadyAdded
			);
			ensure!(
				!Assets::<T>::contains_key(T::NativeAssetId::get()),
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

			let (stable_asset_reserve, stable_asset_hub_reserve) = (
				stable_asset_amount,
				stable_asset_price
					.checked_mul_int(stable_asset_amount)
					.ok_or(ArithmeticError::Overflow)?,
			);

			let (native_asset_reserve, native_asset_hub_reserve) = (
				native_asset_amount,
				native_asset_price
					.checked_mul_int(native_asset_amount)
					.ok_or(ArithmeticError::Overflow)?,
			);

			// Ensure that stable asset has been transferred to protocol account
			ensure!(
				T::Currency::free_balance(T::StableCoinAssetId::get(), &Self::protocol_account())
					>= stable_asset_reserve,
				Error::<T>::MissingBalance
			);

			// Ensure that native asset has been transferred to protocol account
			ensure!(
				T::Currency::free_balance(T::NativeAssetId::get(), &Self::protocol_account()) >= native_asset_reserve,
				Error::<T>::MissingBalance
			);

			// Initial stale of native and stable assets
			let stable_asset_state = AssetState::<T::Balance> {
				reserve: stable_asset_reserve,
				hub_reserve: stable_asset_hub_reserve,
				shares: stable_asset_reserve,
				protocol_shares: stable_asset_reserve,
				tvl: stable_asset_reserve,
				tradable: Tradable::default(),
			};
			let native_asset_state = AssetState::<T::Balance> {
				reserve: native_asset_reserve,
				hub_reserve: native_asset_hub_reserve,
				shares: native_asset_amount,
				protocol_shares: native_asset_amount,
				tvl: native_asset_amount,
				tradable: Tradable::default(),
			};

			// Imbalance update and total hub asset liquidity for stable asset first
			// Note: cannot be merged with native, because the calculations depend on updated values
			Self::recalculate_imbalance(&stable_asset_state, BalanceUpdate::Decrease(stable_asset_reserve))?;
			Self::update_hub_asset_liquidity(
				&BalanceUpdate::Increase(stable_asset_hub_reserve),
				HubAssetIssuanceUpdate::AdjustSupply,
			)?;

			// Imbalance update total hub asset with native asset next
			Self::recalculate_imbalance(&native_asset_state, BalanceUpdate::Decrease(native_asset_reserve))?;

			Self::update_hub_asset_liquidity(
				&BalanceUpdate::Increase(native_asset_hub_reserve),
				HubAssetIssuanceUpdate::AdjustSupply,
			)?;

			// TVL update
			<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
				*tvl = native_asset_price
					.checked_mul(
						&Price::checked_from_rational(stable_asset_reserve, stable_asset_hub_reserve)
							.ok_or(ArithmeticError::DivisionByZero)?,
					)
					.and_then(|v| v.checked_mul_int(native_asset_amount))
					.and_then(|v| v.checked_add(&stable_asset_amount))
					.ok_or(ArithmeticError::Overflow)?;
				Ok(())
			})?;

			<Assets<T>>::insert(T::StableCoinAssetId::get(), stable_asset_state);
			<Assets<T>>::insert(T::NativeAssetId::get(), native_asset_state);

			Self::deposit_event(Event::TokenAdded {
				asset_id: T::StableCoinAssetId::get(),
				initial_amount: stable_asset_amount,
				initial_price: stable_asset_price,
			});

			Self::deposit_event(Event::TokenAdded {
				asset_id: T::NativeAssetId::get(),
				initial_amount: native_asset_amount,
				initial_price: native_asset_price,
			});

			Ok(())
		}

		/// Add new token to omnipool in quantity `amount` at price `initial_price`
		///
		/// Can be called only after pool is initialized, otherwise it returns `NoStableAssetInPool`
		///
		/// Position is minted for LP.
		///
		/// Parameters:
		/// - `asset`: The identifier of the new asset added to the pool. Must be registered in Asset registry
		/// - `amount`: Amount of asset added to omnipool
		/// - `initial_price`: Initial price
		///
		/// Emits `TokenAdded` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::add_token())]
		#[transactional]
		pub fn add_token(
			origin: OriginFor<T>,
			asset: T::AssetId,
			amount: T::Balance,
			initial_price: Price,
		) -> DispatchResult {
			let who = T::AddTokenOrigin::ensure_origin(origin)?;

			ensure!(!Assets::<T>::contains_key(asset), Error::<T>::AssetAlreadyAdded);

			ensure!(T::AssetRegistry::exists(asset), Error::<T>::AssetNotRegistered);

			ensure!(initial_price > FixedU128::zero(), Error::<T>::InvalidInitialAssetPrice);

			// Retrieve stable asset and native asset details first - we fail early if they are not yet in the pool.
			let (stable_asset_reserve, stable_asset_hub_reserve) = Self::stable_asset()?;

			let hub_reserve = initial_price.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;

			// Initial stale of asset
			let state = AssetState::<T::Balance> {
				reserve: amount,
				hub_reserve,
				shares: amount,
				protocol_shares: amount,
				tvl: amount,
				tradable: Tradable::default(),
			};

			T::Currency::transfer(asset, &who, &Self::protocol_account(), amount)?;

			// if provided by LP, create and mint a position instance
			let lp_position = Position::<T::Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: amount,
				price: Position::<T::Balance, T::AssetId>::price_to_balance(initial_price),
			};

			let instance_id = Self::create_and_mint_position_instance(&who)?;

			<Positions<T>>::insert(instance_id, lp_position);

			Self::deposit_event(Event::PositionCreated {
				position_id: instance_id,
				owner: who,
				asset,
				amount,
				shares: amount,
				price: initial_price,
			});

			// Imbalance update
			Self::recalculate_imbalance(&state, BalanceUpdate::Decrease(amount))?;

			// Total hub asset liquidity update
			Self::update_hub_asset_liquidity(
				&BalanceUpdate::Increase(hub_reserve),
				HubAssetIssuanceUpdate::AdjustSupply,
			)?;

			// TVL update
			<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
				*tvl = initial_price
					.checked_mul(
						&Price::checked_from_rational(stable_asset_reserve, stable_asset_hub_reserve)
							.ok_or(ArithmeticError::DivisionByZero)?,
					)
					.and_then(|v| v.checked_mul_int(amount))
					.and_then(|v| v.checked_add(&*tvl))
					.ok_or(ArithmeticError::Overflow)?;
				Ok(())
			})?;

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
		/// add_liquidity adds specified asset amount to pool and in exchange gives the origin
		/// corresponding shares amount in form of NFT at current price.
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
		pub fn add_liquidity(origin: OriginFor<T>, asset: T::AssetId, amount: T::Balance) -> DispatchResult {
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

			Assets::<T>::try_mutate(asset, |maybe_asset| -> DispatchResult {
				let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				let state_changes = calculate_add_liquidity_state_changes::<T>(asset_state, amount, stable_asset)
					.ok_or(ArithmeticError::Overflow)?;

				// New Asset State
				asset_state
					.delta_update(&state_changes.asset)
					.ok_or(ArithmeticError::Overflow)?;

				let hub_reserve_ratio = FixedU128::checked_from_rational(
					asset_state.hub_reserve,
					<HubAssetLiquidity<T>>::get()
						.checked_add(&state_changes.asset.delta_hub_reserve)
						.ok_or(ArithmeticError::Overflow)?,
				)
				.ok_or(ArithmeticError::DivisionByZero)?;

				ensure!(
					hub_reserve_ratio <= Self::asset_weight_cap(),
					Error::<T>::AssetWeightCapExceeded
				);

				let updated_asset_price = asset_state.price();

				// Create LP position with given shares
				let lp_position = Position::<T::Balance, T::AssetId> {
					asset_id: asset,
					amount,
					shares: *state_changes.asset.delta_shares,
					// Note: position needs price after asset state is updated.
					price: Position::<T::Balance, T::AssetId>::price_to_balance(updated_asset_price),
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

				// Token update
				T::Currency::transfer(
					asset,
					&who,
					&Self::protocol_account(),
					*state_changes.asset.delta_reserve,
				)?;

				// Imbalance update
				Self::recalculate_imbalance(asset_state, state_changes.delta_imbalance)?;

				// TVL update
				Self::update_tvl(&state_changes.asset.delta_tvl)?;

				// Total hub asset liquidity update
				Self::update_hub_asset_liquidity(
					&state_changes.asset.delta_hub_reserve,
					HubAssetIssuanceUpdate::AdjustSupply,
				)?;

				// Storage update - asset state
				<Assets<T>>::insert(asset, asset_state);

				Self::deposit_event(Event::LiquidityAdded {
					from: who,
					asset_id: asset,
					amount,
					position_id: instance_id,
				});
				Ok(())
			})
		}

		/// Remove liquidity of asset `asset` in quantity `amount` from Omnipool
		///
		/// remove_liquidity removes specified shares amount from given PositionId (NFT instance).
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
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::NFTHandler::owner(&T::NFTClassId::get(), &position_id) == Some(who.clone()),
				Error::<T>::Forbidden
			);

			let mut position = Positions::<T>::get(position_id).ok_or(Error::<T>::PositionNotFound)?;

			ensure!(position.shares >= amount, Error::<T>::InsufficientShares);

			let stable_asset = Self::stable_asset()?;

			let asset_id = position.asset_id;

			Assets::<T>::try_mutate(asset_id, |maybe_asset| -> DispatchResult {
				let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				let state_changes =
					calculate_remove_liquidity_state_changes::<T>(asset_state, amount, &position, stable_asset)
						.ok_or(ArithmeticError::Overflow)?;

				// New Asset State
				asset_state
					.delta_update(&state_changes.asset)
					.ok_or(ArithmeticError::Overflow)?;

				// Update position state
				position
					.delta_update(
						&state_changes.delta_position_reserve,
						&state_changes.delta_position_shares,
					)
					.ok_or(ArithmeticError::Overflow)?;

				// Token balance updates
				T::Currency::transfer(
					asset_id,
					&Self::protocol_account(),
					&who,
					*state_changes.asset.delta_reserve,
				)?;

				// Imbalance update
				Self::recalculate_imbalance(asset_state, state_changes.delta_imbalance)?;

				// TVL update
				Self::update_tvl(&state_changes.asset.delta_tvl)?;

				// Total Hub asset liquidity
				Self::update_hub_asset_liquidity(
					&state_changes.asset.delta_hub_reserve,
					HubAssetIssuanceUpdate::AdjustSupply,
				)?;

				// LP receives some hub asset
				if state_changes.lp_hub_amount > T::Balance::zero() {
					T::Currency::transfer(
						T::HubAssetId::get(),
						&Self::protocol_account(),
						&who,
						state_changes.lp_hub_amount,
					)?;

					Self::update_hub_asset_liquidity(
						&BalanceUpdate::Decrease(state_changes.lp_hub_amount),
						HubAssetIssuanceUpdate::JustTransfer,
					)?;
				}

				// Storage update - asset state and position
				<Assets<T>>::insert(asset_id, asset_state);

				if position.shares == T::Balance::zero() {
					// All liquidity removed, remove position and burn NFT instance
					<Positions<T>>::remove(position_id);
					T::NFTHandler::burn_from(&T::NFTClassId::get(), &position_id)?;

					Self::deposit_event(Event::PositionDestroyed {
						position_id,
						owner: who.clone(),
					});
				} else {
					<Positions<T>>::insert(position_id, position);
				}

				Self::deposit_event(Event::LiquidityRemoved {
					who,
					position_id,
					asset_id,
					shares_removed: amount,
				});

				Ok(())
			})
		}

		/// Execute a swap of `asset_in` for `asset_out`.
		///
		/// Price is determined by the Omnipool.
		///
		/// Hub asset is traded separately.
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
			amount: T::Balance,
			min_buy_amount: T::Balance,
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

			//Handle selling hub asset separately as math is simplified and asset_in is actually part of asset_out state in this case
			if asset_in == T::HubAssetId::get() {
				return Self::sell_hub_asset(&who, asset_out, amount, min_buy_amount);
			}

			if asset_out == T::HubAssetId::get() {
				// Hub asset is not allowed to be bought,
				// however if it allowed - we cannot call buy_hub_asset because that deals with buying certain amount of Hub asset
				// here we sell asset_in with amount in
				// TODO: ask colin what would be the best way
				return Err(Error::<T>::NotAllowed.into());
			}

			let mut asset_in_state = Assets::<T>::get(asset_in).ok_or(Error::<T>::AssetNotFound)?;
			let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;

			ensure!(
				Self::allow_assets(&asset_in_state, &asset_out_state),
				Error::<T>::NotAllowed
			);

			let current_imbalance = <HubAssetImbalance<T>>::get();

			let state_changes = calculate_sell_state_changes::<T>(
				&asset_in_state,
				&asset_out_state,
				amount,
				Self::asset_fee(),
				Self::protocol_fee(),
				&current_imbalance,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(
				*state_changes.asset_out.delta_reserve >= min_buy_amount,
				Error::<T>::BuyLimitNotReached
			);

			// Pool state update
			asset_in_state
				.delta_update(&state_changes.asset_in)
				.ok_or(ArithmeticError::Overflow)?;
			asset_out_state
				.delta_update(&state_changes.asset_out)
				.ok_or(ArithmeticError::Overflow)?;

			<Assets<T>>::insert(asset_in, asset_in_state);
			<Assets<T>>::insert(asset_out, asset_out_state);

			// Token balances update
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

			// Hub liquidity update - work out difference between in and amount and act accordingly and responsibly, fred!
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

			Self::update_hub_asset_liquidity(&delta_hub_asset, HubAssetIssuanceUpdate::AdjustSupply)?;

			Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

			Self::update_hdx_subpool_hub_asset(state_changes.hdx_hub_amount)?;

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
			amount: T::Balance,
			max_sell_amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				amount >= T::MinimumTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			if asset_out == T::HubAssetId::get() {
				return Self::buy_hub_asset(&who, asset_in, amount, max_sell_amount);
			}

			if asset_in == T::HubAssetId::get() {
				// This is allowed, however what is the math here ?!
				// TODO: ask colin what would be the best way
				// we cannot call sell_hub_asset as that deals amount of hub asset to sell
				// here we have amount of asset out to buy / to swap for hub asset
				return Err(Error::<T>::NotAllowed.into());
			}

			let mut asset_in_state = Assets::<T>::get(asset_in).ok_or(Error::<T>::AssetNotFound)?;
			let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;

			ensure!(
				Self::allow_assets(&asset_in_state, &asset_out_state),
				Error::<T>::NotAllowed
			);

			let current_imbalance = <HubAssetImbalance<T>>::get();

			let state_changes = calculate_buy_state_changes::<T>(
				&asset_in_state,
				&asset_out_state,
				amount,
				Self::asset_fee(),
				Self::protocol_fee(),
				&current_imbalance,
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

			// Pool state update
			asset_in_state
				.delta_update(&state_changes.asset_in)
				.ok_or(ArithmeticError::Overflow)?;
			asset_out_state
				.delta_update(&state_changes.asset_out)
				.ok_or(ArithmeticError::Overflow)?;

			<Assets<T>>::insert(asset_in, asset_in_state);
			<Assets<T>>::insert(asset_out, asset_out_state);

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

			// Hub liquidity update - work out difference between in and amount and act accordingly and responsibly, fred!
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
			Self::update_hub_asset_liquidity(&delta_hub_asset, HubAssetIssuanceUpdate::AdjustSupply)?;

			Self::update_hdx_subpool_hub_asset(state_changes.hdx_hub_amount)?;

			Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

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
		/// Change asset's state to one of `Tradable` states.
		///
		/// Only root can change this state.
		///
		/// Parameters:
		/// - `asset_id`: asset id
		/// - `state`: new state
		///
		/// Emits `TradableStateUpdated` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_tradable_state())]
		#[transactional]
		pub fn set_asset_tradable_state(origin: OriginFor<T>, asset_id: T::AssetId, state: Tradable) -> DispatchResult {
			ensure_root(origin)?;

			Assets::<T>::try_mutate(asset_id, |maybe_asset| -> DispatchResult {
				let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				asset_state.tradable = state.clone();
				Self::deposit_event(Event::TradableStateUpdated { asset_id, state });

				Ok(())
			})
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

	/// Convert protocol fee to FixedU128
	fn protocol_fee() -> Price {
		let fee = T::ProtocolFee::get();
		match fee {
			(_, 0) => FixedU128::zero(),
			(a, b) => FixedU128::from((a, b)),
		}
	}

	/// Convert asset fee to FixedU128
	fn asset_fee() -> Price {
		let fee = T::AssetFee::get();
		match fee {
			(_, 0) => FixedU128::zero(),
			(a, b) => FixedU128::from((a, b)),
		}
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
	fn stable_asset() -> Result<(T::Balance, T::Balance), DispatchError> {
		let stable_asset = <Assets<T>>::get(T::StableCoinAssetId::get()).ok_or(Error::<T>::NoStableAssetInPool)?;
		Ok((stable_asset.reserve, stable_asset.hub_reserve))
	}

	/// Generate an nft instance id and mint NFT into the class and instance.
	#[require_transactional]
	fn create_and_mint_position_instance(owner: &T::AccountId) -> Result<T::PositionInstanceId, DispatchError> {
		<PositionInstanceSequencer<T>>::try_mutate(|current_value| -> Result<T::PositionInstanceId, DispatchError> {
			let next_position_id = *current_value;

			// TODO: think if there is need to embed something helpful into nft instance id ( such as position asset?!).
			let instance_id = T::PositionInstanceId::from(next_position_id);

			T::NFTHandler::mint_into(&T::NFTClassId::get(), &instance_id, owner)?;

			*current_value = current_value.checked_add(1u32).ok_or(ArithmeticError::Overflow)?;

			Ok(instance_id)
		})
	}

	/// Update Hub asset side of HDX subpool annd add given amount to hub_asset_reserve
	fn update_hdx_subpool_hub_asset(hub_asset_amount: T::Balance) -> DispatchResult {
		if hub_asset_amount > T::Balance::zero() {
			let mut native_subpool = Assets::<T>::get(T::NativeAssetId::get()).ok_or(Error::<T>::AssetNotFound)?;
			native_subpool.hub_reserve = native_subpool
				.hub_reserve
				.checked_add(&hub_asset_amount)
				.ok_or(ArithmeticError::Overflow)?;
			<Assets<T>>::insert(T::NativeAssetId::get(), native_subpool);
		}
		Ok(())
	}

	/// Update total hub asset liquidity and write new value to storage.
	/// Update total issueance if AdjustSupply is specified.
	#[require_transactional]
	fn update_hub_asset_liquidity(
		delta_amount: &BalanceUpdate<T::Balance>,
		issuance_update: HubAssetIssuanceUpdate,
	) -> DispatchResult {
		<HubAssetLiquidity<T>>::try_mutate(|liquidity| -> DispatchResult {
			match delta_amount {
				BalanceUpdate::Increase(amount) => {
					*liquidity = liquidity.checked_add(amount).ok_or(ArithmeticError::Overflow)?;
				}
				BalanceUpdate::Decrease(amount) => {
					*liquidity = liquidity.checked_sub(amount).ok_or(ArithmeticError::Underflow)?;
				}
			}
			Ok(())
		})?;

		if issuance_update == HubAssetIssuanceUpdate::AdjustSupply {
			match delta_amount {
				BalanceUpdate::Increase(amount) => {
					T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), *amount)?;
				}
				BalanceUpdate::Decrease(amount) => {
					T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), *amount)?;
				}
			}
		}

		Ok(())
	}

	/// Update imbalance with given delta_imbalance and write new value to storage.
	fn update_imbalance(
		current_imbalance: SimpleImbalance<T::Balance>,
		delta_imbalance: BalanceUpdate<T::Balance>,
	) -> DispatchResult {
		let imbalance = match delta_imbalance {
			BalanceUpdate::Decrease(amount) => current_imbalance.sub(amount).ok_or(ArithmeticError::Overflow)?,
			BalanceUpdate::Increase(amount) => current_imbalance.add(amount).ok_or(ArithmeticError::Overflow)?,
		};
		<HubAssetImbalance<T>>::put(imbalance);

		Ok(())
	}

	/// Recalculate imbalance and call update_imbalance with new imbalance value.
	fn recalculate_imbalance(
		asset_state: &AssetState<T::Balance>,
		delta_amount: BalanceUpdate<T::Balance>,
	) -> DispatchResult {
		let current_imbalance = <HubAssetImbalance<T>>::get();
		let current_hub_asset_liquidity = <HubAssetLiquidity<T>>::get();

		if current_imbalance.value == T::Balance::zero() || current_hub_asset_liquidity == T::Balance::zero() {
			return Ok(());
		}
		let p1 = FixedU128::checked_from_rational(asset_state.hub_reserve, asset_state.reserve)
			.ok_or(ArithmeticError::DivisionByZero)?;
		let p2 = FixedU128::checked_from_rational(current_imbalance.value, current_hub_asset_liquidity)
			.ok_or(ArithmeticError::DivisionByZero)?;
		let p3 = p1.checked_mul(&p2).ok_or(ArithmeticError::Overflow)?;

		let delta_imbalance = p3.checked_mul_int(*delta_amount).ok_or(ArithmeticError::Overflow)?;

		match delta_amount {
			BalanceUpdate::Increase(_) => {
				Self::update_imbalance(current_imbalance, BalanceUpdate::Increase(delta_imbalance))
			}
			BalanceUpdate::Decrease(_) => {
				Self::update_imbalance(current_imbalance, BalanceUpdate::Decrease(delta_imbalance))
			}
		}
	}

	/// Update total tvl balance and check TVL cap if TVL increased.
	#[require_transactional]
	fn update_tvl(delta_tvl: &BalanceUpdate<T::Balance>) -> DispatchResult {
		<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
			match delta_tvl {
				BalanceUpdate::Increase(amount) => {
					*tvl = tvl.checked_add(amount).ok_or(ArithmeticError::Overflow)?;
					ensure!(*tvl <= T::TVLCap::get(), Error::<T>::TVLCapExceeded);
				}
				BalanceUpdate::Decrease(amount) => *tvl = tvl.checked_sub(amount).ok_or(ArithmeticError::Underflow)?,
			}
			Ok(())
		})
	}

	/// Check if assets can be traded
	fn allow_assets(asset_in: &AssetState<T::Balance>, asset_out: &AssetState<T::Balance>) -> bool {
		matches!(
			(&asset_in.tradable, &asset_out.tradable),
			(Tradable::Allowed, Tradable::Allowed)
				| (Tradable::Allowed, Tradable::BuyOnly)
				| (Tradable::SellOnly, Tradable::BuyOnly)
				| (Tradable::SellOnly, Tradable::Allowed)
		)
	}

	/// Swap hub asset for asset_out.
	/// Special handling of sell trade where asset in is Hub Asset.
	fn sell_hub_asset(
		who: &T::AccountId,
		asset_out: T::AssetId,
		amount: T::Balance,
		limit: T::Balance,
	) -> DispatchResult {
		Assets::<T>::try_mutate(asset_out, |maybe_asset| -> DispatchResult {
			let asset_out_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

			ensure!(
				matches!(&asset_out_state.tradable, Tradable::Allowed | Tradable::BuyOnly),
				Error::<T>::NotAllowed
			); // TODO :Add test for this!

			let state_changes = calculate_sell_hub_state_changes::<T>(asset_out_state, amount, Self::asset_fee())
				.ok_or(ArithmeticError::Overflow)?;

			ensure!(
				*state_changes.asset.delta_reserve >= limit,
				Error::<T>::BuyLimitNotReached
			);

			asset_out_state
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

			// Total hub asset liquidity
			Self::update_hub_asset_liquidity(
				&state_changes.asset.delta_hub_reserve,
				HubAssetIssuanceUpdate::JustTransfer,
			)?;

			// Imbalance update
			let current_imbalance = <HubAssetImbalance<T>>::get();
			Self::update_imbalance(current_imbalance, state_changes.delta_imbalance)?;

			<Assets<T>>::insert(asset_out, asset_out_state);

			Self::deposit_event(Event::SellExecuted {
				who: who.clone(),
				asset_in: T::HubAssetId::get(),
				asset_out,
				amount_in: *state_changes.asset.delta_hub_reserve,
				amount_out: *state_changes.asset.delta_reserve,
			});

			Ok(())
		})
	}

	/// Buy hub asset from the pool
	/// Special handling of buy trade where asset out is Hub Asset.
	/// Note: Currently not allowed at all, and not implemented for time being.
	fn buy_hub_asset(
		_who: &T::AccountId,
		_asset_in: T::AssetId,
		_amount: T::Balance,
		_limit: T::Balance,
	) -> DispatchResult {
		// TODO: implement before Hub asset is allowed to be bought
		Err(Error::<T>::NotAllowed.into())
	}
}
