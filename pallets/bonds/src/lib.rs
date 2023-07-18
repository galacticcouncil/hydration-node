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
	ensure,
	pallet_prelude::{DispatchResult, Get}
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use frame_system::pallet_prelude::BlockNumberFor;

use orml_traits::MultiCurrency;
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::{
	ArithmeticError, DispatchError, Permill, RuntimeDebug,
	traits::{BlockNumberProvider, AtLeast32BitUnsigned, CheckedMul, CheckedAdd, CheckedSub}, Saturating,
};
use hydradx_traits::Registry;

#[cfg(test)]
mod tests;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Bond<T: Config>
{
	pub maturity: BlockNumberFor<T>,
	pub amount: T::Balance,
}

impl<T: Config> Bond<T> {
	pub fn maturity(&self) -> BlockNumberFor<T> {
		self.maturity
	}

	pub fn amount(&self) -> T::Balance {
		self.amount
	}
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
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Self::Balance, DispatchError>;

		/// Provider for the current block number.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		/// Min number of blocks for maturity.
		type MinMaturity: Get<Self::BlockNumber>;

		/// Protocol Fee for
		type Fee: Get<Permill>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	/// Registered bonds
	#[pallet::getter(fn assets)]
	pub(super) type Bonds<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, Bond<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A bond asset was registered
		BondTokenCreated {
			asset_id: T::AssetId,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,
		/// Bond not registered
		BondNotRegistered,
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
			maturity: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let block_diff = T::BlockNumberProvider::current_block_number().checked_sub(&maturity).ok_or(ArithmeticError::Overflow)?;
			ensure!(block_diff >= T::MinMaturity::get(), Error::<T>::InvalidMaturity);

			Ok(())
		}

		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::redeem())]
		pub fn redeem(
			origin: OriginFor<T>,
		) -> DispatchResult {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Returns false if the asset id is not registered as a bond
	fn is_mature(asset_id: T::AssetId) -> bool {
		let maybe_bond = Bonds::<T>::get(asset_id);
		match maybe_bond {
			Some(bond) => {
				let block_number = T::BlockNumberProvider::current_block_number();
				bond.maturity() >= block_number
			},
			None => false
		}
	}
}
