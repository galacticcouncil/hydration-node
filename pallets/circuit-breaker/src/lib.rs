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

use codec::{Decode, Encode};
use frame_support::{ensure, pallet_prelude::DispatchResult, traits::Get};
use hydradx_traits::{OnLiquidityChangeHandler, OnPoolStateChangeHandler};
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero};
use sp_runtime::{ArithmeticError, DispatchError, RuntimeDebug};

pub mod weights;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;

#[cfg(test)]
mod tests;

/// Max trade volume limit multiplier of liquidity that can be traded in a block
pub const MAX_LIMIT_VALUE: u32 = 10_000;

#[derive(Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
#[scale_info(skip_type_params(T))]
pub struct TradeVolumeLimit<T: Config> {
	pub volume_in: T::Balance,
	pub volume_out: T::Balance,
	pub limit: T::Balance,
}

impl<T: Config> TradeVolumeLimit<T>
where
	T::Balance: PartialOrd,
{
	pub fn update_amounts(&mut self, amount_in: T::Balance, amount_out: T::Balance) -> DispatchResult {
		self.volume_in = self
			.volume_in
			.checked_add(&amount_in)
			.ok_or(ArithmeticError::Overflow)?;
		self.volume_out = self
			.volume_out
			.checked_add(&amount_out)
			.ok_or(ArithmeticError::Overflow)?;
		Ok(())
	}

	pub fn check_min_limit(&self) -> DispatchResult {
		if self.volume_out > self.volume_in {
			let diff = self
				.volume_out
				.checked_sub(&self.volume_in)
				.ok_or(ArithmeticError::Underflow)?;
			ensure!(diff <= self.limit, Error::<T>::MinTradeVolumePerBlockReached);
		}
		Ok(())
	}

	pub fn check_max_limit(&self) -> DispatchResult {
		if self.volume_in > self.volume_out {
			let diff = self
				.volume_in
				.checked_sub(&self.volume_out)
				.ok_or(ArithmeticError::Underflow)?;
			ensure!(diff <= self.limit, Error::<T>::MaxTradeVolumePerBlockReached);
		}
		Ok(())
	}

	pub fn check_limits(&self) -> DispatchResult {
		self.check_min_limit()?;
		self.check_max_limit()?;
		Ok(())
	}
}

#[derive(Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
#[scale_info(skip_type_params(T))]
pub struct LiquidityLimit<T: Config> {
	pub added_liquidity: T::Balance,
	pub limit: T::Balance,
}

