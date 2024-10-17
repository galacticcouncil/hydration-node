// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

use super::*;
use frame_support::{
	traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
	weights::Weight,
};

/// Migrate the pallet storage to v1. This migration creates NFT collection for omnipool's
/// liquidity mining.
pub fn migrate_to_v1<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> frame_support::weights::Weight {
	let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
	//offset for storage version read
	let mut weight: Weight = T::DbWeight::get().reads(1);

	log::info!(
		target: "runtime::omnipool-liquidity-mining",
		"Running migration storage v1 for omnipool-liquidity-mining with storage version {:?}",
		on_chain_storage_version,
	);

	if on_chain_storage_version < 1 {
		let pallet_account = <Pallet<T>>::account_id();
		match <T as pallet::Config>::NFTHandler::create_collection(
			&<T as pallet::Config>::NFTCollectionId::get(),
			&pallet_account,
			&pallet_account,
		) {
			Ok(_) => {
				weight = weight
					.saturating_add(T::DbWeight::get().reads(1))
					.saturating_add(T::DbWeight::get().writes(2));

				StorageVersion::new(1).put::<P>();
				//add storage version update weight
				weight = weight.saturating_add(T::DbWeight::get().writes(1));

				log::info!(
					target: "runtime::omnipool-liquidity-mining",
					"Running migration storage v1 for omnipool-liquidity-mining with storage version {:?} was complete",
					on_chain_storage_version,
				);
			}
			Err(e) => {
				log::error!(
					target: "runtime: omnipool-liquidity-mining",
					"Error to create NFT collection: {:?}",
					e
				);
				weight = weight.saturating_add(T::DbWeight::get().reads(1));
			}
		};

		// return migration weights
		weight
	} else {
		log::warn!(
			target: "runtime::omnipool-liquidity-mining",
			"Attempted to apply migration to v1 but failed because storage version is {:?}",
			on_chain_storage_version,
		);
		weight
	}
}
