// Originally created by Acala. Modified by GalacticCouncil.

// Copyright (C) 2020-2022 Acala Foundation, GalacticCouncil.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	pallet_prelude::*,
	traits::{CallMetadata, Contains, GetCallMetadata, PalletInfoAccess},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
use sp_runtime::DispatchResult;
use sp_std::{prelude::*, vec::Vec};

mod benchmarking;
pub mod migration;
mod mock;
mod tests;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	// max length of a pallet name or function name
	pub const MAX_STR_LENGTH: u32 = 40;
	pub type BoundedName = BoundedVec<u8, ConstU32<MAX_STR_LENGTH>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin which may set filter.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// can not pause
		CannotPause,
		/// invalid character encoding
		InvalidCharacter,
		/// pallet name or function name is too long
		NameTooLong,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Paused transaction
		TransactionPaused {
			pallet_name_bytes: Vec<u8>,
			function_name_bytes: Vec<u8>,
		},
		/// Unpaused transaction
		TransactionUnpaused {
			pallet_name_bytes: Vec<u8>,
			function_name_bytes: Vec<u8>,
		},
	}

	/// The paused transaction map
	///
	/// map (PalletNameBytes, FunctionNameBytes) => Option<()>
	#[pallet::storage]
	#[pallet::getter(fn paused_transactions)]
	pub type PausedTransactions<T: Config> = StorageMap<_, Twox64Concat, (BoundedName, BoundedName), (), OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::pause_transaction())]
		pub fn pause_transaction(origin: OriginFor<T>, pallet_name: Vec<u8>, function_name: Vec<u8>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let pallet_name_b = BoundedName::try_from(pallet_name.clone()).map_err(|_| Error::<T>::NameTooLong)?;
			let function_name_b = BoundedName::try_from(function_name.clone()).map_err(|_| Error::<T>::NameTooLong)?;

			// not allowed to pause calls of this pallet to ensure safe
			let pallet_name_string = sp_std::str::from_utf8(&pallet_name).map_err(|_| Error::<T>::InvalidCharacter)?;
			ensure!(
				pallet_name_string != <Self as PalletInfoAccess>::name(),
				Error::<T>::CannotPause
			);

			PausedTransactions::<T>::mutate_exists((pallet_name_b, function_name_b), |maybe_paused| {
				if maybe_paused.is_none() {
					*maybe_paused = Some(());
					Self::deposit_event(Event::TransactionPaused {
						pallet_name_bytes: pallet_name,
						function_name_bytes: function_name,
					});
				}
			});
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::unpause_transaction())]
		pub fn unpause_transaction(
			origin: OriginFor<T>,
			pallet_name: Vec<u8>,
			function_name: Vec<u8>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let pallet_name_b = BoundedName::try_from(pallet_name.clone()).map_err(|_| Error::<T>::NameTooLong)?;
			let function_name_b = BoundedName::try_from(function_name.clone()).map_err(|_| Error::<T>::NameTooLong)?;

			if PausedTransactions::<T>::take((&pallet_name_b, &function_name_b)).is_some() {
				Self::deposit_event(Event::TransactionUnpaused {
					pallet_name_bytes: pallet_name,
					function_name_bytes: function_name,
				});
			};
			Ok(())
		}
	}
}

pub struct PausedTransactionFilter<T>(PhantomData<T>);
impl<T: Config> Contains<T::RuntimeCall> for PausedTransactionFilter<T>
where
	<T as frame_system::Config>::RuntimeCall: GetCallMetadata,
{
	fn contains(call: &T::RuntimeCall) -> bool {
		let CallMetadata {
			function_name,
			pallet_name,
		} = call.get_call_metadata();

		let pallet_name_b = BoundedName::try_from(pallet_name.as_bytes().to_vec());
		let function_name_b = BoundedName::try_from(function_name.as_bytes().to_vec());
		if pallet_name_b.is_err() || function_name_b.is_err() {
			return false;
		}

		// it's safe to call unwrap here thanks to the test above
		PausedTransactions::<T>::contains_key((pallet_name_b.unwrap_or_default(), function_name_b.unwrap_or_default()))
	}
}
