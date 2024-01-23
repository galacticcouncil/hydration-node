// This file is part of pallet-transaction-pause

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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
// limitations under the License..

use super::*;
use frame_support::{
	storage_alias,
	traits::{Get, OnRuntimeUpgrade, StorageVersion},
	weights::Weight,
};

/// The log target.
const TARGET: &str = "runtime::transaction-pause::migration::v1";

pub mod v0 {
	use super::*;
	use sp_std::vec::Vec;

	#[storage_alias]
	pub type PausedTransactions<T: Config> = StorageMap<Pallet<T>, Twox64Concat, (Vec<u8>, Vec<u8>), (), OptionQuery>;
}
pub mod v1 {
	use super::*;

	pub struct Migration<T>(PhantomData<T>);

	impl<T: Config> OnRuntimeUpgrade for Migration<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

			let iter = v0::PausedTransactions::<T>::iter_keys();

			log::info!(target: TARGET, "Transaction pause migration: PRE checks successful!");

			Ok(iter.collect::<Vec<(Vec<u8>, Vec<u8>)>>().encode())
		}

		fn on_runtime_upgrade() -> Weight {
			log::info!(target: TARGET, "Running migration to v1 for Transaction pause");

			let mut weight = Weight::zero();

			let status = v0::PausedTransactions::<T>::drain().collect::<Vec<_>>();
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));

			for ((pallet_name, function_name), _) in status.into_iter() {
				let pallet_name_b = BoundedVec::<u8, ConstU32<MAX_STR_LENGTH>>::try_from(pallet_name.clone());
				let function_name_b = BoundedVec::<u8, ConstU32<MAX_STR_LENGTH>>::try_from(function_name.clone());

				match (pallet_name_b, function_name_b) {
					(Ok(pallet), Ok(function)) => {
						crate::PausedTransactions::<T>::insert((pallet, function), ());
						weight.saturating_accrue(T::DbWeight::get().writes(1));
					}
					_ => log::info!(
						target: TARGET,
						"Value not migrated because BoundedVec exceeds its limit: {:?}",
						(pallet_name, function_name)
					),
				};
			}

			StorageVersion::new(1).put::<Pallet<T>>();

			weight.saturating_add(T::DbWeight::get().writes(1))
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");

			let previous_state = <Vec<(Vec<u8>, Vec<u8>)> as codec::Decode>::decode(&mut state.as_slice()).unwrap();

			let new_state = crate::PausedTransactions::<T>::iter_keys()
				.map(|v| (v.0.into_inner(), v.1.into_inner()))
				.collect::<Vec<(Vec<u8>, Vec<u8>)>>();

			for old_entry in previous_state.iter() {
				assert!(
					new_state.contains(old_entry),
					"Migrated storage entries don't match the entries prior migration!"
				);
			}

			log::info!(target: TARGET, "Transaction pause migration: POST checks successful!");

			Ok(())
		}
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use super::*;
	use crate::mock::{Runtime as T, *};

	#[test]
	fn migration_works() {
		ExtBuilder.build().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 0);

			v0::PausedTransactions::<T>::insert(
				("first pallet".as_bytes().to_vec(), "first function".as_bytes().to_vec()),
				(),
			);
			v0::PausedTransactions::<T>::insert(
				(
					"second pallet".as_bytes().to_vec(),
					"second function".as_bytes().to_vec(),
				),
				(),
			);

			let state = v1::Migration::<T>::pre_upgrade().unwrap();
			let _w = v1::Migration::<T>::on_runtime_upgrade();
			v1::Migration::<T>::post_upgrade(state).unwrap();

			assert_eq!(StorageVersion::get::<Pallet<T>>(), 1);

			assert_eq!(
				crate::PausedTransactions::<T>::get((
					BoundedName::try_from("first pallet".as_bytes().to_vec()).unwrap(),
					BoundedName::try_from("first function".as_bytes().to_vec()).unwrap()
				)),
				Some(())
			);
			assert_eq!(
				crate::PausedTransactions::<T>::get((
					BoundedName::try_from("second pallet".as_bytes().to_vec()).unwrap(),
					BoundedName::try_from("second function".as_bytes().to_vec()).unwrap()
				)),
				Some(())
			);
		});
	}
}
