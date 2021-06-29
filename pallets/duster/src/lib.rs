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
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod benchmarking;
mod weights;

use frame_support::{dispatch::DispatchResult, traits::Get};

use orml_traits::MultiCurrencyExtended;
use orml_traits::{GetByKey, MultiCurrency};

use frame_system::ensure_signed;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
	use primitives::Amount;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + From<u32>;

		type MultiCurrency: MultiCurrencyExtended<
			Self::AccountId,
			CurrencyId = Self::CurrencyId,
			Balance = Self::Balance,
			Amount = Amount,
		>;

		type MinCurrencyDeposits: GetByKey<Self::CurrencyId, Self::Balance>;

		#[pallet::constant]
		type DustAccount: Get<Self::AccountId>;

		#[pallet::constant]
		type RewardAccount: Get<Self::AccountId>;

		#[pallet::constant]
		type Reward: Get<Self::Balance>;

		#[pallet::constant]
		type NativeCurrencyId: Get<Self::CurrencyId>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		BalanceSufficient,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Dusted(T::AccountId, T::Balance),
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight((<T as Config>::WeightInfo::dust_account(), DispatchClass::Normal, Pays::Yes))]
		pub fn dust_account(
			origin: OriginFor<T>,
			account: T::AccountId,
			currency_id: T::CurrencyId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let (dustable, dust) = Self::is_dustable(&account, currency_id);

			ensure!(dustable, Error::<T>::BalanceSufficient);

			Self::transfer_dust(&account, &T::DustAccount::get(), currency_id, dust)?;

			Self::deposit_event(Event::Dusted(account, dust));

			// Ignore the result, it fails - no problem.
			let _ = Self::reward_duster(&who, currency_id, dust);

			Ok(().into())
		}
	}
}
impl<T: Config> Pallet<T> {
	fn is_dustable(account: &T::AccountId, currency_id: T::CurrencyId) -> (bool, T::Balance) {
		let ed = T::MinCurrencyDeposits::get(&currency_id);

		let total = T::MultiCurrency::total_balance(currency_id, account);

		(total < ed, total)
	}

	fn reward_duster(_duster: &T::AccountId, _currency_id: T::CurrencyId, _dust: T::Balance) -> DispatchResult {
		let reserve_account = T::RewardAccount::get();
		let reward = T::Reward::get();

		T::MultiCurrency::transfer(T::NativeCurrencyId::get(), &reserve_account, _duster, reward)?;
		Ok(())
	}

	fn transfer_dust(
		from: &T::AccountId,
		dest: &T::AccountId,
		currency_id: T::CurrencyId,
		dust: T::Balance,
	) -> DispatchResult {
		T::MultiCurrency::transfer(currency_id, from, dest, dust)
	}
}
