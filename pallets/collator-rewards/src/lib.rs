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
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod migration;

use frame_support::{traits::Get, BoundedVec};

use orml_traits::MultiCurrency;
use pallet_session::SessionManager;
use sp_staking::SessionIndex;
use sp_std::vec::Vec;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_system::pallet_prelude::BlockNumberFor;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Balance type
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord;

		/// Currency for transfers
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::CurrencyId, Balance = Self::Balance>;

		/// Reward amount per one collator.
		#[pallet::constant]
		type RewardPerCollator: Get<Self::Balance>;

		/// Reward Asset Id
		#[pallet::constant]
		type RewardCurrencyId: Get<Self::CurrencyId>;

		/// List of collator which will not be rewarded.
		type ExcludedCollators: Get<Vec<Self::AccountId>>;

		/// The session manager this pallet will wrap that provides the collator account list on
		/// `new_session`.
		type SessionManager: SessionManager<Self::AccountId>;

		/// Max candidates
		type MaxCandidates: Get<u32>;
	}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Collator was rewarded.
		CollatorRewarded {
			who: T::AccountId,
			amount: T::Balance,
			currency: T::CurrencyId,
		},
	}

	#[pallet::storage]
	#[pallet::getter(fn collators)]
	/// Stores the collators per session (index).
	pub type Collators<T: Config> =
		StorageMap<_, Twox64Concat, SessionIndex, BoundedVec<T::AccountId, T::MaxCandidates>, ValueQuery>;
}

impl<T: Config> SessionManager<T::AccountId> for Pallet<T> {
	fn new_session(index: SessionIndex) -> Option<Vec<T::AccountId>> {
		let maybe_collators = T::SessionManager::new_session(index);
		if let Some(ref collators) = maybe_collators {
			let maybe_collators_b = BoundedVec::<T::AccountId, T::MaxCandidates>::try_from(collators.clone());
			match maybe_collators_b {
				Ok(collators_b) => Collators::<T>::insert(index, collators_b),
				Err(_) => {
					log::warn!(target: "runtime::collator-rewards", "Error reward collators: too many collators {:?}", collators);
					return None;
				}
			}
		}
		maybe_collators
	}

	fn start_session(index: SessionIndex) {
		T::SessionManager::start_session(index)
	}

	fn end_session(index: SessionIndex) {
		T::SessionManager::end_session(index);
		let excluded = T::ExcludedCollators::get();
		// remove the collators so we don't pile up storage
		for collator in Collators::<T>::take(index) {
			if !excluded.contains(&collator) {
				let (currency, amount) = (T::RewardCurrencyId::get(), T::RewardPerCollator::get());
				match T::Currency::deposit(currency, &collator, amount) {
					Ok(_) => Self::deposit_event(Event::CollatorRewarded {
						who: collator,
						amount,
						currency,
					}),
					Err(err) => log::warn!(target: "runtime::collator-rewards", "Error reward collators: {:?}", err),
				}
			}
		}
	}
}
