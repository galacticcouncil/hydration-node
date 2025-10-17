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

use crate::{
	AssetDetails,
	AssetType,
	Assets,
	Balance,
	Config,
	Pallet,
};
use frame_support::{
	traits::{
		Get,
		StorageVersion,
	},
	weights::Weight,
	Twox64Concat,
};

use crate::*;

pub mod v1 {
	use super::*;
	use codec::{
		Decode,
		Encode,
	};
	use frame_support::storage_alias;
	use scale_info::TypeInfo;
	use sp_core::RuntimeDebug;
	use sp_runtime::BoundedVec;

	#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum AssetType<AssetId> {
		Token,
		PoolShare(AssetId, AssetId), // Use XYX instead
		XYK,
		StableSwap,
		Bond,
		External,
	}

	#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo)]
	pub struct AssetDetails<AssetId, Balance, BoundedString> {
		pub name: BoundedString,
		pub asset_type: AssetType<AssetId>,
		pub existential_deposit: Balance,
		pub xcm_rate_limit: Option<Balance>,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, RuntimeDebug, TypeInfo)]
	pub struct AssetMetadata<BoundedString> {
		pub(super) symbol: BoundedString,
		pub(super) decimals: u8,
	}

	#[storage_alias]
	pub type Assets<T: Config> = StorageMap<
		Pallet<T>,
		Twox64Concat,
		<T as crate::Config>::AssetId,
		AssetDetails<<T as crate::Config>::AssetId, Balance, BoundedVec<u8, <T as crate::Config>::StringLimit>>,
		OptionQuery,
	>;

	#[storage_alias]
	pub type AssetMetadataMap<T: Config> = StorageMap<
		Pallet<T>,
		Twox64Concat,
		<T as crate::Config>::AssetId,
		AssetMetadata<BoundedVec<u8, <T as crate::Config>::StringLimit>>,
		OptionQuery,
	>;

	#[storage_alias]
	pub type AssetLocations<T: Config> = StorageMap<
		Pallet<T>,
		Twox64Concat,
		<T as crate::Config>::AssetId,
		<T as crate::Config>::AssetNativeLocation,
		OptionQuery,
	>;
}

pub mod v2 {
	use super::*;

	impl From<v1::AssetType<u32>> for AssetType {
		fn from(value: v1::AssetType<u32>) -> Self {
			match value {
				v1::AssetType::Token => Self::Token,
				v1::AssetType::PoolShare(_, _) => Self::XYK,
				v1::AssetType::XYK => Self::XYK,
				v1::AssetType::StableSwap => Self::StableSwap,
				v1::AssetType::Bond => Self::Bond,
				v1::AssetType::External => Self::External,
			}
		}
	}
	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Storage version too high.");

		log::info!(
			target: "runtime::asset-registry",
			"Asset Registry migration: PRE checks successful!"
		);
	}

	pub fn migrate<T: Config<AssetId = u32>>() -> Weight {
		if StorageVersion::get::<Pallet<T>>() != 1 {
			log::info!(
				target: "runtime::asset-registry",
				"Nothing to migratte, pallet's version is not 1."
			);
			return T::DbWeight::get().reads_writes(1, 0);
		}

		log::info!(
			target: "runtime::asset-registry",
			"Running migration to v2 for Asset Registry"
		);

		log::info!(
			target: "runtime::asset-registry",
			"Migrating Assets storage"
		);

		let mut reads = 0;
		let mut writes = 0;

		let mut v2_assets_details = Vec::<(
			<T as crate::Config>::AssetId,
			AssetDetails<<T as crate::Config>::StringLimit>,
		)>::new();

		let mut assets_count = 0;
		for (k, v) in v1::Assets::<T>::iter() {
			assets_count += 1;
			let (symbol, decimals) = if let Some(meta) = v1::AssetMetadataMap::<T>::get(k) {
				(Some(meta.symbol), Some(meta.decimals))
			} else {
				(None, None)
			};

			v2_assets_details.push((
				k,
				AssetDetails {
					name: Some(v.name),
					asset_type: v.asset_type.into(),
					existential_deposit: v.existential_deposit,
					symbol,
					decimals,
					xcm_rate_limit: v.xcm_rate_limit,
					//All assets created before this are sufficient
					is_sufficient: true,
				},
			));
		}
		reads += assets_count;

		let _ = v1::Assets::<T>::clear(u32::MAX, None);
		writes += assets_count;

		for (k, v) in v2_assets_details {
			writes += 1;
			Assets::<T>::insert(k, v);
			log::info!(
				target: "runtime::asset-registry",
				"Migrated asset: {:?}", k
			);
		}

		//This assumes every asset has metadata and each metadata is touched.
		let _ = v1::AssetMetadataMap::<T>::clear(u32::MAX, None);
		writes += assets_count;

		log::info!(
			target: "runtime::asset-registry",
			"Migrating AssetLocations storage"
		);

		for k in v1::AssetLocations::<T>::iter_keys() {
			reads += 1;
			writes += 1;

			AssetLocations::<T>::migrate_key::<Twox64Concat, <T as crate::Config>::AssetId>(k);

			log::info!(
				target: "runtime::asset-registry",
				"Migrated asset's location: {:?}", k
			);
		}

		StorageVersion::new(2).put::<Pallet<T>>();
		T::DbWeight::get().reads_writes(reads, writes)
	}

	pub fn post_migrate<T: Config>() {
		for a in Assets::<T>::iter_keys() {
			let _ = Assets::<T>::get(a).expect("Assets data must be valid");
		}

		for l in AssetLocations::<T>::iter_keys() {
			let _ = AssetLocations::<T>::get(l).expect("AssetLocations data must be valid");
		}

		assert_eq!(v1::AssetMetadataMap::<T>::iter().count(), 0);

		assert_eq!(StorageVersion::get::<Pallet<T>>(), 2, "Unexpected storage version.");

		log::info!(
			target: "runtime::asset-registry",
			"Asset Registry migration: POST checks successful!"
		);
	}
}
