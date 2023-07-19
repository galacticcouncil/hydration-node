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

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	ensure, BoundedVec,
	pallet_prelude::{DispatchResult, Get},
	traits::Time,
	PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use primitives::Moment;

use hydradx_traits::BondRegistry;
use orml_traits::MultiCurrency;
use pallet_asset_registry::AssetDetails;
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::{
	traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, Zero},
	ArithmeticError, DispatchError, Permill, RuntimeDebug,
};

#[cfg(test)]
mod tests;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Bond<T: Config> {
	pub maturity: Moment,
	// underlying asset id
	pub asset_id: T::AssetId,
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

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: BondRegistry<
			Self::AssetId,
			Vec<u8>,
			Self::Balance,
			AssetDetails<Self::AssetId, Self::Balance, BoundedVec<u8, ConstU32<32>>>,
			DispatchError,
		>;

		/// Provider for the current block number.
		type TimestampProvider: Time<Moment = Moment>;

		/// The pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Min number of blocks for maturity.
		type MinMaturity: Get<Moment>;

		/// Protocol Fee for
		type ProtocolFee: Get<Permill>;

		/// Protocol Fee receiver
		type FeeReceiver: Get<Self::AccountId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	/// Registered bonds
	#[pallet::getter(fn bonds)]
	pub(super) type Bonds<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, Bond<T>>;

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
		/// A bond asset was registered
		BondsRedeemed {
			account_id: T::AccountId,
			bond_id: T::AssetId,
			amount: T::Balance,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,
		/// Bond not registered
		BondNotRegistered,
		/// Bond is not mature
		BondNotMature,
		/// Maturity not long enough
		InvalidMaturity,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
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

			let time_diff = T::TimestampProvider::now()
				.checked_sub(maturity)
				.ok_or(ArithmeticError::Overflow)?;
			ensure!(time_diff >= T::MinMaturity::get(), Error::<T>::InvalidMaturity);

			ensure!(
				T::Currency::free_balance(asset_id, &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			let asset_details = T::AssetRegistry::get_asset_details(asset_id)?;
			let bond_asset_id = T::AssetRegistry::create_bond_asset(&vec![], asset_details.existential_deposit)?;


			let fee = T::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee = amount.checked_sub(&fee).ok_or(ArithmeticError::Overflow)?;
			let pallet_account = Self::account_id();

			T::Currency::transfer(asset_id, &who, &pallet_account, amount_without_fee)?;
			T::Currency::transfer(asset_id, &who, &T::FeeReceiver::get(), fee)?;
			T::Currency::deposit(bond_asset_id, &who, amount_without_fee)?;

			Bonds::<T>::insert(
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

		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::redeem())]
		pub fn redeem(origin: OriginFor<T>, bond_id: T::AssetId, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Bonds::<T>::try_mutate_exists(bond_id, |maybe_bond_data| -> DispatchResult {
				let bond_data = maybe_bond_data.as_mut().ok_or(Error::<T>::BondNotRegistered)?;

				let now = T::TimestampProvider::now();
				ensure!(now >= bond_data.maturity, Error::<T>::BondNotMature);

				ensure!(
					T::Currency::free_balance(bond_id, &who) >= amount,
					Error::<T>::InsufficientBalance
				);

				T::Currency::withdraw(bond_id, &who, amount)?;

				let pallet_account = Self::account_id();
				T::Currency::transfer(bond_data.asset_id, &pallet_account, &who, amount)?;

				bond_data.amount = bond_data.amount.checked_sub(&amount).ok_or(ArithmeticError::Overflow)?;

				if bond_data.amount.is_zero() {
					*maybe_bond_data = None;
				}

				Self::deposit_event(Event::BondsRedeemed {
					account_id: who,
					bond_id,
					amount,
				});

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
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}
