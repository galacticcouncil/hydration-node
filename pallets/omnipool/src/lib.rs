// This file is part of HydraDX.

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
use frame_support::sp_runtime::FixedPointOperand;
use frame_support::PalletId;
use frame_support::{ensure, transactional};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned};
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero};
use sp_std::prelude::*;
use std::cmp::Ordering;

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

mod types;
pub mod weights;

use crate::types::{PositionId, Price};
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
	use crate::types::{AssetState, Position, PositionId, Price, SimpleImbalance};
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::{FixedPointNumber, FixedU128};

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
			+ TypeInfo
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
	/// Total TVL
	pub(super) type TotalTVL<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::storage]
	/// Total amount of hub asset reserve. It equals to sum of hub_reserve for each asset in omnipool
	pub(super) type HubAssetLiquidity<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::storage]
	/// LP positions
	pub(super) type Positions<T: Config> =
		StorageMap<_, Blake2_128Concat, PositionId<T::PositionInstanceId>, Position<T::Balance, T::AssetId>>;

	#[pallet::storage]
	/// Position IDs sequencer
	pub(super) type PositionInstanceSequencer<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		TokenAdded(T::AssetId, T::Balance, Price),
		LiquidityAdded(T::AccountId, T::AssetId, T::Balance, T::PositionInstanceId),
		LiquidityRemoved(T::AccountId, T::PositionInstanceId, T::Balance),
		SellExecuted(T::AccountId, T::AssetId, T::AssetId, T::Balance, T::Balance),
		BuyExecuted(T::AccountId, T::AssetId, T::AssetId, T::Balance, T::Balance),
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq))]
	pub enum Error<T> {
		/// Asset is already in omnipool
		AssetAlreadyAdded,
		/// Asset is not in omnipool
		AssetNotFound,
		/// No stable asset in the pool
		NoStableCoinInPool,
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
		///
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
					ensure_asset_in_pool!(T::NativeAssetId::get(), Error::<T>::AssetNotFound);
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

			<Assets<T>>::insert(asset, state);

			// if root ( None ), it means protocol, so no transfer done assuming asset is already in the protocol account
			if let Some(who) = account {
				T::Currency::transfer(asset, &who, &Self::protocol_account(), amount)?;
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
			let mut current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity = <HubAssetLiquidity<T>>::get();

			if current_imbalance.value != T::Balance::zero() && current_hub_asset_liquidity != T::Balance::zero() {
				// if any is 0, the delta is 0 too.

				let delta_imbalance = initial_price
					.checked_mul(&FixedU128::from((current_imbalance.value, current_hub_asset_liquidity)))
					.and_then(|v| v.checked_mul_int(amount))
					.ok_or(Error::<T>::Overflow)?;

				current_imbalance.add::<T>(delta_imbalance)?;

				<HubAssetImbalance<T>>::put(current_imbalance);
			}

			// Total hub asset liquidity update
			// Note: must be done after imbalance since it requires current value before update
			Self::increase_hub_asset_liquidity(hub_reserve)?;

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

			Self::deposit_event(Event::TokenAdded(asset, amount, initial_price));

			Ok(())
		}

		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity())]
		#[transactional]
		pub fn add_liquidity(origin: OriginFor<T>, asset: T::AssetId, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut asset_state = Assets::<T>::get(asset).ok_or(Error::<T>::AssetNotFound)?;

			let current_shares = asset_state.shares;
			let current_reserve = asset_state.reserve;
			let current_hub_reserve = asset_state.hub_reserve;
			let current_tvl = asset_state.tvl;

			let new_shares = FixedU128::from((current_shares, current_reserve))
				.checked_mul_int(current_reserve.checked_add(&amount).ok_or(Error::<T>::Overflow)?)
				.ok_or(Error::<T>::Overflow)?;

			let current_price = Price::from((asset_state.hub_reserve, asset_state.reserve));

			let delta_q = current_price.checked_mul_int(amount).ok_or(Error::<T>::Overflow)?;

			let new_hub_reserve = asset_state
				.hub_reserve
				.checked_add(&delta_q)
				.ok_or(Error::<T>::Overflow)?;

			ensure!(
				new_hub_reserve <= T::AssetWeightCap::get(),
				Error::<T>::AssetWeightCapExceeded
			);

			// New Asset State
			asset_state.reserve = current_reserve.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
			asset_state.shares = new_shares;
			asset_state.hub_reserve = new_hub_reserve;

			let new_price = Price::from((asset_state.hub_reserve, asset_state.reserve));

			// Create LP position
			let lp_position = Position::<T::Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: new_shares.checked_sub(&current_shares).ok_or(Error::<T>::Overflow)?,
				price: Position::<T::Balance, T::AssetId>::price_to_balance(new_price),
			};

			let lp_position_id = Self::create_and_mint_position_instance(&who)?;

			let instance_id = lp_position_id.0;

			<Positions<T>>::insert(lp_position_id, lp_position);

			// Token update
			T::Currency::transfer(asset, &who, &Self::protocol_account(), amount)?;
			T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), delta_q)?;

			// Imbalance update
			let mut current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity = <HubAssetLiquidity<T>>::get();

			if current_imbalance.value != T::Balance::zero() && current_hub_asset_liquidity != T::Balance::zero() {
				// if any is 0, the delta is 0 too.

				let p1 = Price::from((current_reserve, current_hub_reserve));
				let p2 = Price::from((current_imbalance.value, current_hub_asset_liquidity));

				let delta_imbalance = p1
					.checked_mul(&p2)
					.and_then(|v| v.checked_mul_int(amount))
					.ok_or(Error::<T>::Overflow)?;

				current_imbalance.add::<T>(delta_imbalance)?;

				<HubAssetImbalance<T>>::put(current_imbalance);
			}

			// TVL update
			let (stable_asset_reserve, stable_asset_hub_reserve) = Self::stable_asset()?;

			if stable_asset_reserve != T::Balance::zero() && stable_asset_hub_reserve != T::Balance::zero() {
				<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
					let hr = Price::from((stable_asset_reserve, stable_asset_hub_reserve))
						.checked_mul_int(new_hub_reserve)
						.ok_or(Error::<T>::Overflow)?;

					match hr.cmp(&current_tvl) {
						Ordering::Greater => {
							let delta_tvl = hr.checked_sub(&current_tvl).ok_or(Error::<T>::Overflow)?;
							*tvl = tvl.checked_add(&delta_tvl).ok_or(Error::<T>::Overflow)?;

							ensure!(*tvl <= T::TVLCap::get(), Error::<T>::TVLCapExceeded);

							asset_state.tvl = hr;
						}
						Ordering::Less => {
							// no need to check the cap because we decreasing tvl
							let delta_tvl = current_tvl.checked_sub(&hr).ok_or(Error::<T>::Overflow)?;
							// TODO: Q: colin: can delta tvl here be > total tvl? probably not, max equal?
							// if not, isit safe to return error is such case happen, which would mean
							// we have some math somewhere wrong
							*tvl = tvl.checked_sub(&delta_tvl).ok_or(Error::<T>::Overflow)?;
							asset_state.tvl = hr;
						}
						Ordering::Equal => {
							// nothing to do
						}
					}

					Ok(())
				})?;
			}

			<Assets<T>>::insert(asset, asset_state);

			// Total hub asset liquidity update
			// Note: must be done after imbalance since it requires current value before update
			Self::increase_hub_asset_liquidity(delta_q)?;

			Self::deposit_event(Event::LiquidityAdded(who, asset, amount, instance_id));

			Ok(())
		}

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

			let mut position = Positions::<T>::get(PositionId(position_id)).ok_or(Error::<T>::PositionNotFound)?;

			ensure!(position.shares >= amount, Error::<T>::InsufficientShares);

			let mut asset_state = Assets::<T>::get(position.asset_id).ok_or(Error::<T>::AssetNotFound)?;

			let current_shares = asset_state.shares;
			let current_reserve = asset_state.reserve;
			let current_hub_reserve = asset_state.hub_reserve;

			let current_price = FixedU128::from((current_hub_reserve, current_reserve));

			let position_price = position.fixed_price();

			let delta_b = if current_price < position_price {
				let sum = current_price.checked_add(&position_price).ok_or(Error::<T>::Overflow)?;
				let sub = position_price.checked_sub(&current_price).ok_or(Error::<T>::Overflow)?;

				sub.checked_div(&sum)
					.and_then(|v| v.checked_mul_int(amount))
					.ok_or(Error::<T>::Overflow)?
			} else {
				T::Balance::zero()
			};

			let delta_s = if delta_b != T::Balance::zero() {
				//TODO: Q: colin: can delta_b > amount
				amount.checked_sub(&delta_b).ok_or(Error::<T>::Overflow)?
			} else {
				amount
			};

			let delta_r = FixedU128::from((current_reserve, current_shares))
				.checked_mul_int(delta_s)
				.ok_or(Error::<T>::Overflow)?;

			let delta_q = FixedU128::from((delta_r, current_reserve))
				.checked_mul_int(current_hub_reserve)
				.ok_or(Error::<T>::Overflow)?;

			if current_price > position_price {
				// TODO: send some hub asset, fred!
				// LP receives some hub asset
			}

			// Asset state update
			asset_state.protocol_shares = asset_state
				.protocol_shares
				.checked_sub(&delta_b)
				.ok_or(Error::<T>::Overflow)?;

			asset_state.shares = current_shares.checked_sub(&delta_s).ok_or(Error::<T>::Overflow)?;
			asset_state.reserve = current_reserve.checked_sub(&delta_r).ok_or(Error::<T>::Overflow)?;

			// LRNA update
			asset_state.hub_reserve = current_hub_reserve.checked_sub(&delta_q).ok_or(Error::<T>::Overflow)?;

			Self::decrease_hub_asset_liquidity(delta_q)?;

			// Update position shares
			position.amount = position.amount.checked_sub(&delta_r).ok_or(Error::<T>::Overflow)?; // TODO: Q: colin: can delta_r > amount ?
			position.shares = position.shares.checked_sub(&amount).ok_or(Error::<T>::Overflow)?;

			// Token balance updates
			T::Currency::transfer(position.asset_id, &Self::protocol_account(), &who, delta_r)?;

			// Burn LRNA
			T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), delta_q)?;

			Self::deposit_event(Event::LiquidityRemoved(who, position_id, amount));

			// Store updated asset state and position
			<Assets<T>>::insert(position.asset_id, asset_state);

			if position.shares == T::Balance::zero() {
				// All liquidity romoved, remove position and burn NFT instance
				<Positions<T>>::remove(PositionId(position_id));
				T::NFTHandler::burn_from(&T::NFTClassId::get(), &position_id)?;
			} else {
				<Positions<T>>::insert(PositionId(position_id), position);
			}

			// Imbalance update
			// TODO:

			// TVL update
			// TODO:

			Ok(())
		}

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

			//Handle selling hub asset separately as math is simplified and asset_in is actually part of asset_out state in this case
			if asset_in == T::HubAssetId::get() {
				return Self::sell_hub_asset(&who, asset_out, amount, min_limit);
			}

			let mut asset_in_state = Assets::<T>::get(asset_in).ok_or(Error::<T>::AssetNotFound)?;
			let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;

			let delta_q_in = FixedU128::from((
				amount,
				(asset_in_state
					.reserve
					.checked_add(&amount)
					.ok_or(Error::<T>::Overflow)?),
			))
			.checked_mul_int(asset_in_state.hub_reserve)
			.ok_or(Error::<T>::Overflow)?;

			let fee_p = Price::from(1)
				.checked_sub(&Self::protocol_fee())
				.ok_or(Error::<T>::Overflow)?;

			let delta_q_out = fee_p.checked_mul_int(delta_q_in).ok_or(Error::<T>::Overflow)?;

			let fee_a = Price::from(1)
				.checked_sub(&Self::asset_fee())
				.ok_or(Error::<T>::Overflow)?;

			let out_hub_reserve = asset_out_state
				.hub_reserve
				.checked_add(&delta_q_out)
				.ok_or(Error::<T>::Overflow)?;

			let delta_r_out = FixedU128::from((delta_q_out, out_hub_reserve))
				.checked_mul(&fee_a)
				.and_then(|v| v.checked_mul_int(asset_out_state.reserve))
				.ok_or(Error::<T>::Overflow)?;

			ensure!(delta_r_out >= min_limit, Error::<T>::BuyLimitNotReached);

			// Pool state update
			asset_in_state.reserve = asset_in_state
				.reserve
				.checked_add(&amount)
				.ok_or(Error::<T>::Overflow)?;
			asset_in_state.hub_reserve = asset_in_state
				.hub_reserve
				.checked_sub(&delta_q_in)
				.ok_or(Error::<T>::Overflow)?;
			asset_out_state.reserve = asset_out_state
				.reserve
				.checked_sub(&delta_r_out)
				.ok_or(Error::<T>::Overflow)?;
			asset_out_state.hub_reserve = asset_out_state
				.hub_reserve
				.checked_add(&delta_q_out)
				.ok_or(Error::<T>::Overflow)?;

			<Assets<T>>::insert(asset_in, asset_in_state);
			<Assets<T>>::insert(asset_out, asset_out_state);

			// Token balances update
			T::Currency::transfer(asset_in, &who, &Self::protocol_account(), amount)?;
			T::Currency::transfer(asset_out, &Self::protocol_account(), &who, delta_r_out)?;

			// Hub liquidity update
			Self::update_hub_asset_liquidity(delta_q_in, delta_q_out)?;

			// TVL update
			// TODO: waiting for update from wiser people!

			// Imbalance update
			// TODO: waiting for update from wiser people!

			Self::deposit_event(Event::SellExecuted(who, asset_in, asset_out, amount, delta_r_out));

			Ok(())
		}

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

			// Positive
			let fee_asset = FixedU128::from(1)
				.checked_sub(&Self::asset_fee())
				.ok_or(Error::<T>::Overflow)?;
			let fee_protocol = FixedU128::from(1)
				.checked_sub(&Self::protocol_fee())
				.ok_or(Error::<T>::Overflow)?;

			let q_out_part = FixedU128::from((
				amount,
				fee_asset
					.checked_mul_int(asset_out_state.reserve)
					.and_then(|v| v.checked_sub(&amount))
					.ok_or(Error::<T>::Overflow)?,
			));

			let delta_q_out = q_out_part
				.checked_mul_int(asset_out_state.hub_reserve)
				.ok_or(Error::<T>::Overflow)?;

			// Negative
			let delta_q_in: T::Balance = FixedU128::from_inner(delta_q_out.into())
				.checked_div(&fee_protocol)
				.ok_or(Error::<T>::Overflow)?
				.into_inner()
				.into();

			// Positive
			let delta_r_in = FixedU128::from((
				delta_q_in,
				asset_in_state
					.hub_reserve
					.checked_add(&delta_q_in)
					.ok_or(Error::<T>::Overflow)?,
			))
			.checked_mul_int(asset_in_state.reserve)
			.ok_or(Error::<T>::Overflow)?;

			ensure!(delta_r_in <= max_limit, Error::<T>::SellLimitExceeded);

			// Pool state update
			asset_in_state.reserve = asset_in_state
				.reserve
				.checked_add(&delta_r_in)
				.ok_or(Error::<T>::Overflow)?;
			asset_in_state.hub_reserve = asset_in_state
				.hub_reserve
				.checked_sub(&delta_q_in)
				.ok_or(Error::<T>::Overflow)?;

			asset_out_state.reserve = asset_out_state
				.reserve
				.checked_sub(&amount)
				.ok_or(Error::<T>::Overflow)?;
			asset_out_state.hub_reserve = asset_out_state
				.hub_reserve
				.checked_add(&delta_q_out)
				.ok_or(Error::<T>::Overflow)?;

			<Assets<T>>::insert(asset_in, asset_in_state);
			<Assets<T>>::insert(asset_out, asset_out_state);

			// Token balances update
			T::Currency::transfer(asset_in, &who, &Self::protocol_account(), delta_r_in)?;
			T::Currency::transfer(asset_out, &Self::protocol_account(), &who, amount)?;

			// Hub liquidity update
			Self::update_hub_asset_liquidity(delta_q_in, delta_q_out)?;

			// TVL update
			// TODO: waiting for update from wiser people!

			// Imbalance update
			// TODO: waiting for update from wiser people!

			Self::deposit_event(Event::BuyExecuted(who, asset_in, asset_out, amount, delta_r_in));

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	fn protocol_account() -> T::AccountId {
		PalletId(*b"omnipool").into_account()
	}

	fn protocol_fee() -> Price {
		let fee = T::ProtocolFee::get();
		match fee {
			(_, 0) => FixedU128::zero(),
			(a, b) => FixedU128::from((a, b)),
		}
	}

	fn asset_fee() -> Price {
		let fee = T::AssetFee::get();
		match fee {
			(_, 0) => FixedU128::zero(),
			(a, b) => FixedU128::from((a, b)),
		}
	}

	fn stable_asset() -> Result<(T::Balance, T::Balance), DispatchError> {
		let stable_asset = <Assets<T>>::get(T::StableCoinAssetId::get()).ok_or(Error::<T>::NoStableCoinInPool)?;
		Ok((stable_asset.reserve, stable_asset.hub_reserve))
	}

	fn create_and_mint_position_instance(
		owner: &T::AccountId,
	) -> Result<PositionId<T::PositionInstanceId>, DispatchError> {
		<PositionInstanceSequencer<T>>::try_mutate(
			|current_value| -> Result<PositionId<T::PositionInstanceId>, DispatchError> {
				let next_position_id = *current_value;

				// TODO: generate cool looking instance id, see liquidity mining
				let instance_id = T::PositionInstanceId::from(next_position_id);

				T::NFTHandler::mint_into(&T::NFTClassId::get(), &instance_id, owner)?;

				*current_value = current_value.checked_add(1u32).ok_or(Error::<T>::Overflow)?;

				Ok(PositionId(instance_id))
			},
		)
	}

	fn increase_hub_asset_liquidity(amount: T::Balance) -> DispatchResult {
		<HubAssetLiquidity<T>>::try_mutate(|liquidity| -> DispatchResult {
			*liquidity = liquidity.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
			Ok(())
		})
	}
	fn decrease_hub_asset_liquidity(amount: T::Balance) -> DispatchResult {
		<HubAssetLiquidity<T>>::try_mutate(|liquidity| -> DispatchResult {
			*liquidity = liquidity.checked_sub(&amount).ok_or(Error::<T>::Overflow)?;
			Ok(())
		})
	}

	/// Updates total hub asset liquidity. It either burn or mint some based on the diff of in and out.
	fn update_hub_asset_liquidity(delta_amount_in: T::Balance, delta_amount_out: T::Balance) -> DispatchResult {
		match delta_amount_in.cmp(&delta_amount_out) {
			Ordering::Greater => {
				// We need to burn some in this case
				let diff = delta_amount_in
					.checked_sub(&delta_amount_out)
					.ok_or(Error::<T>::Overflow)?;
				T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), diff)?;
				Self::decrease_hub_asset_liquidity(diff)
			}
			Ordering::Less => {
				// We need to mint some in this case
				let diff = delta_amount_out
					.checked_sub(&delta_amount_in)
					.ok_or(Error::<T>::Overflow)?;
				T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), diff)?;
				Self::increase_hub_asset_liquidity(diff)
			}
			Ordering::Equal => Ok(()), // If equal, nothing to do
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

	fn sell_hub_asset(
		who: &T::AccountId,
		asset_out: T::AssetId,
		amount: T::Balance,
		limit: T::Balance,
	) -> DispatchResult {
		let mut asset_out_state = Assets::<T>::get(asset_out).ok_or(Error::<T>::AssetNotFound)?;

		let delta_r = FixedU128::from((
			amount,
			asset_out_state
				.hub_reserve
				.checked_add(&amount)
				.ok_or(Error::<T>::Overflow)?,
		))
		.checked_mul_int(asset_out_state.reserve)
		.ok_or(Error::<T>::Overflow)?;

		ensure!(delta_r >= limit, Error::<T>::BuyLimitNotReached);

		let fee_asset = FixedU128::from(1)
			.checked_sub(&Self::asset_fee())
			.ok_or(Error::<T>::Overflow)?;

		asset_out_state.reserve = asset_out_state
			.reserve
			.checked_sub(&fee_asset.checked_mul_int(delta_r).ok_or(Error::<T>::Overflow)?)
			.ok_or(Error::<T>::Overflow)?;

		asset_out_state.hub_reserve = asset_out_state
			.hub_reserve
			.checked_add(&amount)
			.ok_or(Error::<T>::Overflow)?;

		// Token updates
		T::Currency::transfer(T::HubAssetId::get(), who, &Self::protocol_account(), amount)?;
		T::Currency::transfer(asset_out, &Self::protocol_account(), who, delta_r)?;

		// Total hub asset liquidity
		Self::increase_hub_asset_liquidity(amount)?;

		// TODO: imbalance update

		// TODO: tvl update

		<Assets<T>>::insert(asset_out, asset_out_state);

		Self::deposit_event(Event::SellExecuted(
			who.clone(),
			T::HubAssetId::get(),
			asset_out,
			amount,
			delta_r,
		));

		Ok(())
	}
}
