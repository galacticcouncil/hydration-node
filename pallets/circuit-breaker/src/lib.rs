// This file is part of HydraDX.

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

#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(feature = "std")]
pub use primitives::Balance;

use frame_support::{ensure, traits::Get};
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedSub};
use sp_runtime::{ArithmeticError, DispatchResult, Percent};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_finalize(_n: T::BlockNumber) {
			let _ = <AllowedLiquidityRangePerAsset<T>>::clear(u32::MAX, None);
		}

		fn integrity_test() {
			assert!(
				!T::MaxNetTradeVolumeLimitPerBlock::get().is_zero(),
				"Circuit Breaker: Max Net Trade Volume Limit Per Block is set to 0."
			);
		}
	}

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

		/// Balance type
		type Balance: Parameter
			+ Member
			+ Copy
			+ PartialOrd
			+ MaybeSerializeDeserialize
			+ Default
			+ CheckedAdd
			+ CheckedSub
			+ AtLeast32BitUnsigned;

		type MaxNetTradeVolumeLimitPerBlock: Get<Percent>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn allowed_liqudity_range_per_asset)]
	pub type AllowedLiquidityRangePerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, (T::Balance, T::Balance)>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Entry does not exist in the storage
		EntryNotExist,
		/// Minimum pool volume has been reached
		MinPoolVolumeReached,
		/// Maximum pool volume has been reached
		MaxPoolVolumeReached,
	}
	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {}

/// Handler used by AMM pools to perform some tasks when a trade is executed.
pub trait BeforeAndAfterPoolStateChangeHandler<AssetId, Balance> {
	fn before_pool_state_change(asset_id: AssetId, initial_liquidity: Balance) -> DispatchResult;
	fn after_pool_state_change(asset_id: AssetId, update_liquidity_state: Balance) -> DispatchResult;
}

impl<T: Config> BeforeAndAfterPoolStateChangeHandler<T::AssetId, T::Balance> for Pallet<T> {
	fn before_pool_state_change(asset_id: T::AssetId, initial_liquidity: T::Balance) -> DispatchResult {
		if !<AllowedLiquidityRangePerAsset<T>>::contains_key(asset_id) {
			let liquidity_diff = T::MaxNetTradeVolumeLimitPerBlock::get().mul_floor(initial_liquidity);
			let min_limit = initial_liquidity
				.checked_sub(&liquidity_diff)
				.ok_or(ArithmeticError::Underflow)?;
			let max_limit = initial_liquidity
				.checked_add(&liquidity_diff)
				.ok_or(ArithmeticError::Overflow)?;
			<AllowedLiquidityRangePerAsset<T>>::insert(asset_id, (min_limit, max_limit));
		}
		Ok(())
	}
	fn after_pool_state_change(asset_id: T::AssetId, update_liquidity_state: T::Balance) -> DispatchResult {
		let (min_limit, max_limit) =
			Pallet::<T>::allowed_liqudity_range_per_asset(asset_id).ok_or(Error::<T>::EntryNotExist)?;
		ensure!(min_limit <= update_liquidity_state, Error::<T>::MinPoolVolumeReached);
		ensure!(max_limit >= update_liquidity_state, Error::<T>::MaxPoolVolumeReached);
		Ok(())
	}
}
