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

use scale_info::TypeInfo;
use sp_runtime::{ArithmeticError, DispatchResult, Percent};
use frame_support::traits::Get;
use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedSub};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use codec::HasCompact;

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_finalize(_n: T::BlockNumber) {
			let _ = <InitialLiquidity<T>>::clear(u32::MAX, None);
		}

		// fn integrity_test() {
		// 	assert_ne!(
		// 		T::MaxValueLimit::get().is_zero(),
		// 		"Max Value Limit is 0."
		// 	);
		// }
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

		type MaxVolumeLimit: Get<Percent>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn initial_liquidity)]
	pub type InitialLiquidity<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, (T::Balance, T::Balance)>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {}

/// Handler used by AMM pools to perform some tasks when a trade is executed.
pub trait OnTradeHandler<AssetId, Balance> {
    fn before_pool_state_change(asset_id: AssetId, initial_liquidity: Balance) -> DispatchResult;
	fn after_pool_state_change(asset_id: AssetId, initial_liquidity: Balance) -> DispatchResult;
}

impl<T: Config> OnTradeHandler<T::AssetId, T::Balance> for Pallet<T> {
	fn before_pool_state_change(asset_id: T::AssetId, initial_liquidity: T::Balance) -> DispatchResult {
		if !<InitialLiquidity<T>>::contains_key(asset_id) {
			let liquidity_diff = T::MaxVolumeLimit::get().mul_floor(initial_liquidity);
			let min_limit = initial_liquidity.checked_sub(&liquidity_diff)
				.ok_or(ArithmeticError::Underflow)?;
			let max_limit = initial_liquidity.checked_add(&liquidity_diff)
				.ok_or(ArithmeticError::Overflow)?;
			<InitialLiquidity<T>>::insert(asset_id, (min_limit, max_limit));
		}
		Ok(())
	}
	fn after_pool_state_change(asset_id: T::AssetId, liquidity: T::Balance) -> DispatchResult {
		Ok(())
	}
}