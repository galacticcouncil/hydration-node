// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

pub use primitives::Balance;

use frame_support::{ensure, traits::Get};
use hydradx_traits::OnPoolStateChangeHandler;
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

		/// The maximum percentage of a pool's liquidity that can be traded in a block
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

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Allowed liquidity limit is not stored for asset
		LiquidityLimitNotStoredForAsset,
		/// Minimum pool trade volume per block has been reached
		MinTradeVolumePerBlockReached,
		/// Maximum pool trade volume per block has been reached
		MaxTradeVolumePerBlockReached,
	}
}

impl<T: Config> Pallet<T> {
	fn calculate_and_store_liquidity_limits(asset_id: T::AssetId, initial_liquidity: T::Balance) -> DispatchResult {
		if !<AllowedLiquidityRangePerAsset<T>>::contains_key(asset_id) {
			let liquidity_diff = T::MaxNetTradeVolumeLimitPerBlock::get().mul_floor(initial_liquidity);
			let min_limit = initial_liquidity
				.checked_sub(&liquidity_diff)
				.ok_or(ArithmeticError::Overflow)?;
			let max_limit = initial_liquidity
				.checked_add(&liquidity_diff)
				.ok_or(ArithmeticError::Overflow)?;
			<AllowedLiquidityRangePerAsset<T>>::insert(asset_id, (min_limit, max_limit));
		}
		Ok(())
	}

	fn test_liquidity_limits(asset_id: T::AssetId, updated_liquidity: T::Balance) -> DispatchResult {
		let (min_limit, max_limit) = Pallet::<T>::allowed_liqudity_range_per_asset(asset_id)
			.ok_or(Error::<T>::LiquidityLimitNotStoredForAsset)?;

		//TODO: tell don't ask, add this in some LimitRange object or so
		ensure!(
			min_limit <= updated_liquidity,
			Error::<T>::MinTradeVolumePerBlockReached
		);
		ensure!(
			max_limit >= updated_liquidity,
			Error::<T>::MaxTradeVolumePerBlockReached
		);
		Ok(())
	}
}

impl<T: Config> OnPoolStateChangeHandler<T::AssetId, T::Balance> for Pallet<T> {
	fn before_pool_state_change(
		asset_a: T::AssetId,
		asset_b: T::AssetId,
		initial_liquidity_a: T::Balance,
		initial_liquidity_b: T::Balance,
	) -> DispatchResult {
		Pallet::<T>::calculate_and_store_liquidity_limits(asset_a, initial_liquidity_a)?;
		Pallet::<T>::calculate_and_store_liquidity_limits(asset_b, initial_liquidity_b)?;
		Ok(())
	}
	fn after_pool_state_change(
		asset_a: T::AssetId,
		asset_b: T::AssetId,
		updated_liquidity_a: T::Balance,
		updated_liquidity_b: T::Balance,
	) -> DispatchResult {
		Pallet::<T>::test_liquidity_limits(asset_a, updated_liquidity_a)?;
		Pallet::<T>::test_liquidity_limits(asset_b, updated_liquidity_b)?;
		Ok(())
	}
}
