// This file is part of Basilisk-node.

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
// limitations under the License..

use crate::Config;
use frame_support::weights::Weight;
use sp_std::vec::Vec;

pub mod v1 {
	use super::*;
	use frame_support::traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion};

	pub fn pre_migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() {
		let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
		assert_eq!(on_chain_storage_version, 0, "Storage version too high.");

		log::info!(
			target: "runtime::duster",
			"Duster migration: PRE checks successful!"
		);
	}

	pub fn migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>(
		account_blacklist: Vec<T::AccountId>,
		reward_account: T::AccountId,
		dust_account: T::AccountId,
	) -> Weight {
		//offset of storage version updated
		let mut reads: u64 = 1;
		let mut writes: u64 = 1;

		log::info!(
			target: "runtime::duster",
			"Running migration to v1 for Duster"
		);

		log::info!(
			target: "runtime::duster",
			"Updating AccountBlacklist"
		);
		reads += 1;
		account_blacklist.iter().for_each(|account_id| {
			crate::AccountBlacklist::<T>::insert(account_id, ());
			writes += 1;
		});

		log::info!(
			target: "runtime::duster",
			"Updating RewardAccount"
		);
		reads += 1;
		writes += 1;
		crate::RewardAccount::<T>::put(reward_account);

		log::info!(
			target: "runtime::duster",
			"Updating DustAccount"
		);
		reads += 1;
		writes += 1;
		crate::DustAccount::<T>::put(dust_account);

		StorageVersion::new(1).put::<P>();

		T::DbWeight::get().reads_writes(reads, writes)
	}

	pub fn post_migrate<T: Config, P: GetStorageVersion + PalletInfoAccess>() {
		let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
		assert_eq!(on_chain_storage_version, 1, "Unexpected storage version.");

		log::info!(
			target: "runtime::duster",
			"Duster migration: POST checks successful!"
		);
	}
}
