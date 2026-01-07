// Copyright (C) 2020-2026 Intergalactic, Limited (GIB).
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

use codec::{Decode, Encode, MaxEncodedLen};
use core::convert::TryFrom;
use frame_support::pallet_prelude::OptionQuery;
use frame_support::{storage_alias, traits::OnRuntimeUpgrade, weights::Weight, Blake2_128Concat};
use pallet_asset_registry::*;
use scale_info::TypeInfo;
use sp_core::Get;
use sp_std::vec::Vec;

use polkadot_xcm::v3::MultiLocation as V3MultiLocation;
use polkadot_xcm::v5::Location as V5Location;
use polkadot_xcm::VersionedLocation;

/// Maximum number of records to migrate (safety limit for single-block migrations)
pub const MAX_RECORDS_TO_MIGRATE: u64 = 100;

/// Old AssetLocation wrapper type that matches the on-chain encoding
/// The old type was: `pub struct AssetLocation(pub MultiLocation);`
#[derive(Debug, Default, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct OldAssetLocation(pub V3MultiLocation);

/// Module containing old storage type aliases with correct storage names
mod old_storage {
	use super::*;

	#[storage_alias]
	pub type AssetLocations<T: pallet::Config> =
		StorageMap<pallet::Pallet<T>, Blake2_128Concat, <T as pallet::Config>::AssetId, OldAssetLocation, OptionQuery>;

	#[storage_alias]
	pub type LocationAssets<T: pallet::Config> =
		StorageMap<pallet::Pallet<T>, Blake2_128Concat, OldAssetLocation, <T as pallet::Config>::AssetId, OptionQuery>;
}

