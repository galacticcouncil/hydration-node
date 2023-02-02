#![allow(warnings)]
//
// This file is part of https://github.com/galacticcouncil/HydraDX-node

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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::ensure;
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::Get;
use frame_support::transactional;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use orml_traits::arithmetic::{CheckedAdd, CheckedSub};
use scale_info::TypeInfo;
use sp_runtime::traits::Saturating;
use sp_runtime::FixedU128;
use sp_runtime::traits::{BlockNumberProvider, ConstU32};
use sp_runtime::ArithmeticError;
use sp_runtime::{BoundedVec, DispatchError};
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

use crate::types::*;

type BlockNumberFor<T> = <T as frame_system::Config>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{EncodeLike, HasCompact};

  use frame_system::pallet_prelude::OriginFor;
  use orml_traits::MultiReservableCurrency;

  #[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

  #[pallet::config]
  pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

     /// Identifier for the class of asset.
     type AssetId: Member
     + Parameter
     + Ord
     + Default
     + Copy
     + HasCompact
     + MaybeSerializeDeserialize
     + MaxEncodedLen
     + TypeInfo;

    type MultiReservableCurrency: MultiReservableCurrency<
			Self::AccountId,
      CurrencyId = Self::AssetId,
      Balance = Balance,
      >;

    /// Native Asset
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

    /// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
  }

  #[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Emitted after an Order has been placed
		OrderPlaced {
			id: OrderId,
			who: T::AccountId,
		},
    /// An Order has been (partially) filled
    OrderFill {
      id: OrderId,
      who: T::AccountId,
      amount: Balance,
    },
	}

  #[pallet::error]
	pub enum Error<T> {
		/// Order cannot be found
		OrderNotFound,
  }

  /// ID sequencer for Orders
	#[pallet::storage]
	#[pallet::getter(fn next_order_id)]
	pub type NextPositionId<T: Config> = StorageValue<_, OrderId, ValueQuery>;

  #[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type Orders<T: Config> =
		StorageMap<_, Blake2_128Concat, OrderId, Order<T::AssetId, BlockNumberFor<T>>, OptionQuery>;

  #[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as Config>::WeightInfo::create_order())]
		#[transactional]
		pub fn create_order(
      origin: OriginFor<T>,
      asset: T::AssetId,
    ) -> DispatchResult {
      Ok(())
    }
  }
}
