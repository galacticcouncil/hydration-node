// This file is part of pallet-asset-registry

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

use crate::{AssetDetails, AssetType, Assets, Config, Pallet};
use frame_support::{
	traits::{Get, StorageVersion},
	weights::Weight,
};

///
pub mod v1 {
	use super::*;
	use codec::{Decode, Encode};
	use scale_info::TypeInfo;
	use sp_core::RuntimeDebug;

	#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo)]
	pub struct OldAssetDetails<AssetId, Balance, BoundedString> {
		/// The name of this asset. Limited in length by `StringLimit`.
		pub(super) name: BoundedString,

		pub(super) asset_type: AssetType<AssetId>,

		pub(super) existential_deposit: Balance,

		pub(super) locked: bool,
	}

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

		log::info!(
			target: "runtime::asset-registry",
			"Asset Registry migration: PRE checks successful!"
		);
	}

	pub fn migrate<T: Config>() -> Weight {
		log::info!(
			target: "runtime::asset-registry",
			"Running migration to v1 for Asset Registry"
		);

		let mut i = 0;
		Assets::<T>::translate(
			|_key,
			 OldAssetDetails {
			     name,
			     asset_type,
			     existential_deposit,
			     locked: _,
			 }| {
				i += 1;
				Some(AssetDetails {
					name,
					asset_type,
					existential_deposit,
					xcm_rate_limit: None,
				})
			},
		);

		StorageVersion::new(1).put::<Pallet<T>>();

		T::DbWeight::get().reads_writes(i, i)
	}

	pub fn post_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");

		log::info!(
			target: "runtime::asset-registry",
			"Asset Registry migration: POST checks successful!"
		);
	}
}