// This migration re-encodes AssetLocations storage to ensure proper v5 Location encoding.
//
// Even though v3 MultiLocation and v5 Location have compatible SCALE encodings for most cases,
// we want to ensure the data is stored with the canonical v5 encoding.
//
// The migration does not use a StorageVersion, make sure it is removed from the Runtime Executive
// after it has been run.
pub struct MigrateAssetRegistryToXcmV5<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MigrateAssetRegistryToXcmV5<T>
where
	T::AssetNativeLocation: TryFrom<V5Location>,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			target: "asset-registry",
			"MigrateAssetRegistryToXcmV5 started..."
		);

		let mut reads = 0u64;
		let mut writes = 0u64;
		let mut migrated_count = 0u64;

		// Use the old storage type to drain all entries
		let old_locations: Vec<(T::AssetId, OldAssetLocation)> = old_storage::AssetLocations::<T>::drain().collect();

		log::info!(
			target: "asset-registry",
			"Found {} locations to migrate (limit: {})",
			old_locations.len(),
			MAX_RECORDS_TO_MIGRATE
		);

		// Clear the old reverse map
		let _ = old_storage::LocationAssets::<T>::clear(u32::MAX, None);

		for (asset_id, old_loc) in old_locations {
			reads += 1;

			// Check if we've reached the migration limit
			if migrated_count >= MAX_RECORDS_TO_MIGRATE {
				log::warn!(
					target: "asset-registry",
					"Migration limit reached ({} records). Remaining assets will be skipped.",
					MAX_RECORDS_TO_MIGRATE
				);
				break;
			}

			// Convert v3 MultiLocation -> v5 Location via VersionedLocation
			let versioned = VersionedLocation::V3(old_loc.0);
			let v5_location: V5Location = match versioned.try_into() {
				Ok(loc) => loc,
				Err(_) => {
					log::error!(
						target: "asset-registry",
						"Asset {:?}: Failed to convert v3->v5, skipping",
						asset_id
					);
					continue;
				}
			};

			// Create new AssetNativeLocation from the v5 location
			let new_loc: T::AssetNativeLocation = match v5_location.clone().try_into() {
				Ok(loc) => loc,
				Err(_) => {
					log::error!(
						target: "asset-registry",
						"Asset {:?}: Failed to create AssetNativeLocation from V5Location, skipping",
						asset_id
					);
					continue;
				}
			};

			// Write back with proper v5 encoding
			AssetLocations::<T>::insert(asset_id, &new_loc);
			writes += 1;

			// Update reverse map with new encoding
			LocationAssets::<T>::insert(&new_loc, asset_id);
			writes += 1;

			migrated_count += 1;

			log::info!(
				target: "asset-registry",
				"Asset {:?}: re-encoded location to v5 ({}/{})",
				asset_id,
				migrated_count,
				MAX_RECORDS_TO_MIGRATE
			);
		}

		log::info!(
			target: "asset-registry",
			"MigrateAssetRegistryToXcmV5 finished — {} records migrated, {} reads, {} writes",
			migrated_count,
			reads,
			writes
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Runtime;
	use frame_support::traits::OnRuntimeUpgrade;
	use polkadot_xcm::v3::MultiLocation as V3MultiLocation;
	use sp_io::TestExternalities;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> TestExternalities {
		frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap()
			.into()
	}

	fn mock_old_location_parent() -> OldAssetLocation {
		OldAssetLocation(V3MultiLocation {
			parents: 1,
			interior: polkadot_xcm::v3::Junctions::Here,
		})
	}

	fn mock_old_location_with_index(parents: u8, index: u128) -> OldAssetLocation {
		OldAssetLocation(V3MultiLocation {
			parents,
			interior: polkadot_xcm::v3::Junctions::X1(polkadot_xcm::v3::Junction::GeneralIndex(index)),
		})
	}

	fn mock_old_location_parachain(para_id: u32) -> OldAssetLocation {
		OldAssetLocation(V3MultiLocation {
			parents: 1,
			interior: polkadot_xcm::v3::Junctions::X1(polkadot_xcm::v3::Junction::Parachain(para_id)),
		})
	}

	#[test]
	fn migration_converts_v3_parent_location_to_v5() {
		new_test_ext().execute_with(|| {
			let asset_id: u32 = 1;
			let old_location = mock_old_location_parent();

			// Insert using the old storage type (OldAssetLocation wrapping v3 MultiLocation)
			old_storage::AssetLocations::<Runtime>::insert(asset_id, old_location.clone());
			assert!(old_storage::AssetLocations::<Runtime>::contains_key(asset_id));

			// Run migration
			let weight = MigrateAssetRegistryToXcmV5::<Runtime>::on_runtime_upgrade();
			assert!(
				weight.ref_time() > 0,
				"weight should be > 0, meaning migration processed entries"
			);

			// Verify the location was migrated to new storage
			let migrated = AssetLocations::<Runtime>::get(asset_id).expect("should exist after migration");

			// Check the migrated location has correct v5 structure
			assert_eq!(migrated.0.parents, 1);
			assert_eq!(migrated.0.interior, polkadot_xcm::v5::Junctions::Here);

			// Verify reverse mapping was created
			let reverse = LocationAssets::<Runtime>::get(&migrated);
			assert_eq!(reverse, Some(asset_id));
		});
	}

	#[test]
	fn migration_converts_v3_location_with_general_index() {
		new_test_ext().execute_with(|| {
			let asset_id: u32 = 42;
			let old_location = mock_old_location_with_index(0, 999);

			// Insert using the old storage type
			old_storage::AssetLocations::<Runtime>::insert(asset_id, old_location);

			// Run migration
			MigrateAssetRegistryToXcmV5::<Runtime>::on_runtime_upgrade();

			// Verify migration
			let migrated = AssetLocations::<Runtime>::get(asset_id).expect("should exist");
			assert_eq!(migrated.0.parents, 0);

			// Check interior has GeneralIndex(999)
			match &migrated.0.interior {
				polkadot_xcm::v5::Junctions::X1(junctions) => {
					assert_eq!(junctions.len(), 1);
					assert_eq!(junctions[0], polkadot_xcm::v5::Junction::GeneralIndex(999));
				}
				_ => panic!("Expected X1 junction"),
			}

			// Verify reverse mapping
			assert_eq!(LocationAssets::<Runtime>::get(&migrated), Some(asset_id));
		});
	}

	#[test]
	fn migration_converts_v3_parachain_location() {
		new_test_ext().execute_with(|| {
			let asset_id: u32 = 100;
			let old_location = mock_old_location_parachain(2000);

			// Insert using the old storage type
			old_storage::AssetLocations::<Runtime>::insert(asset_id, old_location);

			// Run migration
			MigrateAssetRegistryToXcmV5::<Runtime>::on_runtime_upgrade();

			// Verify migration
			let migrated = AssetLocations::<Runtime>::get(asset_id).expect("should exist");
			assert_eq!(migrated.0.parents, 1);

			match &migrated.0.interior {
				polkadot_xcm::v5::Junctions::X1(junctions) => {
					assert_eq!(junctions[0], polkadot_xcm::v5::Junction::Parachain(2000));
				}
				_ => panic!("Expected X1 junction with Parachain"),
			}

			assert_eq!(LocationAssets::<Runtime>::get(&migrated), Some(asset_id));
		});
	}

	#[test]
	fn migration_handles_multiple_assets() {
		new_test_ext().execute_with(|| {
			// Insert multiple old locations using the old storage type
			let locations = vec![
				(1u32, mock_old_location_parent()),
				(2u32, mock_old_location_with_index(0, 100)),
				(3u32, mock_old_location_parachain(1000)),
			];

			for (asset_id, old_loc) in &locations {
				old_storage::AssetLocations::<Runtime>::insert(*asset_id, old_loc.clone());
			}

			// Run migration
			MigrateAssetRegistryToXcmV5::<Runtime>::on_runtime_upgrade();

			// Verify all assets were migrated
			for (asset_id, _) in &locations {
				assert!(
					AssetLocations::<Runtime>::get(*asset_id).is_some(),
					"Asset {} should be migrated",
					asset_id
				);
				let loc = AssetLocations::<Runtime>::get(*asset_id).unwrap();
				assert_eq!(
					LocationAssets::<Runtime>::get(&loc),
					Some(*asset_id),
					"Reverse mapping for asset {} should exist",
					asset_id
				);
			}
		});
	}

	#[test]
	fn migration_handles_empty_storage() {
		new_test_ext().execute_with(|| {
			// No assets in storage

			// Run migration - should not panic
			let weight = MigrateAssetRegistryToXcmV5::<Runtime>::on_runtime_upgrade();

			// Weight should be zero (no reads/writes)
			assert_eq!(weight.ref_time(), 0);
		});
	}
}
