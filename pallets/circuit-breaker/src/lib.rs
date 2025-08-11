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
#![allow(clippy::manual_inspect)]

use codec::{Decode, Encode};
use frame_support::traits::{Contains, EnsureOrigin};
use frame_support::weights::Weight;
use frame_support::{dispatch::Pays, ensure, pallet_prelude::DispatchResult, traits::Get};
use frame_system::ensure_signed_or_root;
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
use orml_traits::Handler;
use orml_traits::{GetByKey, Happened};
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero};
use sp_runtime::Saturating;
use sp_runtime::{ArithmeticError, DispatchError, RuntimeDebug};
pub mod weights;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;

pub mod fuses;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod types;

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

	pub fn check_outflow_limit(&self) -> DispatchResult {
		if self.volume_out > self.volume_in {
			let diff = self
				.volume_out
				.checked_sub(&self.volume_in)
				.ok_or(ArithmeticError::Underflow)?;
			ensure!(diff <= self.limit, Error::<T>::TokenOutflowLimitReached);
		}
		Ok(())
	}

	pub fn check_influx_limit(&self) -> DispatchResult {
		if self.volume_in > self.volume_out {
			let diff = self
				.volume_in
				.checked_sub(&self.volume_out)
				.ok_or(ArithmeticError::Underflow)?;
			ensure!(diff <= self.limit, Error::<T>::TokenInfluxLimitReached);
		}
		Ok(())
	}

	pub fn check_limits(&self) -> DispatchResult {
		self.check_outflow_limit()?;
		self.check_influx_limit()?;
		Ok(())
	}
}

#[derive(Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
#[scale_info(skip_type_params(T))]
pub struct LiquidityLimit<T: Config> {
	pub liquidity: T::Balance,
	pub limit: T::Balance,
}

impl<T: Config> LiquidityLimit<T>
where
	T::Balance: PartialOrd,
{
	pub fn update_amount(&mut self, liquidity_in: T::Balance) -> DispatchResult {
		self.liquidity = self
			.liquidity
			.checked_add(&liquidity_in)
			.ok_or(ArithmeticError::Overflow)?;
		Ok(())
	}

	pub fn check_limit(&self) -> DispatchResult {
		ensure!(
			self.liquidity <= self.limit,
			Error::<T>::MaxLiquidityLimitPerBlockReached
		);
		Ok(())
	}
}