impl<T: Config> LiquidityLimit<T>
where
	T::Balance: PartialOrd,
{
	pub fn update_amount(&mut self, liquidity_in: T::Balance) -> DispatchResult {
		self.added_liquidity = self
			.added_liquidity
			.checked_add(&liquidity_in)
			.ok_or(ArithmeticError::Overflow)?;
		Ok(())
	}

	pub fn check_limit(&self) -> DispatchResult {
		ensure!(
			self.added_liquidity <= self.limit,
			Error::<T>::MaxLiquidityLimitPerBlockReached
		);
		Ok(())
	}
}

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_finalize(_n: T::BlockNumber) {
			let _ = <AllowedTradeVolumeLimitPerAsset<T>>::clear(u32::MAX, None);
			let _ = <AllowedLiquidityAmountPerAsset<T>>::clear(u32::MAX, None);
		}

		fn integrity_test() {
			assert!(
				Self::validate_limit(T::DefaultMaxNetTradeVolumeLimitPerBlock::get()).is_ok(),
				"Circuit Breaker: Max net trade volume limit per block is set to invalid value."
			);

			if let Some(liquidity_limit) = T::DefaultMaxLiquidityLimitPerBlock::get() {
				assert!(
					Self::validate_limit(liquidity_limit).is_ok(),
					"Circuit Breaker: Max liquidity limit per block is set to invalid value."
				);
			}
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

		/// Balance type.
		type Balance: Parameter
			+ Member
			+ Copy
			+ PartialOrd
			+ MaybeSerializeDeserialize
			+ Default
			+ CheckedAdd
			+ CheckedSub
			+ AtLeast32BitUnsigned
			+ MaxEncodedLen;

		/// Origin able to change the trade volume limit of an asset.
		type TechnicalOrigin: EnsureOrigin<Self::Origin>;

		/// The maximum percentage of a pool's liquidity that can be traded in a block.
		/// Represented as a non-zero fraction (nominator, denominator) with the max value being 10_000.
		type DefaultMaxNetTradeVolumeLimitPerBlock: Get<(u32, u32)>;

		/// The maximum percentage of a pool's liquidity that can be added in a block.
		/// Represented as an optional non-zero fraction (nominator, denominator) with the max value being 10_000.
		/// If set to None, the limits are not enforced.
		type DefaultMaxLiquidityLimitPerBlock: Get<Option<(u32, u32)>>;

		/// Omnipool's hub asset id. The limits are not tracked for this asset.
		type OmnipoolHubAsset: Get<Self::AssetId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Default maximum net trade volume limit per block
	#[pallet::type_value]
	pub fn DefaultTradeVolumeLimit<T: Config>() -> (u32, u32) {
		T::DefaultMaxNetTradeVolumeLimitPerBlock::get()
	}

	#[pallet::storage]
	/// Trade volume limits of assets that don't use the default value
	#[pallet::getter(fn trade_volume_limit_per_asset)]
	pub type TradeVolumeLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, (u32, u32), ValueQuery, DefaultTradeVolumeLimit<T>>;

	#[pallet::storage]
	/// Trade volumes per asset
	#[pallet::getter(fn allowed_trade_volume_limit_per_asset)]
	pub type AllowedTradeVolumeLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, TradeVolumeLimit<T>>;

	/// Default maximum liquidity limit per block
	#[pallet::type_value]
	pub fn DefaultLiquidityLimit<T: Config>() -> Option<(u32, u32)> {
		T::DefaultMaxLiquidityLimitPerBlock::get()
	}

	#[pallet::storage]
	/// Liquidity limits of assets that don't use the default value
	#[pallet::getter(fn liquidity_limit_per_asset)]
	pub type LiquidityLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, Option<(u32, u32)>, ValueQuery, DefaultLiquidityLimit<T>>;

	#[pallet::storage]
	/// Liquidity volumes per asset
	#[pallet::getter(fn allowed_liquidity_limit_per_asset)]
	pub type AllowedLiquidityAmountPerAsset<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, LiquidityLimit<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Trade volume limit of an asset was changed.
		TradeVolumeLimitChanged {
			asset_id: T::AssetId,
			trade_volume_limit: (u32, u32),
		},
		/// Liquidity limit of an asset was changed.
		LiquidityLimitChanged {
			asset_id: T::AssetId,
			liquidity_limit: Option<(u32, u32)>,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Invalid value for a limit. Limit must be non-zero.
		InvalidLimitValue,
		/// Allowed liquidity limit is not stored for asset
		LiquidityLimitNotStoredForAsset,
		/// Minimum pool's trade volume per block has been reached
		MinTradeVolumePerBlockReached,
		/// Maximum pool's trade volume per block has been reached
		MaxTradeVolumePerBlockReached,
		/// Maximum pool's liquidity limit per block has been reached
		MaxLiquidityLimitPerBlockReached,
		/// Asset is not allowed to have a limit
		NotAllowed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set trade volume limit for an asset.
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Must be `TechnicalOrigin`
		/// - `asset_id`: The identifier of an asset
		/// - `trade_volume_limit`: New trade volume limit represented as a percentage
		///
		/// Emits `TradeVolumeLimitChanged` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::set_trade_volume_limit())]
		pub fn set_trade_volume_limit(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			trade_volume_limit: (u32, u32),
		) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			ensure!(asset_id != T::OmnipoolHubAsset::get(), Error::<T>::NotAllowed);

			Self::validate_limit(trade_volume_limit)?;

			<TradeVolumeLimitPerAsset<T>>::insert(asset_id, trade_volume_limit);

			Self::deposit_event(Event::TradeVolumeLimitChanged {
				asset_id,
				trade_volume_limit,
			});

			Ok(())
		}

		/// Set liquidity limit for an asset.
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Must be `TechnicalOrigin`
		/// - `asset_id`: The identifier of an asset
		/// - `liquidity_limit`: Optional liquidity limit represented as a percentage
		///
		/// Emits `LiquidityLimitChanged` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::set_liquidity_limit())]
		pub fn set_liquidity_limit(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			liquidity_limit: Option<(u32, u32)>,
		) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			ensure!(asset_id != T::OmnipoolHubAsset::get(), Error::<T>::NotAllowed);

			if let Some(limit) = liquidity_limit {
				Self::validate_limit(limit)?;
			}

			<LiquidityLimitPerAsset<T>>::insert(asset_id, liquidity_limit);

			Self::deposit_event(Event::LiquidityLimitChanged {
				asset_id,
				liquidity_limit,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn calculate_and_store_trade_limit(asset_id: T::AssetId, current_asset_reserve: T::Balance) -> DispatchResult {
		if asset_id != T::OmnipoolHubAsset::get() && !<AllowedTradeVolumeLimitPerAsset<T>>::contains_key(asset_id) {
			let limit = Self::calculate_limit(
				current_asset_reserve,
				Pallet::<T>::trade_volume_limit_per_asset(asset_id),
			)?;
			<AllowedTradeVolumeLimitPerAsset<T>>::insert(
				asset_id,
				TradeVolumeLimit::<T> {
					limit,
					volume_in: Zero::zero(),
					volume_out: Zero::zero(),
				},
			);
		}
		Ok(())
	}

	fn calculate_and_store_liquidity_limit(asset_id: T::AssetId, initial_liquidity: T::Balance) -> DispatchResult {
		if let Some(limit) = Pallet::<T>::liquidity_limit_per_asset(asset_id) {
			if asset_id != T::OmnipoolHubAsset::get() && !<AllowedLiquidityAmountPerAsset<T>>::contains_key(asset_id) {
				let max_limit = Self::calculate_limit(initial_liquidity, limit)?;
				<AllowedLiquidityAmountPerAsset<T>>::insert(
					asset_id,
					LiquidityLimit::<T> {
						limit: max_limit,
						added_liquidity: Zero::zero(),
					},
				);
			}
		}

		Ok(())
	}

	fn ensure_and_update_trade_volume_limit(
		asset_in: T::AssetId,
		amount_in: T::Balance,
		asset_out: T::AssetId,
		amount_out: T::Balance,
	) -> DispatchResult {
		// liquidity in
		// ignore Omnipool's hub asset
		if asset_in != T::OmnipoolHubAsset::get() {
			let mut allowed_liquidity_range = Pallet::<T>::allowed_trade_volume_limit_per_asset(asset_in)
				.ok_or(Error::<T>::LiquidityLimitNotStoredForAsset)?;

			allowed_liquidity_range.update_amounts(amount_in, Zero::zero())?;
			allowed_liquidity_range.check_limits()?;

			<AllowedTradeVolumeLimitPerAsset<T>>::insert(asset_in, allowed_liquidity_range);
		}

		// liquidity out
		// ignore Omnipool's hub asset
		if asset_out != T::OmnipoolHubAsset::get() {
			let mut allowed_liquidity_range = Pallet::<T>::allowed_trade_volume_limit_per_asset(asset_out)
				.ok_or(Error::<T>::LiquidityLimitNotStoredForAsset)?;

			allowed_liquidity_range.update_amounts(Zero::zero(), amount_out)?;
			allowed_liquidity_range.check_limits()?;

			<AllowedTradeVolumeLimitPerAsset<T>>::insert(asset_out, allowed_liquidity_range);
		}

		Ok(())
	}

	fn ensure_and_update_liquidity_limit(asset_id: T::AssetId, added_liquidity: T::Balance) -> DispatchResult {
		if asset_id != T::OmnipoolHubAsset::get() && Pallet::<T>::liquidity_limit_per_asset(asset_id).is_some() {
			let mut allowed_liquidity_limit = Pallet::<T>::allowed_liquidity_limit_per_asset(asset_id)
				.ok_or(Error::<T>::LiquidityLimitNotStoredForAsset)?;

			allowed_liquidity_limit.update_amount(added_liquidity)?;
			allowed_liquidity_limit.check_limit()?;

			<AllowedLiquidityAmountPerAsset<T>>::insert(asset_id, allowed_liquidity_limit);
		}

		Ok(())
	}

	pub fn validate_limit(limit: (u32, u32)) -> DispatchResult {
		let (numerator, denominator) = (limit.0, limit.1);
		ensure!(
			numerator <= MAX_LIMIT_VALUE && denominator <= MAX_LIMIT_VALUE,
			Error::<T>::InvalidLimitValue
		);
		ensure!(
			!numerator.is_zero() && !denominator.is_zero(),
			Error::<T>::InvalidLimitValue
		);

		Ok(())
	}

	pub fn calculate_limit(liquidity: T::Balance, limit: (u32, u32)) -> Result<T::Balance, DispatchError> {
		let (numerator, denominator) = (limit.0, limit.1);

		// TODO: use u256
		liquidity
			.checked_mul(&T::Balance::from(numerator))
			.ok_or(ArithmeticError::Overflow)?
			.checked_div(&T::Balance::from(denominator))
			.ok_or_else(|| ArithmeticError::DivisionByZero.into())
	}
}

impl<T: Config> OnPoolStateChangeHandler<T::AssetId, T::Balance> for Pallet<T> {
	fn after_pool_state_change(
		asset_in: T::AssetId,
		asset_in_reserve: T::Balance,
		amount_in: T::Balance,
		asset_out: T::AssetId,
		asset_out_reserve: T::Balance,
		amount_out: T::Balance,
	) -> DispatchResult {
		Pallet::<T>::calculate_and_store_trade_limit(asset_in, asset_in_reserve)?;
		Pallet::<T>::calculate_and_store_trade_limit(asset_out, asset_out_reserve)?;
		Pallet::<T>::ensure_and_update_trade_volume_limit(asset_in, amount_in, asset_out, amount_out)?;
		Ok(())
	}
}

impl<T: Config> OnLiquidityChangeHandler<T::AssetId, T::Balance> for Pallet<T> {
	fn after_add_liquidity(
		asset_id: T::AssetId,
		initial_liquidity: T::Balance,
		added_liquidity: T::Balance,
	) -> DispatchResult {
		Pallet::<T>::calculate_and_store_liquidity_limit(asset_id, initial_liquidity)?;
		Pallet::<T>::ensure_and_update_liquidity_limit(asset_id, added_liquidity)?;
		Ok(())
	}
}
