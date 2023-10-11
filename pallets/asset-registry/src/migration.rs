// This file is part of pallet-asset-registry

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License..

use crate::{AssetDetails, AssetType, Assets, Balance, Config, Pallet};
use frame_support::{
	log,
	traits::{Get, StorageVersion},
	weights::Weight,
	Twox64Concat,
};

use crate::*;

pub mod v1 {

	use super::*;
	use codec::{Decode, Encode};
	use frame_support::storage_alias;
	use scale_info::TypeInfo;
	use sp_core::RuntimeDebug;
	use sp_runtime::BoundedVec;

	#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo)]
	pub struct OldAssetDetails<AssetId, Balance, BoundedString> {
		pub name: BoundedString,
		pub asset_type: AssetType<AssetId>,
		pub existential_deposit: Balance,
		pub xcm_rate_limit: Option<Balance>,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, RuntimeDebug, TypeInfo)]
	pub struct OldAssetMetadata<BoundedString> {
		pub(super) symbol: BoundedString,
		pub(super) decimals: u8,
	}

	#[storage_alias]
	pub type Assets<T: Config> = StorageMap<
		Pallet<T>,
		Twox64Concat,
		<T as crate::Config>::AssetId,
		OldAssetDetails<<T as crate::Config>::AssetId, Balance, BoundedVec<u8, <T as crate::Config>::StringLimit>>,
		OptionQuery,
	>;

	#[storage_alias]
	pub type AssetMetadataMap<T: Config> = StorageMap<
		Pallet<T>,
		Twox64Concat,
		<T as crate::Config>::AssetId,
		OldAssetMetadata<BoundedVec<u8, <T as crate::Config>::StringLimit>>,
		OptionQuery,
	>;
}

pub mod v2 {
	use super::*;
	use sp_std::vec::Vec;

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Storage version too high.");

		log::info!(
			target: "runtime::asset-registry",
			"Asset Registry migration: PRE checks successful!"
		);
	}

	pub fn migrate<T: Config<AssetId = u32>>() -> Weight {
		log::info!(
			target: "runtime::asset-registry",
			"Running migration to v2 for Asset Registry"
		);

		let mut i = 0;
		let mut details_updated = Vec::<(
			<T as crate::Config>::AssetId,
			AssetDetails<<T as crate::Config>::AssetId, <T as crate::Config>::StringLimit>,
		)>::new();

		for (k, v) in v1::Assets::<T>::iter() {
			log::info!(
				target: "runtime::asset-registry",
				"key: {:?}, name: {:?}", k, v.name
			);

			//insert + old metada read = 2
			i += 2;
			let (symbol, decimals) = if let Some(meta) = v1::AssetMetadataMap::<T>::get(k) {
				(Some(meta.symbol), Some(meta.decimals))
			} else {
				(None, None)
			};

			details_updated.push((
				k,
				AssetDetails {
					name: Some(v.name),
					asset_type: v.asset_type,
					existential_deposit: v.existential_deposit,
					symbol,
					decimals,
					xcm_rate_limit: v.xcm_rate_limit,
					//All assets created until this point are sufficient
					is_sufficient: true,
				},
			));

			//NOTE: problem is with stablepool - probably becasue master is not merged yet
			let _ = Assets::<T>::clear(u32::MAX, None);
			for (k, v) in &details_updated {
				log::info!(
					target: "runtime::asset-registry",
					"Inserting asset: {:?}:", k
				);
				Assets::<T>::insert(k, v);
			}
		}

		StorageVersion::new(1).put::<Pallet<T>>();

		T::DbWeight::get().reads_writes(i, i)
	}

	pub fn post_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 2, "Unexpected storage version.");

		log::info!(
			target: "runtime::asset-registry",
			"Asset Registry migration: POST checks successful!"
		);
	}
}
