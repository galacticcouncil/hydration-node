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
	log, storage_alias,
	traits::{Get, StorageVersion},
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

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

		log::info!(target: TARGET, "Transaction pause migration: PRE checks successful!");
	}

	pub fn migrate<T: Config>() -> Weight {
		log::info!(target: TARGET, "Running migration to v1 for Transaction pause");

		let mut weight = Weight::zero();

		let status = v0::PausedTransactions::<T>::drain().collect::<Vec<_>>();
		weight.saturating_accrue(T::DbWeight::get().reads(status.len() as u64));

		for ((pallet_name, function_name), _) in status.into_iter() {
			let pallet_name_b = BoundedVec::<u8, ConstU32<MAX_STR_LENGTH>>::try_from(pallet_name.clone());
			let function_name_b = BoundedVec::<u8, ConstU32<MAX_STR_LENGTH>>::try_from(function_name.clone());
			if pallet_name_b.is_err() || function_name_b.is_err() {
				log::info!(
					target: TARGET,
					"Value not migrated because it's too long: {:?}",
					(pallet_name_b, function_name_b)
				);
				continue;
			}

			PausedTransactions::<T>::insert((pallet_name_b.unwrap(), function_name_b.unwrap()), ());
		}

		StorageVersion::new(1).put::<Pallet<T>>();

		T::DbWeight::get().reads_writes(1, 1)
	}

	pub fn post_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");

		log::info!(target: TARGET, "Transaction pause migration: POST checks successful!");
	}
}
