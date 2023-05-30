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

use crate::{Collections, Config, ItemInfoOf, Items, Pallet};
use frame_support::{
    log,
    traits::{Get, PalletInfoAccess, StorageVersion},
    weights::Weight,
};

/// Storage names are changed from Classes to Collections and from Instances to Items.
pub mod v1 {
    use super::*;
    use frame_support::{
        migration::move_prefix,
        storage::{storage_prefix, unhashed, StoragePrefixedMap},
        storage_alias, Twox64Concat,
    };
    use sp_io::hashing::twox_128;

    #[storage_alias]
    type Classes<T: Config> =
        StorageMap<Pallet<T>, Twox64Concat, <T as Config>::NftCollectionId, crate::CollectionInfoOf<T>>;

    #[storage_alias]
    type Instances<T: Config> = StorageDoubleMap<
        Pallet<T>,
        Twox64Concat,
        <T as Config>::NftCollectionId,
        Twox64Concat,
        <T as Config>::NftItemId,
        ItemInfoOf<T>,
    >;

    pub fn pre_migrate<T: Config>() {
        assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

        log::info!(
            target: "runtime::nft",
            "NFT migration: PRE checks successful!"
        );
    }

    pub fn migrate<T: Config>() -> Weight {
        log::info!(
            target: "runtime::nft",
            "Running migration to v1 for NFT"
        );

        let pallet_name = <Pallet<T> as PalletInfoAccess>::name().as_bytes();

        // move Classes to Collections
        let new_storage_prefix = storage_prefix(pallet_name, Collections::<T>::storage_prefix());
        let old_storage_prefix = storage_prefix(pallet_name, Classes::<T>::storage_prefix());

        // If the number of collections overflows the max weight, return the max weight.
        // Make sure this won't happen by running try-runtime command before executing the migration.
        let num_of_collections = Collections::<T>::iter().count().try_into().unwrap_or(u64::MAX);

        move_prefix(&old_storage_prefix, &new_storage_prefix);
        if let Some(value) = unhashed::get_raw(&old_storage_prefix) {
            unhashed::put_raw(&new_storage_prefix, &value);
            unhashed::kill(&old_storage_prefix);
        }

        // move Instances to Items
        let new_storage_prefix = storage_prefix(pallet_name, Items::<T>::storage_prefix());
        let old_storage_prefix = storage_prefix(pallet_name, Instances::<T>::storage_prefix());

        // If the number of items overflows the max weight, return the max weight.
        // Make sure this won't happen by running try-runtime command before executing the migration.
        let num_of_instances = Items::<T>::iter().count().try_into().unwrap_or(u64::MAX);

        move_prefix(&old_storage_prefix, &new_storage_prefix);
        if let Some(value) = unhashed::get_raw(&old_storage_prefix) {
            unhashed::put_raw(&new_storage_prefix, &value);
            unhashed::kill(&old_storage_prefix);
        }

        StorageVersion::new(1).put::<Pallet<T>>();

        let reads = num_of_collections
            .checked_mul(2)
            .and_then(|v| v.checked_add(num_of_instances.checked_mul(2).unwrap_or(u64::MAX)))
            .and_then(|v| v.checked_add(6))
            .unwrap_or(u64::MAX);
        let writes = num_of_collections
            .checked_add(num_of_instances)
            .and_then(|v| v.checked_add(5))
            .unwrap_or(u64::MAX);

        T::DbWeight::get().reads_writes(reads, writes)
    }

    pub fn post_migrate<T: Config>() {
        assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");

        let pallet_name = <Pallet<T> as PalletInfoAccess>::name().as_bytes();

        // Assert that no `Classes` storage remains at the old prefix.
        let old_storage_prefix = Classes::<T>::storage_prefix();
        let old_key = [&twox_128(pallet_name), &twox_128(old_storage_prefix)[..]].concat();
        let old_key_iter =
            frame_support::storage::KeyPrefixIterator::new(old_key.to_vec(), old_key.to_vec(), |_| Ok(()));
        assert_eq!(old_key_iter.count(), 0);

        // Assert that no `Instances` storage remains at the old prefix.
        let old_storage_prefix = Instances::<T>::storage_prefix();
        let old_key = [&twox_128(pallet_name), &twox_128(old_storage_prefix)[..]].concat();
        let old_key_iter =
            frame_support::storage::KeyPrefixIterator::new(old_key.to_vec(), old_key.to_vec(), |_| Ok(()));
        assert_eq!(old_key_iter.count(), 0);

        log::info!(
            target: "runtime::nft",
            "NFT migration: POST checks successful!"
        );
    }
}
