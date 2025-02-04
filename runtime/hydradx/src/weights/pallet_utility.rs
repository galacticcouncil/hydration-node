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


//! Autogenerated weights for `pallet_utility`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2024-11-14, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `bench-bot`, CPU: `Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: `1024`

// Executed Command:
// target/release/hydradx
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --wasm-execution=compiled
// --pallet=pallet-utility
// --extrinsic=*
// --template=scripts/pallet-weight-template.hbs
// --output=./weights/pallet_utility.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weights for `pallet_utility`.
pub struct WeightInfo<T>(PhantomData<T>);

/// Weights for `pallet_utility` using the HydraDX node and recommended hardware.
pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_utility::WeightInfo for HydraWeight<T> {
	/// Storage: `Broadcast::IncrementalId` (r:1 w:1)
	/// Proof: `Broadcast::IncrementalId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Broadcast::IdStack` (r:1 w:1)
	/// Proof: `Broadcast::IdStack` (`max_values`: Some(1), `max_size`: Some(51), added: 546, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[0, 1000]`.
	fn batch(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `109`
		//  Estimated: `1536`
		// Minimum execution time: 11_565_000 picoseconds.
		Weight::from_parts(1_469_920, 1536)
			// Standard Error: 2_956
			.saturating_add(Weight::from_parts(4_030_477, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	fn as_derivative() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 6_183_000 picoseconds.
		Weight::from_parts(6_383_000, 0)
	}
	/// Storage: `Broadcast::IncrementalId` (r:1 w:1)
	/// Proof: `Broadcast::IncrementalId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Broadcast::IdStack` (r:1 w:1)
	/// Proof: `Broadcast::IdStack` (`max_values`: Some(1), `max_size`: Some(51), added: 546, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[0, 1000]`.
	fn batch_all(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `109`
		//  Estimated: `1536`
		// Minimum execution time: 11_377_000 picoseconds.
		Weight::from_parts(12_184_543, 1536)
			// Standard Error: 2_950
			.saturating_add(Weight::from_parts(4_283_377, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	fn dispatch_as() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_714_000 picoseconds.
		Weight::from_parts(8_898_000, 0)
	}
	/// Storage: `Broadcast::IncrementalId` (r:1 w:1)
	/// Proof: `Broadcast::IncrementalId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Broadcast::IdStack` (r:1 w:1)
	/// Proof: `Broadcast::IdStack` (`max_values`: Some(1), `max_size`: Some(51), added: 546, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[0, 1000]`.
	fn force_batch(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `109`
		//  Estimated: `1536`
		// Minimum execution time: 11_519_000 picoseconds.
		Weight::from_parts(11_090_812, 1536)
			// Standard Error: 2_756
			.saturating_add(Weight::from_parts(4_001_584, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}