// Re-export pallet items so that they can be accessed from the crate namespace.
use crate::traits::AssetDepositLimiter;
use crate::types::LockdownStatus;
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::Contains;
	use traits::AssetDepositLimiter;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			T::WeightInfo::on_finalize(0, 0)
		}

		fn on_finalize(_n: BlockNumberFor<T>) {
			let _ = <AllowedTradeVolumeLimitPerAsset<T>>::clear(u32::MAX, None);
			let _ = <AllowedAddLiquidityAmountPerAsset<T>>::clear(u32::MAX, None);
			let _ = <AllowedRemoveLiquidityAmountPerAsset<T>>::clear(u32::MAX, None);
		}

		fn integrity_test() {
			assert!(
				Self::validate_limit(T::DefaultMaxNetTradeVolumeLimitPerBlock::get()).is_ok(),
				"Circuit Breaker: Max net trade volume limit per block is set to invalid value."
			);

			if let Some(liquidity_limit) = T::DefaultMaxAddLiquidityLimitPerBlock::get() {
				assert!(
					Self::validate_limit(liquidity_limit).is_ok(),
					"Circuit Breaker: Max add liquidity limit per block is set to invalid value."
				);
			}

			if let Some(liquidity_limit) = T::DefaultMaxRemoveLiquidityLimitPerBlock::get() {
				assert!(
					Self::validate_limit(liquidity_limit).is_ok(),
					"Circuit Breaker: Max remove liquidity limit per block is set to invalid value."
				);
			}
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ AtLeast32BitUnsigned;

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
			+ MaxEncodedLen
			+ From<u128>;

		/// Origin able to change the trade volume limit of an asset.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// List of accounts that bypass checks for adding/removing liquidity. Root is always whitelisted
		type WhitelistedAccounts: Contains<Self::AccountId>;

		/// The maximum percentage of a pool's liquidity that can be traded in a block.
		/// Represented as a non-zero fraction (nominator, denominator) with the max value being 10_000.
		#[pallet::constant]
		type DefaultMaxNetTradeVolumeLimitPerBlock: Get<(u32, u32)>;

		/// The maximum percentage of a pool's liquidity that can be added in a block.
		/// Represented as an optional non-zero fraction (nominator, denominator) with the max value being 10_000.
		/// If set to None, the limits are not enforced.
		#[pallet::constant]
		type DefaultMaxAddLiquidityLimitPerBlock: Get<Option<(u32, u32)>>;

		/// The maximum percentage of a pool's liquidity that can be removed in a block.
		/// Represented as an optional non-zero fraction (nominator, denominator) with the max value being 10_000.
		/// If set to None, the limits are not enforced.
		#[pallet::constant]
		type DefaultMaxRemoveLiquidityLimitPerBlock: Get<Option<(u32, u32)>>;

		/// Omnipool's hub asset id. The limits are not tracked for this asset.
		type OmnipoolHubAsset: Get<Self::AssetId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		///Deposit limiter for assets to prevent undesired issuance increase.
		type DepositLimiter: AssetDepositLimiter<Self::AccountId, Self::AssetId, Self::Balance>;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: types::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Default maximum net trade volume limit per block
	#[pallet::type_value]
	pub fn DefaultTradeVolumeLimit<T: Config>() -> (u32, u32) {
		T::DefaultMaxNetTradeVolumeLimitPerBlock::get()
	}

	#[pallet::storage]
	/// Trade volume limits of assets set by set_trade_volume_limit.
	/// If not set, returns the default limit.
	#[pallet::getter(fn trade_volume_limit_per_asset)]
	pub type TradeVolumeLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, (u32, u32), ValueQuery, DefaultTradeVolumeLimit<T>>;

	#[pallet::storage]
	/// Trade volumes per asset
	#[pallet::getter(fn allowed_trade_volume_limit_per_asset)]
	pub type AllowedTradeVolumeLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, TradeVolumeLimit<T>>;

	/// Default maximum add liquidity limit per block
	#[pallet::type_value]
	pub fn DefaultAddLiquidityLimit<T: Config>() -> Option<(u32, u32)> {
		T::DefaultMaxAddLiquidityLimitPerBlock::get()
	}

	#[pallet::storage]
	/// Liquidity limits of assets for adding liquidity.
	/// If not set, returns the default limit.
	#[pallet::getter(fn add_liquidity_limit_per_asset)]
	pub type LiquidityAddLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, Option<(u32, u32)>, ValueQuery, DefaultAddLiquidityLimit<T>>;

	#[pallet::storage]
	/// Add liquidity volumes per asset
	#[pallet::getter(fn allowed_add_liquidity_limit_per_asset)]
	pub type AllowedAddLiquidityAmountPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, LiquidityLimit<T>>;

	#[pallet::storage]
	#[pallet::getter(fn asset_lockdown_state)]
	pub type AssetLockdownState<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, LockdownStatus<BlockNumberFor<T>, T::Balance>, OptionQuery>;

	/// Default maximum remove liquidity limit per block
	#[pallet::type_value]
	pub fn DefaultRemoveLiquidityLimit<T: Config>() -> Option<(u32, u32)> {
		T::DefaultMaxRemoveLiquidityLimitPerBlock::get()
	}

	#[pallet::storage]
	/// Liquidity limits of assets for removing liquidity.
	/// If not set, returns the default limit.
	#[pallet::getter(fn remove_liquidity_limit_per_asset)]
	pub type LiquidityRemoveLimitPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, Option<(u32, u32)>, ValueQuery, DefaultRemoveLiquidityLimit<T>>;

	#[pallet::storage]
	/// Remove liquidity volumes per asset
	#[pallet::getter(fn allowed_remove_liquidity_limit_per_asset)]
	pub type AllowedRemoveLiquidityAmountPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, LiquidityLimit<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Trade volume limit of an asset was changed.
		TradeVolumeLimitChanged {
			asset_id: T::AssetId,
			trade_volume_limit: (u32, u32),
		},
		/// Add liquidity limit of an asset was changed.
		AddLiquidityLimitChanged {
			asset_id: T::AssetId,
			liquidity_limit: Option<(u32, u32)>,
		},
		/// Remove liquidity limit of an asset was changed.
		RemoveLiquidityLimitChanged {
			asset_id: T::AssetId,
			liquidity_limit: Option<(u32, u32)>,
		},
		/// Asset went to lockdown
		AssetLockdown {
			asset_id: T::AssetId,
			until: BlockNumberFor<T>,
		},
		/// Asset lockdown was removed
		AssetLockdownRemoved { asset_id: T::AssetId },

		/// All reserved amount of deposit was released
		DepositReleased { who: T::AccountId, asset_id: T::AssetId },
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Invalid value for a limit. Limit must be non-zero.
		InvalidLimitValue,
		/// Allowed liquidity limit is not stored for asset
		LiquidityLimitNotStoredForAsset,
		/// Token trade outflow per block has been reached
		TokenOutflowLimitReached,
		/// Token trade influx per block has been reached
		TokenInfluxLimitReached,
		/// Maximum pool's liquidity limit per block has been reached
		MaxLiquidityLimitPerBlockReached,
		/// Asset is not allowed to have a limit
		NotAllowed,
		/// Asset still in lockdown as it reached the allowed deposit limit for the period
		/// Query the `asset_lockdown_state` storage to determine until which block the asset is locked,
		/// so that the deposit can be released afterward.
		AssetInLockdown,
		/// Asset is not in a lockdown
		AssetNotInLockdown,
		/// Invalid amount to save deposit
		InvalidAmount,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set trade volume limit for an asset.
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Must be `UpdateLimitsOrigin`
		/// - `asset_id`: The identifier of an asset
		/// - `trade_volume_limit`: New trade volume limit represented as a percentage
		///
		/// Emits `TradeVolumeLimitChanged` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_trade_volume_limit())]
		pub fn set_trade_volume_limit(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			trade_volume_limit: (u32, u32),
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(asset_id != T::OmnipoolHubAsset::get(), Error::<T>::NotAllowed);

			Self::validate_limit(trade_volume_limit)?;

			<TradeVolumeLimitPerAsset<T>>::insert(asset_id, trade_volume_limit);

			Self::deposit_event(Event::TradeVolumeLimitChanged {
				asset_id,
				trade_volume_limit,
			});

			Ok(())
		}

		/// Set add liquidity limit for an asset.
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Must be `UpdateLimitsOrigin`
		/// - `asset_id`: The identifier of an asset
		/// - `liquidity_limit`: Optional add liquidity limit represented as a percentage
		///
		/// Emits `AddLiquidityLimitChanged` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_add_liquidity_limit())]
		pub fn set_add_liquidity_limit(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			liquidity_limit: Option<(u32, u32)>,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(asset_id != T::OmnipoolHubAsset::get(), Error::<T>::NotAllowed);

			if let Some(limit) = liquidity_limit {
				Self::validate_limit(limit)?;
			}

			<LiquidityAddLimitPerAsset<T>>::insert(asset_id, liquidity_limit);

			Self::deposit_event(Event::AddLiquidityLimitChanged {
				asset_id,
				liquidity_limit,
			});

			Ok(())
		}

		/// Set remove liquidity limit for an asset.
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Must be `UpdateLimitsOrigin`
		/// - `asset_id`: The identifier of an asset
		/// - `liquidity_limit`: Optional remove liquidity limit represented as a percentage
		///
		/// Emits `RemoveLiquidityLimitChanged` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_remove_liquidity_limit())]
		pub fn set_remove_liquidity_limit(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			liquidity_limit: Option<(u32, u32)>,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(asset_id != T::OmnipoolHubAsset::get(), Error::<T>::NotAllowed);

			if let Some(limit) = liquidity_limit {
				Self::validate_limit(limit)?;
			}

			<LiquidityRemoveLimitPerAsset<T>>::insert(asset_id, liquidity_limit);

			Self::deposit_event(Event::RemoveLiquidityLimitChanged {
				asset_id,
				liquidity_limit,
			});

			Ok(())
		}

		/// Lockdown an asset for minting
		///
		/// Can be called only by an authority origin
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Must be `AuthorityOrigin`
		/// - `asset_id`: The identifier of an asset
		/// - `until`: The block number until which the asset is locked
		///
		/// /// Emits `AssetLockdowned` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::lockdown_asset())]
		pub fn lockdown_asset(origin: OriginFor<T>, asset_id: T::AssetId, until: BlockNumberFor<T>) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			Self::do_lockdown_asset(asset_id, until)
		}

		/// Remove asset lockdown regardless of the state.
		///
		/// Can be called only by an authority origin
		///
		/// Parameters:
		///
		/// - `origin`: The dispatch origin for this call. Must be `AuthorityOrigin`
		/// - `asset_id`: The identifier of an asset
		///
		///Emits `AssetLockdownRemoved` event when successful.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::force_lift_lockdown())]
		pub fn force_lift_lockdown(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let Some(last_state) = AssetLockdownState::<T>::get(asset_id) else {
				return Err(Error::<T>::AssetNotInLockdown.into());
			};
			ensure!(
				matches!(last_state, crate::types::LockdownStatus::Locked(_)),
				Error::<T>::AssetNotInLockdown
			);

			Self::do_lift_lockdown(asset_id, T::Balance::default())
		}

		/// Release deposit of an asset.
		///
		/// It releases all the pallet reserved balance of the asset for the given account
		///
		/// Can be called by any origin, but only if the asset is not in active lockdown.
		///
		/// The caller does not pay for this call if successful.
		///
		/// Parameters:
		/// - `origin`: The dispatch origin for this call. Can be signed or root.
		/// - `who`: The account that is saving the deposit.
		/// - `asset_id`: The identifier of the asset.
		///
		/// Emits `DepositReleased` event when successful.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::release_deposit())]
		pub fn release_deposit(
			origin: OriginFor<T>,
			who: T::AccountId,
			asset_id: T::AssetId,
		) -> DispatchResultWithPostInfo {
			match ensure_signed_or_root(origin.clone()) {
				Ok(_) => {}
				Err(_) => T::AuthorityOrigin::ensure_origin(origin).map(|_| ())?,
			}

			let current_block = <frame_system::Pallet<T>>::block_number();
			let last_state = AssetLockdownState::<T>::get(asset_id);

			if let Some(LockdownStatus::Locked(until)) = last_state {
				if until >= current_block {
					return Err(Error::<T>::AssetInLockdown.into());
				}
			}

			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::OnDepositRelease::handle(
				&(asset_id, who.clone()),
			)?;

			Self::deposit_event(Event::DepositReleased { who, asset_id });

			Ok(Pays::No.into())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn initialize_trade_limit(asset_id: T::AssetId, initial_asset_reserve: T::Balance) -> DispatchResult {
		if asset_id != T::OmnipoolHubAsset::get() && !<AllowedTradeVolumeLimitPerAsset<T>>::contains_key(asset_id) {
			let limit = Self::calculate_limit(
				initial_asset_reserve,
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

	fn calculate_and_store_liquidity_limits(asset_id: T::AssetId, initial_liquidity: T::Balance) -> DispatchResult {
		// we don't track liquidity limits for the Omnipool Hub asset
		if asset_id == T::OmnipoolHubAsset::get() {
			return Ok(());
		}

		// add liquidity
		if let Some(limit) = Pallet::<T>::add_liquidity_limit_per_asset(asset_id) {
			if !<AllowedAddLiquidityAmountPerAsset<T>>::contains_key(asset_id) {
				let max_limit = Self::calculate_limit(initial_liquidity, limit)?;
				<AllowedAddLiquidityAmountPerAsset<T>>::insert(
					asset_id,
					LiquidityLimit::<T> {
						limit: max_limit,
						liquidity: Zero::zero(),
					},
				);
			}
		}

		// remove liquidity
		if let Some(limit) = Pallet::<T>::remove_liquidity_limit_per_asset(asset_id) {
			if !<AllowedRemoveLiquidityAmountPerAsset<T>>::contains_key(asset_id) {
				let max_limit = Self::calculate_limit(initial_liquidity, limit)?;
				<AllowedRemoveLiquidityAmountPerAsset<T>>::insert(
					asset_id,
					LiquidityLimit::<T> {
						limit: max_limit,
						liquidity: Zero::zero(),
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

	fn ensure_and_update_add_liquidity_limit(asset_id: T::AssetId, added_liquidity: T::Balance) -> DispatchResult {
		if asset_id != T::OmnipoolHubAsset::get() && Pallet::<T>::add_liquidity_limit_per_asset(asset_id).is_some() {
			let mut allowed_liquidity_limit = Pallet::<T>::allowed_add_liquidity_limit_per_asset(asset_id)
				.ok_or(Error::<T>::LiquidityLimitNotStoredForAsset)?;

			allowed_liquidity_limit.update_amount(added_liquidity)?;
			allowed_liquidity_limit.check_limit()?;

			<AllowedAddLiquidityAmountPerAsset<T>>::insert(asset_id, allowed_liquidity_limit);
		}

		Ok(())
	}

	fn ensure_and_update_remove_liquidity_limit(asset_id: T::AssetId, removed_liquidity: T::Balance) -> DispatchResult {
		if asset_id != T::OmnipoolHubAsset::get() && Pallet::<T>::remove_liquidity_limit_per_asset(asset_id).is_some() {
			let mut allowed_liquidity_limit = Pallet::<T>::allowed_remove_liquidity_limit_per_asset(asset_id)
				.ok_or(Error::<T>::LiquidityLimitNotStoredForAsset)?;

			allowed_liquidity_limit.update_amount(removed_liquidity)?;
			allowed_liquidity_limit.check_limit()?;

			<AllowedRemoveLiquidityAmountPerAsset<T>>::insert(asset_id, allowed_liquidity_limit);
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

	pub fn ensure_pool_state_change_limit(
		asset_in: T::AssetId,
		asset_in_reserve: T::Balance,
		amount_in: T::Balance,
		asset_out: T::AssetId,
		asset_out_reserve: T::Balance,
		amount_out: T::Balance,
	) -> Result<Weight, DispatchError> {
		Pallet::<T>::initialize_trade_limit(asset_in, asset_in_reserve)?;
		Pallet::<T>::initialize_trade_limit(asset_out, asset_out_reserve)?;
		Pallet::<T>::ensure_and_update_trade_volume_limit(asset_in, amount_in, asset_out, amount_out)?;

		Ok(T::WeightInfo::ensure_pool_state_change_limit())
	}

	pub fn ensure_add_liquidity_limit(
		origin: OriginFor<T>,
		asset_id: T::AssetId,
		initial_liquidity: T::Balance,
		added_liquidity: T::Balance,
	) -> Result<Weight, DispatchError> {
		let is_whitelisted = Self::is_origin_whitelisted_or_root(origin)?;
		if is_whitelisted {
			return Ok(Weight::zero());
		}

		Pallet::<T>::calculate_and_store_liquidity_limits(asset_id, initial_liquidity)?;
		Pallet::<T>::ensure_and_update_add_liquidity_limit(asset_id, added_liquidity)?;

		Ok(T::WeightInfo::ensure_add_liquidity_limit())
	}

	pub fn ensure_remove_liquidity_limit(
		origin: OriginFor<T>,
		asset_id: T::AssetId,
		initial_liquidity: T::Balance,
		removed_liquidity: T::Balance,
	) -> Result<Weight, DispatchError> {
		let is_whitelisted = Self::is_origin_whitelisted_or_root(origin)?;
		if is_whitelisted {
			return Ok(Weight::zero());
		}

		Pallet::<T>::calculate_and_store_liquidity_limits(asset_id, initial_liquidity)?;
		Pallet::<T>::ensure_and_update_remove_liquidity_limit(asset_id, removed_liquidity)?;

		Ok(T::WeightInfo::ensure_remove_liquidity_limit())
	}

	pub(crate) fn is_origin_whitelisted_or_root(origin: OriginFor<T>) -> Result<bool, DispatchError> {
		let who = ensure_signed_or_root(origin)?;
		match who {
			Some(account) => {
				if T::WhitelistedAccounts::contains(&account) {
					Ok(true)
				} else {
					Ok(false)
				}
			}
			None => {
				//origin is root
				//root is always whitelisted
				Ok(true)
			}
		}
	}

	pub(crate) fn do_reset_deposit_limits(asset_id: T::AssetId, extra_amount: T::Balance) -> DispatchResult {
		let asset_issuance =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Issuance::get(&asset_id);

		AssetLockdownState::<T>::insert(
			asset_id,
			LockdownStatus::Unlocked((
				<frame_system::Pallet<T>>::block_number(),
				asset_issuance.saturating_sub(extra_amount),
			)),
		);

		Ok(())
	}

	pub(crate) fn do_lockdown_asset(asset_id: T::AssetId, until: BlockNumberFor<T>) -> DispatchResult {
		AssetLockdownState::<T>::insert(asset_id, LockdownStatus::Locked(until));
		<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::OnLimitReached::happened(
			&(asset_id),
		);
		Pallet::<T>::deposit_event(Event::AssetLockdown { asset_id, until });
		Ok(())
	}

	pub(crate) fn do_lift_lockdown(asset_id: T::AssetId, extra_amount: T::Balance) -> DispatchResult {
		Self::do_reset_deposit_limits(asset_id, extra_amount)?;
		Pallet::<T>::deposit_event(Event::AssetLockdownRemoved { asset_id });
		Ok(())
	}

	pub(crate) fn do_lock_deposit(who: &T::AccountId, asset_id: T::AssetId, amount: T::Balance) -> DispatchResult {
		<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(
			asset_id,
			who.clone(),
			amount,
		))
	}
}
