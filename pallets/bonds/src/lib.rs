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

//! # Bonds pallet
//!
//! ## Overview
//!
//! This pallet provides functionality to issue bonds.
//! Once the bonds are mature, they can be redeemed for the underlying asset.
//! The pallet uses `Time` trait to get the timestamp of the last block, provided by the timestamp pallet.
//!
//! ## Assumptions
//!
//! * When issuing new bonds, new asset of the `AssetType::Bond` is registered for the bonds.
//! * It's possible to create multiple bonds for the same underlying asset.
//! * Bonds can be issued for all available asset types.
//! * The existential deposit of the bond is the same as of the underlying asset.
//! * A user receives the same amount of bonds as the amount of the underlying asset he provided.
//! * Maturity of bonds is represented using the Unix time in milliseconds.
//! * Underlying assets are stored in the pallet account until redeemed.
//! * Protocol fee is applied to the amount of the underlying asset and transferred to the fee receiver.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	ensure,
	pallet_prelude::{DispatchResult, Get},
	sp_runtime::{
		traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, Zero},
		ArithmeticError, DispatchError, Permill, RuntimeDebug,
	},
	traits::Time,
	PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;

use hydradx_traits::{BondRegistry, Registry};
use orml_traits::{GetByKey, MultiCurrency};
use primitives::Moment;
use sp_std::vec;
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Bond<T: Config> {
	pub maturity: Moment,
	pub asset_id: T::AssetId, // underlying asset id
	pub amount: T::Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

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
			+ MaxEncodedLen
			+ From<u128>;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Self::Balance>;

		/// Asset Registry mechanism - used to register bonds in the asset registry
		type AssetRegistry: BondRegistry<Self::AssetId, Vec<u8>, Self::Balance, DispatchError>;

		/// Provider for existential deposits of assets
		type ExistentialDeposits: GetByKey<Self::AssetId, Self::Balance>;

		/// Provider for the current block number.
		type TimestampProvider: Time<Moment = Moment>;

		/// The pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Min number of blocks for maturity.
		type MinMaturity: Get<Moment>;

		/// Min amount of bonds that can be created
		// type MinAmount: Get<Balance>; TODO: Do we want this param?

		// type Deposit: Get<Balance>; TODO: Do we want this param?

		/// The origin which can issue new bonds.
		type IssueOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The origin which can issue new bonds.
		type UnlockOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Protocol Fee
		type ProtocolFee: Get<Permill>;

		/// Protocol Fee receiver
		type FeeReceiver: Get<Self::AccountId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	/// Registered bonds
	#[pallet::getter(fn bonds)]
	pub(super) type RegisteredBonds<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, Bond<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A bond asset was registered
		BondTokenCreated {
			issuer: T::AccountId,
			asset_id: T::AssetId,
			bond_asset_id: T::AssetId,
			amount: T::Balance,
			fee: T::Balance,
		},
		/// Bonds were redeemed
		BondsRedeemed {
			who: T::AccountId,
			bond_id: T::AssetId,
			amount: T::Balance,
		},
		/// Bonds were unlocked
		BondsUnlocked { bond_id: T::AssetId },
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,
		/// Bond not registered
		BondNotRegistered,
		/// Underlying asset is not registered
		UnderlyingAssetNotRegistered,
		/// Bond is not mature
		BondNotMature,
		/// Maturity not long enough
		InvalidMaturity,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issue new bonds.
		/// New asset id is registered and assigned to the bonds.
		/// The number of bonds the issuer receives is 1:1 to the `amount` of the underlying asset.
		/// The bond asset is registered with the empty string for the asset name,
		/// and with the same existential deposit as of the underlying asset.
		/// Bonds can be redeemed 1:1 for the underlying asset once mature.
		/// Protocol fee is applied to the amount, and transferred to `T::FeeReceiver`.
		///
		/// Parameters:
		/// - `origin`: issuer of new bonds, needs to be `T::IssueOrigin`
		/// - `asset_id`: underlying asset id
		/// - `amount`: the amount of the underlying asset
		/// - `maturity`: Unix time in milliseconds, when the bonds will be mature. Needs to be set
		/// more than `T::MinMaturity` from now.
		///
		/// Emits `BondTokenCreated` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::issue())]
		pub fn issue(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			amount: T::Balance,
			maturity: Moment,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let time_diff = maturity
				.checked_sub(T::TimestampProvider::now())
				.ok_or(ArithmeticError::Overflow)?;
			ensure!(time_diff >= T::MinMaturity::get(), Error::<T>::InvalidMaturity);

			ensure!(
				T::AssetRegistry::exists(asset_id),
				Error::<T>::UnderlyingAssetNotRegistered
			);
			ensure!(
				T::Currency::free_balance(asset_id, &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			let asset_ed = T::ExistentialDeposits::get(&asset_id);

			// not covered in the tests. Create an asset with empty name should always work
			let bond_asset_id = T::AssetRegistry::create_bond_asset(asset_ed)?;

			let fee = T::ProtocolFee::get().mul_ceil(amount); // TODO: check
			let amount_without_fee = amount.checked_sub(&fee).ok_or(ArithmeticError::Overflow)?;
			let pallet_account = Self::pallet_account_id();

			T::Currency::transfer(asset_id, &who, &pallet_account, amount_without_fee)?;
			T::Currency::transfer(asset_id, &who, &T::FeeReceiver::get(), fee)?;
			T::Currency::deposit(bond_asset_id, &who, amount_without_fee)?;

			RegisteredBonds::<T>::insert(
				bond_asset_id,
				Bond {
					maturity,
					asset_id,
					amount: amount_without_fee,
				},
			);

			Self::deposit_event(Event::BondTokenCreated {
				issuer: who,
				asset_id,
				bond_asset_id,
				amount: amount_without_fee,
				fee,
			});

			Ok(())
		}

		/// Redeem bonds for the underlying asset.
		/// The amount of the underlying asset the `origin` receives is 1:1 to the `amount` of the bonds.
		/// Anyone who holds the bonds is able to redeem them.
		/// The bonds are partially redeemable.
		///
		/// Parameters:
		/// - `origin`: account id
		/// - `asset_id`: bond asset id
		/// - `amount`: the amount of the underlying underlying asset to redeem for the bonds
		///
		/// Emits `BondsRedeemed` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::redeem())]
		pub fn redeem(origin: OriginFor<T>, bond_id: T::AssetId, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			RegisteredBonds::<T>::try_mutate_exists(bond_id, |maybe_bond_data| -> DispatchResult {
				let bond_data = maybe_bond_data.as_mut().ok_or(Error::<T>::BondNotRegistered)?;

				let now = T::TimestampProvider::now();
				ensure!(now >= bond_data.maturity, Error::<T>::BondNotMature);

				ensure!(
					T::Currency::free_balance(bond_id, &who) >= amount,
					Error::<T>::InsufficientBalance
				);

				T::Currency::withdraw(bond_id, &who, amount)?;

				bond_data.amount = bond_data.amount.checked_sub(&amount).ok_or(ArithmeticError::Overflow)?;

				let pallet_account = Self::pallet_account_id();
				T::Currency::transfer(bond_data.asset_id, &pallet_account, &who, amount)?;

				// if there are no bonds left, remove the bond from the storage
				if bond_data.amount.is_zero() {
					*maybe_bond_data = None;
				}

				Self::deposit_event(Event::BondsRedeemed { who, bond_id, amount });

				Ok(())
			})?;

			Ok(())
		}

		/// Unlock bonds by making them mature.
		/// The maturity of the bonds is not updated if the bonds are already mature.
		///
		/// Parameters:
		/// - `origin`: needs to be `T::UnlockOrigin`
		/// - `bond_id`: the asset id of the bonds
		///
		/// Emits `BondsUnlocked` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::redeem())]
		pub fn unlock(origin: OriginFor<T>, bond_id: T::AssetId) -> DispatchResult {
			T::UnlockOrigin::ensure_origin(origin)?;

			RegisteredBonds::<T>::try_mutate_exists(bond_id, |maybe_bond_data| -> DispatchResult {
				let bond_data = maybe_bond_data.as_mut().ok_or(Error::<T>::BondNotRegistered)?;

				let now = T::TimestampProvider::now();
				// do nothing if the bonds are already mature
				if bond_data.maturity > now {
					bond_data.maturity = now;

					Self::deposit_event(Event::BondsUnlocked { bond_id });
				}
				Ok(())
			})?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// The account ID of the bonds pot.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn pallet_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}
