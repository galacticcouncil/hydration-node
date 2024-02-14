// This file is part of pallet-collator-rewards

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
	traits::{Get, StorageVersion},
	weights::Weight,
};

/// The log target.
const TARGET: &str = "runtime::collator-rewards::migration::v1";

pub mod v0 {
	use super::*;
	use frame_support::{pallet_prelude::ValueQuery, Twox64Concat};

	#[storage_alias]
	pub type Collators<T: Config> =
		StorageMap<Pallet<T>, Twox64Concat, SessionIndex, Vec<<T as frame_system::Config>::AccountId>, ValueQuery>;
}
pub mod v1 {
	use super::*;

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

		log::info!(target: TARGET, "Collator rewards migration: PRE checks successful!");
	}

	pub fn migrate<T: Config>() -> Weight {
		log::info!(target: TARGET, "Collator rewards to v1 for Transaction pause");

		let mut weight = Weight::zero();

		Collators::<T>::translate::<Vec<<T as frame_system::Config>::AccountId>, _>(|session_index, old_value| {
			let maybe_collators_b =
				BoundedVec::<<T as frame_system::Config>::AccountId, T::MaxCandidates>::try_from(old_value.clone());
			match maybe_collators_b {
				Ok(collators_b) => {
					weight.saturating_accrue(T::DbWeight::get().writes(1));
					Some(collators_b)
				}
				Err(_) => {
					log::info!(
						target: TARGET,
						"Value not migrated because it's too long: {:?}",
						(session_index, old_value)
					);
					None
				}
			}
		});

		StorageVersion::new(1).put::<Pallet<T>>();
		weight.saturating_add(T::DbWeight::get().writes(1))
	}

	pub fn post_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");

		log::info!(target: TARGET, "Collator rewards migration: POST checks successful!");
	}
}
