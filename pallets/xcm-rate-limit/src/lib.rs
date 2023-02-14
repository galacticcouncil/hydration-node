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

use codec::{Decode, Encode};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
	sp_runtime::{
		traits::{DispatchInfoOf, SignedExtension},
		transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction},
	},
	traits::{Currency, Get, Imbalance, IsSubType},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use primitives::Balance;
use scale_info::TypeInfo;
use sp_runtime::{traits::Zero, ModuleError};
use sp_std::{marker::PhantomData, prelude::*, vec::Vec};
use weights::WeightInfo;

mod benchmarking;
mod traits;
pub use traits::*;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Prefix: Get<&'static [u8]>;

		type WeightInfo: WeightInfo;

		type Currency: Currency<Self::AccountId>;

		// This type is needed to convert from Currency to Balance
		type CurrencyBalance: From<Balance>
			+ Into<<Self::Currency as Currency<<Self as frame_system::Config>::AccountId>>::Balance>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Event(),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Error
		TestError,
	}

	/// Asset id storage for each shared token
	#[pallet::storage]
	#[pallet::getter(fn claims)]
	pub type Claims<T: Config> = StorageMap<_, Blake2_128Concat, EthereumAddress, BalanceOf<T>, ValueQuery>;


	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight((<T as Config>::WeightInfo::claim(), DispatchClass::Normal, Pays::No))]
		pub fn asd(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {

}


