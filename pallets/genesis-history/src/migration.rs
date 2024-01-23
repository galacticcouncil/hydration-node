// This file is part of pallet-genesis-history

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

pub use crate::*;
pub use frame_support::{
	traits::{Get, StorageVersion},
	weights::Weight,
};

// This migration fixes the corrupted value in the storage and fixes it
pub mod v1 {
	use super::*;
	use hex_literal::hex;

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

		log::info!(
			target: "runtime::genesis-history",
			"Genesis history migration: PRE checks successful!"
		);
	}

	pub fn migrate<T: Config>() -> Weight {
		log::info!(
			target: "runtime::genesis-history",
			"Running migration to v1 for Genesis history"
		);

		PreviousChain::<T>::put(Chain {
			genesis_hash: H256::from(hex!("d2a620c27ec5cbc5621ff9a522689895074f7cca0d08e7134a7804e1a3ba86fc")),
			last_block_hash: H256::from(hex!("1c83220b0d0c0c252dddd6a98c9b643e34625419b646c4a6447583c92c01dbcc")),
		});

		StorageVersion::new(1).put::<Pallet<T>>();

		T::DbWeight::get().reads_writes(0, 2)
	}

	pub fn post_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");
		assert_eq!(
			PreviousChain::<T>::get(),
			Chain {
				genesis_hash: H256::from(hex!("d2a620c27ec5cbc5621ff9a522689895074f7cca0d08e7134a7804e1a3ba86fc")),
				last_block_hash: H256::from(hex!("1c83220b0d0c0c252dddd6a98c9b643e34625419b646c4a6447583c92c01dbcc")),
			},
			"Unexpected storage version."
		);

		log::info!(
			target: "runtime::genesis-history",
			"Genesis history migration: POST checks successful!"
		);
	}
}
