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
			crate::AccountWhitelist::<T>::insert(account_id, ());
			writes += 1;
		});

		log::info!(
			target: "runtime::duster",
			"Updating DustAccount"
		);
		reads += 1;
		writes += 1;

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

pub mod v2 {
	use super::*;
	use frame_support::traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion, UncheckedOnRuntimeUpgrade};
	use frame_support::storage_alias;
	use frame_support::Blake2_128Concat;
	use frame_support::migrations::VersionedMigration;

	#[storage_alias]
	type AccountBlacklist<T: Config, P: GetStorageVersion + PalletInfoAccess> = StorageMap<P, Blake2_128Concat, <T as frame_system::Config>::AccountId, (), frame_support::pallet_prelude::OptionQuery>;

	// Private module to hide the migration.
	mod unversioned {
		use frame_support::pallet_prelude::{GetStorageVersion, PalletInfoAccess};

		pub struct InnerMigrateV1ToV2<T: crate::Config, P: GetStorageVersion + PalletInfoAccess>(core::marker::PhantomData<(T, P)>);
	}

	impl<T: Config, P: GetStorageVersion + PalletInfoAccess> UncheckedOnRuntimeUpgrade for unversioned::InnerMigrateV1ToV2<T, P> {
		fn on_runtime_upgrade() -> Weight {
			log::info!(
				target: "runtime::duster",
				"Running migration to v2 for Duster - renaming AccountBlacklist to AccountWhitelist"
			);

			let mut reads: u64 = 0;
			let mut writes: u64 = 0;

			// Iterate over all entries in the old AccountBlacklist storage
			let blacklisted_accounts: Vec<_> = AccountBlacklist::<T, P>::iter().collect();
			reads += blacklisted_accounts.len() as u64;

			log::info!(
				target: "runtime::duster",
				"Migrating {} accounts from AccountBlacklist to AccountWhitelist",
				blacklisted_accounts.len()
			);

			// Insert all entries into the new AccountWhitelist storage
			for (account_id, _) in &blacklisted_accounts {
				crate::AccountWhitelist::<T>::insert(account_id, ());
				writes += 1;
			}

			// Remove all entries from the old storage
			let removed_keys = AccountBlacklist::<T, P>::drain().count();
			writes += removed_keys as u64;

			log::info!(
				target: "runtime::duster",
				"Migration completed: {} accounts migrated, {} old entries removed",
				blacklisted_accounts.len(),
				removed_keys
			);

			T::DbWeight::get().reads_writes(reads, writes)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
			// Verify that old storage is empty
			assert_eq!(AccountBlacklist::<T, P>::iter().count(), 0, "Old AccountBlacklist storage should be empty");

			// Verify that new storage has the expected entries
			let whitelist_count = crate::AccountWhitelist::<T>::iter().count();
			log::info!(
				target: "runtime::duster",
				"Duster v2 migration: POST checks successful! AccountWhitelist has {} entries",
				whitelist_count
			);
			Ok(())
		}
	}

	pub type MigrateV1ToV2<T> =
		VersionedMigration<1, 2, unversioned::InnerMigrateV1ToV2<T, crate::Pallet<T>>, crate::Pallet<T>, <T as frame_system::Config>::DbWeight>;
}
