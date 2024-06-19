// This file is part of HydraDX.

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
// limitations under the License.


//! Autogenerated weights for `pallet_staking`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2024-05-29, STEPS: `10`, REPEAT: `30`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `bench-bot`, CPU: `Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: `1024`

// Executed Command:
// target/release/hydradx
// benchmark
// pallet
// --chain=dev
// --steps=10
// --repeat=30
// --wasm-execution=compiled
// --heap-pages=4096
// --template=scripts/pallet-weight-template.hbs
// --pallet=pallet-staking
// --output=weights/staking.rs
// --extrinsic=*

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weights for `pallet_staking`.
pub struct WeightInfo<T>(PhantomData<T>);

/// Weights for `pallet_staking` using the HydraDX node and recommended hardware.
pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_staking::WeightInfo for HydraWeight<T> {
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ClassAccount` (r:0 w:1)
	/// Proof: `Uniques::ClassAccount` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	fn initialize_staking() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `516`
		//  Estimated: `3655`
		// Minimum execution time: 34_969_000 picoseconds.
		Weight::from_parts(35_577_000, 3655)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:1 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Staking::NextPositionId` (r:1 w:1)
	/// Proof: `Staking::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:0 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	fn stake() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1274`
		//  Estimated: `6196`
		// Minimum execution time: 89_837_000 picoseconds.
		Weight::from_parts(90_522_000, 6196)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(8_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Staking::ProcessedVotes` (r:1 w:0)
	/// Proof: `Staking::ProcessedVotes` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:1 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Staking::PositionVotes` (r:1 w:1)
	/// Proof: `Staking::PositionVotes` (`max_values`: None, `max_size`: Some(2134), added: 4609, mode: `MaxEncodedLen`)
	/// Storage: `Democracy::ReferendumInfoOf` (r:100 w:0)
	/// Proof: `Democracy::ReferendumInfoOf` (`max_values`: None, `max_size`: Some(201), added: 2676, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	fn increase_stake() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3379`
		//  Estimated: `268590`
		// Minimum execution time: 260_523_000 picoseconds.
		Weight::from_parts(264_621_000, 268590)
			.saturating_add(T::DbWeight::get().reads(109_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Staking::ProcessedVotes` (r:1 w:0)
	/// Proof: `Staking::ProcessedVotes` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:1 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	/// Storage: `Staking::PositionVotes` (r:1 w:1)
	/// Proof: `Staking::PositionVotes` (`max_values`: None, `max_size`: Some(2134), added: 4609, mode: `MaxEncodedLen`)
	/// Storage: `Democracy::ReferendumInfoOf` (r:100 w:0)
	/// Proof: `Democracy::ReferendumInfoOf` (`max_values`: None, `max_size`: Some(201), added: 2676, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3379`
		//  Estimated: `268590`
		// Minimum execution time: 256_924_000 picoseconds.
		Weight::from_parts(260_188_000, 268590)
			.saturating_add(T::DbWeight::get().reads(109_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:1 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	/// Storage: `Staking::PositionVotes` (r:1 w:1)
	/// Proof: `Staking::PositionVotes` (`max_values`: None, `max_size`: Some(2134), added: 4609, mode: `MaxEncodedLen`)
	/// Storage: `Staking::ProcessedVotes` (r:1 w:0)
	/// Proof: `Staking::ProcessedVotes` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn unstake() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1431`
		//  Estimated: `6196`
		// Minimum execution time: 136_034_000 picoseconds.
		Weight::from_parts(137_189_000, 6196)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(10_u64))
	}
}