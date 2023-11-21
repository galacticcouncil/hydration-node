// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

//! # Referrals pallet
//!

#![cfg_attr(not(feature = "std"), no_std)]

mod weights;

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::AccountIdConversion;

pub use pallet::*;

use weights::WeightInfo;

pub type Balance = u128;
pub type ReferralCode<S> = BoundedVec<u8, S>;

const MIN_CODE_LENGTH: usize = 3;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::fungibles::Transfer;
	use frame_support::PalletId;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type
		type AssetId: frame_support::traits::tokens::AssetId + MaybeSerializeDeserialize;

		/// Support for transfers.
		type Currency: Transfer<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>;

		/// Pallet id. Determines account which holds accumulated rewards in various assets.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Registration fee details.
		/// (ID of an asset which fee is to be paid in, Amount, Beneficiary account)
		#[pallet::constant]
		type RegistrationFee: Get<(Self::AssetId, Balance, Self::AccountId)>;

		/// Maximum referral code length.
		#[pallet::constant]
		type CodeLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Referral codes
	/// Maps an account to a referral code.
	#[pallet::storage]
	#[pallet::getter(fn referral_account)]
	pub(super) type ReferralCodes<T: Config> =
		StorageMap<_, Blake2_128Concat, ReferralCode<T::CodeLength>, T::AccountId>;

	/// Linked accounts.
	/// Maps an account to a referral account.
	#[pallet::storage]
	#[pallet::getter(fn linked_referral_account)]
	pub(super) type LinkedAccounts<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

	/// Accrued amounts of an asset from trading activity.
	/// Maps an amount to asset and account. This amount needs to be converted to native currency prior to claiming as a reward.
	#[pallet::storage]
	#[pallet::getter(fn accrued)]
	pub(super) type Accrued<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AssetId, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Accumulated rewards
	/// Reward amount of native asset per account.
	#[pallet::storage]
	#[pallet::getter(fn account_rewards)]
	pub(super) type Rewards<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		CodeRegistered {
			code: ReferralCode<T::CodeLength>,
			account: T::AccountId,
		},
		CodeLinked {
			account: T::AccountId,
			code: ReferralCode<T::CodeLength>,
			referral_account: T::AccountId,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		TooLong,
		TooShort,
		InvalidCharacter,
		AlreadyExists,
		InvalidCode,
		AlreadyLinked,
		ZeroAmount,
		/// Linking an account to the same referral account is not allowed.
		LinkNotAllowed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register new referral code.
		///
		/// `origin` pays the registration fee.
		/// `code` is assigned to the given `account`.
		///
		/// Length of the `code` must be at least `MIN_CODE_LENGTH`.
		/// Maximum length is limited to `T::CodeLength`.
		/// `code` must contain only alfa-numeric characters and all characters will be converted to upper case.
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `code`:
		/// - `account`:
		///
		/// Emits `CodeRegistered` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register_code())]
		pub fn register_code(origin: OriginFor<T>, code: Vec<u8>, account: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let code: ReferralCode<T::CodeLength> = code.try_into().map_err(|_| Error::<T>::TooLong)?;

			ensure!(code.len() >= MIN_CODE_LENGTH, Error::<T>::TooShort);

			//TODO: can we do without cloning ? or perhaps merge with normalization
			ensure!(
				code.clone()
					.into_inner()
					.iter()
					.all(|c| char::is_alphanumeric(*c as char)),
				Error::<T>::InvalidCharacter
			);

			let code = Self::normalize_code(code);

			ReferralCodes::<T>::mutate(code.clone(), |v| -> DispatchResult {
				ensure!(v.is_none(), Error::<T>::AlreadyExists);

				let (fee_asset, fee_amount, beneficiary) = T::RegistrationFee::get();
				T::Currency::transfer(fee_asset, &who, &beneficiary, fee_amount, true)?;

				*v = Some(account.clone());
				Self::deposit_event(Event::CodeRegistered { code, account });
				Ok(())
			})
		}

		/// Link a code to an account.
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `code`:
		///
		/// Emits `CodeLinked` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::link_code())]
		pub fn link_code(origin: OriginFor<T>, code: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let code: ReferralCode<T::CodeLength> = code.try_into().map_err(|_| Error::<T>::InvalidCode)?;
			let code = Self::normalize_code(code);
			let ref_account = Self::referral_account(&code).ok_or(Error::<T>::InvalidCode)?;

			LinkedAccounts::<T>::mutate(who.clone(), |v| -> DispatchResult {
				ensure!(v.is_none(), Error::<T>::AlreadyLinked);

				ensure!(who != ref_account, Error::<T>::LinkNotAllowed);

				*v = Some(ref_account.clone());
				Self::deposit_event(Event::CodeLinked {
					account: who,
					code,
					referral_account: ref_account,
				});
				Ok(())
			})?;
			Ok(())
		}

		/// Convert accrued asset amount to native currency.
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `asset_id`:
		///
		/// Emits `Converted` event when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::convert())]
		pub fn convert(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Ok(())
		}

		/// Claim accumulated rewards
		///
		/// /// Parameters:
		/// - `origin`:
		///
		/// Emits `Claimed` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	//TODO: when added to runtime, make sure the account is added to the whitelist of account that cannot be dusted
	pub fn pot_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub(crate) fn normalize_code(code: ReferralCode<T::CodeLength>) -> ReferralCode<T::CodeLength> {
		let r = code.into_inner().iter().map(|v| v.to_ascii_uppercase()).collect();
		ReferralCode::<T::CodeLength>::truncate_from(r)
	}
}
