// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

use frame_support::pallet_prelude::Get;
use frame_support::sp_runtime::FixedPointOperand;
use frame_support::transactional;
use frame_support::PalletId;
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned};
use sp_std::prelude::*;

use orml_traits::MultiCurrency;
use sp_runtime::DispatchError;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod types;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

pub(crate) const LOG_TARGET: &str = "runtime::omnipool";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: crate::LOG_TARGET,
			concat!("[{:?}] 👜", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
	};
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::types::{AssetState, Position, PositionId, Price, SimpleImbalance};
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{CheckedAdd, CheckedMul};
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
			+ FixedPointOperand;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Position identifier
		type PositionInstanceId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Self::Balance>;

		/// Add token origin
		type AddTokenOrigin: EnsureOrigin<Self::Origin, Success = Option<Self::AccountId>>;

		/// Hub Asset ID
		#[pallet::constant]
		type HubAssetId: Get<Self::AssetId>;

		/// Hub Asset ID
		#[pallet::constant]
		type StableCoinAssetId: Get<Self::AssetId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: weights::WeightInfo;
	}

	#[pallet::storage]
	/// State of an asset in the omnipool
	pub(super) type Assets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, AssetState<T::Balance>>;

	#[pallet::storage]
	/// Imbalance of hub asset
	// TODO: support for negative or positive imbalance
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

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		TokenAdded(T::AssetId),
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq))]
	pub enum Error<T> {
		/// No stable asset in the pool
		NoStableCoinInPool,
		///
		Overflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as Config>::WeightInfo::add_token())]
		#[transactional]
		pub fn add_token(
			origin: OriginFor<T>,
			asset: T::AssetId,
			amount: T::Balance,
			initial_price: Price,
		) -> DispatchResult {
			let account = T::AddTokenOrigin::ensure_origin(origin)?;

			let mut state = AssetState::<T::Balance>::default();

			let hub_reserve = initial_price.checked_mul_int(amount).ok_or(Error::<T>::Overflow)?;

			state.reserve = amount;
			state.hub_reserve = hub_reserve;
			state.shares = amount;
			state.protocol_shares = amount;
			state.tvl = amount;

			<Assets<T>>::insert(asset, state);

			// Note: Q here is how do we know if we adding asset as protocol ?
			// currently if root ( None ), it means protocol, so no transfer done assuming asset is already in the protocol account
			if let Some(who) = account {
				T::Currency::transfer(asset, &who, &Self::protocol_account(), amount)?;
			}

			// Mint matching Hub asset
			T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), hub_reserve)?;

			// Imbalance update
			let mut current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity = <HubAssetLiquidity<T>>::get();

			let delta_imbalance = initial_price
				.checked_mul(&FixedU128::from((current_imbalance.value, current_hub_asset_liquidity)))
				.ok_or(Error::<T>::Overflow)?
				.checked_mul_int(amount)
				.ok_or(Error::<T>::Overflow)?;

			// TODO: verify if always add here.
			current_imbalance.add::<T>(delta_imbalance)?;

			// Total hub asset liquidity update
			// Note: must be done after imbalance since it requires current value before update
			<HubAssetLiquidity<T>>::try_mutate(|liquidity| -> DispatchResult {
				*liquidity = liquidity.checked_add(&hub_reserve).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			// TVL update
			let (stable_asset_reserve, stable_asset_hub_reserve) = Self::stable_asset()?;
			let delta_tvl = initial_price
				.checked_mul(&Price::from((stable_asset_reserve, stable_asset_hub_reserve)))
				.ok_or(Error::<T>::Overflow)?
				.checked_mul_int(amount);
			let delta_tvl = delta_tvl.ok_or(Error::<T>::Overflow)?;

			<TotalTVL<T>>::try_mutate(|tvl| -> DispatchResult {
				*tvl = tvl.checked_add(&delta_tvl).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			log!(
				debug,
				"Added token to pool - tvl {:?}, imbalance {:?}",
				delta_tvl,
				delta_imbalance
			);

			Self::deposit_event(Event::TokenAdded(asset));

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

	fn stable_asset() -> Result<(T::Balance, T::Balance), DispatchError> {
		let stable_asset = <Assets<T>>::get(T::StableCoinAssetId::get()).ok_or(Error::<T>::NoStableCoinInPool)?;
		Ok((stable_asset.reserve, stable_asset.hub_reserve))
	}
}
