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

extern crate core;

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::sp_runtime::FixedPointOperand;
use frame_support::PalletId;
use frame_support::{ensure, transactional};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned};
use sp_runtime::traits::{CheckedAdd, CheckedMul, CheckedSub, Zero};
use sp_std::prelude::*;
use std::cmp::Ordering;
use std::ops::{Add, Sub};

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use hydradx_traits::Registry;
use orml_traits::MultiCurrency;
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128};

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
use crate::types::{AssetState, BalanceUpdate, Price, SimpleImbalance};
use math::calculate_sell_state_changes;
pub use pallet::*;
pub use weights::WeightInfo;

#[macro_export]
macro_rules! ensure_asset_in_pool {
	( $x:expr, $y:expr $(,)? ) => {{
		if !Assets::<T>::contains_key($x) {
			return Err($y.into());
		}
	}};
}

type NFTClassIdOf<T> = <<T as Config>::NFTHandler as Inspect<<T as frame_system::Config>::AccountId>>::ClassId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::math::{
		calculate_add_liquidity_state_changes, calculate_buy_state_changes, calculate_remove_liquidity_state_changes,
	};
	use crate::types::{AssetState, Position, Price, SimpleImbalance};
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::FixedPointNumber;

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
			// TODO: the following from/into is due to use of FixedU128, which internally uses u128.
			// might think of better way or use directly u128 instead as there is not much choice here anyway
			// Or make fixed point number generic too ?!
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
		type AddTokenOrigin: EnsureOrigin<Self::Origin, Success = Option<Self::AccountId>>;

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
		type AssetWeightCap: Get<Self::Balance>;

		/// TVL cap
		#[pallet::constant]
		type TVLCap: Get<Self::Balance>;

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
		NoStableCoinInPool,
		/// No native asset in the pool yet.
		NoNativeAssetInPool,
		/// Adding token as protocol ( root ), token balance has not been updated prior to add token.
		MissingBalance,
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
		/// Math overflow
		Overflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add new token to omnipool in quantity `amount` at price `initial_price`
		///
		/// First added assets must be:
		/// - preferred stable coin asset set as `StableCoinAssetId` pallet parameter
		/// - native asset
		///
		/// `add_token` returns `NoStableCoinInPool` error if stable asset is missing
		/// `add_token` returns `NoNativeAssetInPool` error if native asset is missing
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
			let account = T::AddTokenOrigin::ensure_origin(origin)?;

			ensure!(!Assets::<T>::contains_key(asset), Error::<T>::AssetAlreadyAdded);

			ensure!(T::AssetRegistry::exists(asset), Error::<T>::AssetNotRegistered);

			// Retrieve stable asset and native asset details first - we fail early if they are not yet in the pool.
			let (stable_asset_reserve, stable_asset_hub_reserve) = if asset != T::StableCoinAssetId::get() {
				// Ensure first that Native asset and Hub asset is already in pool
				if asset != T::NativeAssetId::get() {
					ensure_asset_in_pool!(T::NativeAssetId::get(), Error::<T>::NoNativeAssetInPool);
				}
				Self::stable_asset()?
			} else {
				// Trying to add preferred stable asset.
				// This can happen only once , since it is first token to add to the pool.
				(
					amount,
					initial_price.checked_mul_int(amount).ok_or(Error::<T>::Overflow)?,
				)
			};

			let hub_reserve = initial_price.checked_mul_int(amount).ok_or(Error::<T>::Overflow)?;

			// Initial stale of asset
			let state = AssetState::<T::Balance> {
				reserve: amount,
				hub_reserve,
				shares: amount,
				protocol_shares: amount,
				tvl: amount,
			};

			// if root ( None ), it means protocol, so no transfer done assuming asset is already in the protocol account
			if let Some(who) = account {
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
			} else {
				// Ensure that it has been transferred to protocol account by other means
				ensure!(
					T::Currency::free_balance(asset, &Self::protocol_account()) >= amount,
					Error::<T>::MissingBalance
				);
			}

			// Mint matching Hub asset
			T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), hub_reserve)?;

			// Imbalance update
			Self::recalculate_imbalance(&state, BalanceUpdate::Decrease(amount))?;

			// Total hub asset liquidity update
			Self::update_hub_asset_liquidity(&BalanceUpdate::Increase(hub_reserve))?;

			// TVL update
			if stable_asset_reserve != T::Balance::zero() && stable_asset_hub_reserve != T::Balance::zero() {
				<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
					*tvl = initial_price
						.checked_mul(&Price::from((stable_asset_reserve, stable_asset_hub_reserve)))
						.and_then(|v| v.checked_mul_int(amount))
						.and_then(|v| v.checked_add(&*tvl))
						.ok_or(Error::<T>::Overflow)?;
					Ok(())
				})?;
			}

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
				T::Currency::free_balance(asset, &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			let mut asset_state = Assets::<T>::get(asset).ok_or(Error::<T>::AssetNotFound)?;

			let state_changes =
				calculate_add_liquidity_state_changes::<T>(&asset_state, amount).ok_or(Error::<T>::Overflow)?;

			// New Asset State
			asset_state
				.delta_update(&state_changes.asset)
				.ok_or(Error::<T>::Overflow)?;

			ensure!(
				asset_state.hub_reserve <= T::AssetWeightCap::get(), // TODO: add test when weight cap is exceeded
				Error::<T>::AssetWeightCapExceeded
			);

			// Create LP position
			let lp_position = Position::<T::Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: *state_changes.asset.delta_shares,
				price: Position::<T::Balance, T::AssetId>::price_to_balance(asset_state.price()),
			};

			let instance_id = Self::create_and_mint_position_instance(&who)?;

			<Positions<T>>::insert(instance_id, lp_position);

			Self::deposit_event(Event::PositionCreated {
				position_id: instance_id,
				owner: who.clone(),
				asset,
				amount,
				shares: *state_changes.asset.delta_shares,
				price: asset_state.price(),
			});

			// Token update
			T::Currency::transfer(
				asset,
				&who,
				&Self::protocol_account(),
				*state_changes.asset.delta_reserve,
			)?;
			T::Currency::deposit(
				T::HubAssetId::get(),
				&Self::protocol_account(),
				*state_changes.asset.delta_hub_reserve,
			)?;

			// Imbalance update
			Self::recalculate_imbalance(&asset_state, state_changes.delta_imbalance)?;

			// TVL update
			Self::update_tvl(&mut asset_state)?;

			// Total hub asset liquidity update
			Self::update_hub_asset_liquidity(&state_changes.asset.delta_hub_reserve)?;

			// Storage update - asset state
			<Assets<T>>::insert(asset, asset_state);

			Self::deposit_event(Event::LiquidityAdded {
				from: who,
				asset_id: asset,
				amount,
				position_id: instance_id,
			});

			Ok(())
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

			let asset_id = position.asset_id;

			let mut asset_state = Assets::<T>::get(asset_id).ok_or(Error::<T>::AssetNotFound)?;

			let state_changes =
				calculate_remove_liquidity_state_changes::<T>(&asset_state, amount, position.fixed_price())
					.ok_or(Error::<T>::Overflow)?;

			// New Asset State
			asset_state
				.delta_update(&state_changes.asset)
				.ok_or(Error::<T>::Overflow)?;

			// Update position state
			position
				.delta_update(&state_changes.delta_position_reserve, &state_changes.asset.delta_shares)
				.ok_or(Error::<T>::Overflow)?;

			// Token balance updates
			T::Currency::transfer(
				asset_id,
				&Self::protocol_account(),
				&who,
				*state_changes.asset.delta_reserve,
			)?;
			T::Currency::withdraw(
				T::HubAssetId::get(),
				&Self::protocol_account(),
				*state_changes.asset.delta_hub_reserve,
			)?;
			// LP receives some hub asset, if 0 - it is noop.
			T::Currency::transfer(
				T::HubAssetId::get(),
				&Self::protocol_account(),
				&who,
				state_changes.lp_hub_amount,
			)?;

			// Imbalance update
			Self::recalculate_imbalance(&asset_state, state_changes.delta_imbalance)?;

			// TVL update
			Self::update_tvl(&mut asset_state)?;

			// Total Hub asset liquidity
			Self::update_hub_asset_liquidity(
				&state_changes
					.asset
					.delta_hub_reserve
					.diff(BalanceUpdate::Decrease(state_changes.lp_hub_amount))
					.ok_or(Error::<T>::Overflow)?,
			)?;

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
		/// - `min_limit`: Minimum amount required to receive
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
			min_limit: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::allow_assets(asset_in, asset_out), Error::<T>::NotAllowed);

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			//Handle selling hub asset separately as math is simplified and asset_in is actually part of asset_out state in this case
			if asset_in == T::HubAssetId::get() {
				return Self::sell_hub_asset(&who, asset_out, amount, min_limit);
			}

			let mut asset_in_state = Assets::<T>::get(asset_in).ok_or(Error::<T>::AssetNotFound)?;
			let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;
			let current_imbalance = <HubAssetImbalance<T>>::get();

			let state_changes = calculate_sell_state_changes::<T>(
				&asset_in_state,
				&asset_out_state,
				amount,
				Self::asset_fee(),
				Self::protocol_fee(),
				&current_imbalance,
			)
			.ok_or(Error::<T>::Overflow)?;

			ensure!(
				*state_changes.asset_out.delta_reserve >= min_limit,
				Error::<T>::BuyLimitNotReached
			);

			// Pool state update
			asset_in_state
				.delta_update(&state_changes.asset_in)
				.ok_or(Error::<T>::Overflow)?;
			asset_out_state
				.delta_update(&state_changes.asset_out)
				.ok_or(Error::<T>::Overflow)?;

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
				.diff(state_changes.asset_out.delta_hub_reserve)
				.ok_or(Error::<T>::Overflow)?;
			Self::update_hub_asset_liquidity(&delta_hub_asset)?;

			//Burn or mint the hub asset amount difference
			match delta_hub_asset {
				BalanceUpdate::Increase(amount) => {
					T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				}
				BalanceUpdate::Decrease(amount) => {
					T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				}
			}

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
		/// - `max_limit`: Maximum amount to be sold.
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
			max_limit: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::allow_assets(asset_in, asset_out), Error::<T>::NotAllowed);

			// TODO: handle buy hub asset separately.
			// Note: hub asset is not allowed to be bought at the moment.
			// but when it does - it needs to be handled separately

			let mut asset_in_state = Assets::<T>::get(asset_in).ok_or(Error::<T>::AssetNotFound)?;
			let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;
			let current_imbalance = <HubAssetImbalance<T>>::get();

			let state_changes = calculate_buy_state_changes::<T>(
				&asset_in_state,
				&asset_out_state,
				amount,
				Self::asset_fee(),
				Self::protocol_fee(),
				&current_imbalance,
			)
			.ok_or(Error::<T>::Overflow)?;

			ensure!(
				T::Currency::free_balance(asset_in, &who) >= *state_changes.asset_in.delta_reserve,
				Error::<T>::InsufficientBalance
			);

			ensure!(
				*state_changes.asset_in.delta_reserve <= max_limit,
				Error::<T>::SellLimitExceeded
			);

			// Pool state update
			asset_in_state
				.delta_update(&state_changes.asset_in)
				.ok_or(Error::<T>::Overflow)?;
			asset_out_state
				.delta_update(&state_changes.asset_out)
				.ok_or(Error::<T>::Overflow)?;

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
				.diff(state_changes.asset_out.delta_hub_reserve)
				.ok_or(Error::<T>::Overflow)?;
			Self::update_hub_asset_liquidity(&delta_hub_asset)?;

			//Burn or mint the hub asset amount difference
			match delta_hub_asset {
				BalanceUpdate::Increase(amount) => {
					T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				}
				BalanceUpdate::Decrease(amount) => {
					T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				}
			}

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

	/// Retrieve stable asset detail from the pool.
	/// Return NoStableCoinInPool if stable asset is not yet in the pool.
	fn stable_asset() -> Result<(T::Balance, T::Balance), DispatchError> {
		let stable_asset = <Assets<T>>::get(T::StableCoinAssetId::get()).ok_or(Error::<T>::NoStableCoinInPool)?;
		Ok((stable_asset.reserve, stable_asset.hub_reserve))
	}

	/// Generate an nft instance id and mint NFT into the class and instance.
	fn create_and_mint_position_instance(owner: &T::AccountId) -> Result<T::PositionInstanceId, DispatchError> {
		<PositionInstanceSequencer<T>>::try_mutate(|current_value| -> Result<T::PositionInstanceId, DispatchError> {
			let next_position_id = *current_value;

			// TODO: generate cool looking instance id, see liquidity mining
			let instance_id = T::PositionInstanceId::from(next_position_id);

			T::NFTHandler::mint_into(&T::NFTClassId::get(), &instance_id, owner)?;

			*current_value = current_value.checked_add(1u32).ok_or(Error::<T>::Overflow)?;

			Ok(instance_id)
		})
	}

	fn update_hdx_subpool_hub_asset(hub_asset_amount: T::Balance) -> DispatchResult {
		if hub_asset_amount > T::Balance::zero() {
			let mut native_subpool = Assets::<T>::get(T::NativeAssetId::get()).ok_or(Error::<T>::AssetNotFound)?;
			native_subpool.hub_reserve = native_subpool
				.hub_reserve
				.checked_add(&hub_asset_amount)
				.ok_or(Error::<T>::Overflow)?;
			<Assets<T>>::insert(T::NativeAssetId::get(), native_subpool);
		}
		Ok(())
	}

	/// Updates total hub asset liquidity. It either burn or mint some based on the diff of in and out.
	fn update_hub_asset_liquidity(delta_amount: &BalanceUpdate<T::Balance>) -> DispatchResult {
		<HubAssetLiquidity<T>>::try_mutate(|liquidity| -> DispatchResult {
			match delta_amount {
				BalanceUpdate::Increase(amount) => {
					*liquidity = liquidity.checked_add(amount).ok_or(Error::<T>::Overflow)?;
				}
				BalanceUpdate::Decrease(amount) => {
					*liquidity = liquidity.checked_sub(amount).ok_or(Error::<T>::Overflow)?;
				}
			}
			Ok(())
		})
	}

	fn update_imbalance(
		current_imbalance: SimpleImbalance<T::Balance>,
		delta_imbalance: BalanceUpdate<T::Balance>,
	) -> DispatchResult {
		let imbalance = match delta_imbalance {
			BalanceUpdate::Decrease(amount) => current_imbalance.sub(amount).ok_or(Error::<T>::Overflow)?,
			BalanceUpdate::Increase(amount) => current_imbalance.add(amount).ok_or(Error::<T>::Overflow)?,
		};
		<HubAssetImbalance<T>>::put(imbalance);

		Ok(())
	}

	fn recalculate_imbalance(
		asset_state: &AssetState<T::Balance>,
		delta_amount: BalanceUpdate<T::Balance>,
	) -> DispatchResult {
		let current_imbalance = <HubAssetImbalance<T>>::get();
		let current_hub_asset_liquidity = <HubAssetLiquidity<T>>::get();

		if current_imbalance.value != T::Balance::zero() && current_hub_asset_liquidity != T::Balance::zero() {
			// if any is 0, the delta is 0 too.

			let p1 = FixedU128::from((asset_state.hub_reserve, asset_state.reserve));
			let p2 = FixedU128::from((current_imbalance.value, current_hub_asset_liquidity));
			let p3 = p1.checked_mul(&p2).ok_or(Error::<T>::Overflow)?;

			let delta_imbalance = p3.checked_mul_int(*delta_amount).ok_or(Error::<T>::Overflow)?;

			match delta_amount {
				BalanceUpdate::Increase(_) => {
					return Self::update_imbalance(current_imbalance, BalanceUpdate::Increase(delta_imbalance));
				}
				BalanceUpdate::Decrease(_) => {
					return Self::update_imbalance(current_imbalance, BalanceUpdate::Decrease(delta_imbalance));
				}
			};
		}

		Ok(())
	}

	fn update_tvl(asset_state: &mut AssetState<T::Balance>) -> DispatchResult {
		let (stable_asset_reserve, stable_asset_hub_reserve) = Self::stable_asset()?;

		if stable_asset_reserve != T::Balance::zero() && stable_asset_hub_reserve != T::Balance::zero() {
			<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
				let adjusted_asset_tvl = Price::from((stable_asset_reserve, stable_asset_hub_reserve))
					.checked_mul_int(asset_state.hub_reserve)
					.ok_or(Error::<T>::Overflow)?;

				// Handle decrease or increase accordingly
				match adjusted_asset_tvl.cmp(&asset_state.tvl) {
					Ordering::Greater => {
						let delta_tvl = adjusted_asset_tvl
							.checked_sub(&asset_state.tvl)
							.ok_or(Error::<T>::Overflow)?;
						*tvl = tvl.checked_add(&delta_tvl).ok_or(Error::<T>::Overflow)?;

						ensure!(*tvl <= T::TVLCap::get(), Error::<T>::TVLCapExceeded);

						asset_state.tvl = asset_state.tvl.checked_add(&delta_tvl).ok_or(Error::<T>::Overflow)?;
					}
					Ordering::Less => {
						// no need to check the cap because we decreasing tvl
						let delta_tvl = asset_state
							.tvl
							.checked_sub(&adjusted_asset_tvl)
							.ok_or(Error::<T>::Overflow)?;

						// If for some reason, delta_tvl is > total tvl - it is an error, we have some math wrong somewhere
						*tvl = tvl.checked_sub(&delta_tvl).ok_or(Error::<T>::Overflow)?;
						asset_state.tvl = asset_state.tvl.checked_sub(&delta_tvl).ok_or(Error::<T>::Overflow)?;
					}
					Ordering::Equal => {
						// nothing to do
					}
				}
				Ok(())
			})
		} else {
			Ok(())
		}
	}

	/// Check if assets can be traded
	fn allow_assets(_asset_in: T::AssetId, asset_out: T::AssetId) -> bool {
		// TODO: use flag for asset , stored in asset state to manage whether it can be traded
		// Or use a list and preload in on_init
		// probably needs to be called with asset state already retrieved
		if asset_out == T::HubAssetId::get() {
			// Hub asset is not allowed to be bought
			return false;
		}
		true
	}

	/// Swap hub asset for asset_out.
	fn sell_hub_asset(
		who: &T::AccountId,
		asset_out: T::AssetId,
		amount: T::Balance,
		limit: T::Balance,
	) -> DispatchResult {
		let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;

		let state_changes = calculate_sell_hub_state_changes::<T>(&asset_out_state, amount, Self::asset_fee())
			.ok_or(Error::<T>::Overflow)?;

		ensure!(
			*state_changes.asset.delta_reserve >= limit,
			Error::<T>::BuyLimitNotReached
		);

		asset_out_state
			.delta_update(&state_changes.asset)
			.ok_or(Error::<T>::Overflow)?;

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

		// Fee accounting and imbalance
		let current_imbalance = <HubAssetImbalance<T>>::get();

		// Total hub asset liquidity
		Self::update_hub_asset_liquidity(&state_changes.asset.delta_hub_reserve)?;

		// Imbalance update
		let imbalance = current_imbalance
			.sub(*state_changes.delta_imbalance)
			.ok_or(Error::<T>::Overflow)?;
		<HubAssetImbalance<T>>::put(imbalance);

		<Assets<T>>::insert(asset_out, asset_out_state);

		Self::deposit_event(Event::SellExecuted {
			who: who.clone(),
			asset_in: T::HubAssetId::get(),
			asset_out,
			amount_in: *state_changes.asset.delta_hub_reserve,
			amount_out: *state_changes.asset.delta_reserve,
		});

		Ok(())
	}
}
