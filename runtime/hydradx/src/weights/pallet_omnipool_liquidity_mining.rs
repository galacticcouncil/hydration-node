// This file is part of HydraDX.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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


//! Autogenerated weights for `pallet_omnipool_liquidity_mining`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 43.0.0
//! DATE: 2025-04-28, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `bench-bot`, CPU: `Intel(R) Core(TM) i7-7700K CPU @ 4.20GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `None`, DB CACHE: `1024`

// Executed Command:
// ./bin/hydradx
// benchmark
// pallet
// --wasm-execution=compiled
// --pallet
// pallet_omnipool_liquidity_mining
// --extrinsic
// *
// --heap-pages
// 4096
// --steps
// 50
// --repeat
// 20
// --template=scripts/pallet-weight-template.hbs
// --output
// runtime/hydradx/src/weights/pallet_omnipool_liquidity_mining.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weights for `pallet_omnipool_liquidity_mining`.
pub struct WeightInfo<T>(PhantomData<T>);

/// Weights for `pallet_omnipool_liquidity_mining` using the HydraDX node and recommended hardware.
pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_omnipool_liquidity_mining::WeightInfo for HydraWeight<T> {
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::FarmSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::FarmSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:0 w:1)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	fn create_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2129`
		//  Estimated: `6196`
		// Minimum execution time: 106_314_000 picoseconds.
		Weight::from_parts(107_747_000, 6196)
			.saturating_add(T::DbWeight::get().reads(5_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn update_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6645`
		//  Estimated: `6294`
		// Minimum execution time: 154_534_000 picoseconds.
		Weight::from_parts(155_588_000, 6294)
			.saturating_add(T::DbWeight::get().reads(6_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:1)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	fn terminate_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5901`
		//  Estimated: `6196`
		// Minimum execution time: 118_286_000 picoseconds.
		Weight::from_parts(119_492_000, 6196)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::FarmSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::FarmSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	fn create_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7141`
		//  Estimated: `6294`
		// Minimum execution time: 175_913_000 picoseconds.
		Weight::from_parts(177_543_000, 6294)
			.saturating_add(T::DbWeight::get().reads(9_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:0)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn update_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7647`
		//  Estimated: `6294`
		// Minimum execution time: 179_838_000 picoseconds.
		Weight::from_parts(180_969_000, 6294)
			.saturating_add(T::DbWeight::get().reads(9_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn stop_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7220`
		//  Estimated: `6294`
		// Minimum execution time: 169_913_000 picoseconds.
		Weight::from_parts(171_097_000, 6294)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn resume_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7764`
		//  Estimated: `6294`
		// Minimum execution time: 177_074_000 picoseconds.
		Weight::from_parts(178_639_000, 6294)
			.saturating_add(T::DbWeight::get().reads(9_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:0)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn terminate_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6269`
		//  Estimated: `6196`
		// Minimum execution time: 112_800_000 picoseconds.
		Weight::from_parts(114_692_000, 6196)
			.saturating_add(T::DbWeight::get().reads(5_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	fn deposit_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `10537`
		//  Estimated: `11598`
		// Minimum execution time: 272_521_000 picoseconds.
		Weight::from_parts(274_996_000, 11598)
			.saturating_add(T::DbWeight::get().reads(17_u64))
			.saturating_add(T::DbWeight::get().writes(14_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:1 w:0)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn redeposit_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `13539`
		//  Estimated: `11598`
		// Minimum execution time: 237_014_000 picoseconds.
		Weight::from_parts(238_657_000, 11598)
			.saturating_add(T::DbWeight::get().reads(15_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:3)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `10785`
		//  Estimated: `8799`
		// Minimum execution time: 229_155_000 picoseconds.
		Weight::from_parts(230_592_000, 8799)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:1 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:3)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:2)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn withdraw_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `9241`
		//  Estimated: `8799`
		// Minimum execution time: 321_540_000 picoseconds.
		Weight::from_parts(322_747_000, 8799)
			.saturating_add(T::DbWeight::get().reads(15_u64))
			.saturating_add(T::DbWeight::get().writes(15_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:6 w:6)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `18785 + c * (507 ±0)`
		//  Estimated: `11598 + c * (2680 ±0)`
		// Minimum execution time: 278_298_000 picoseconds.
		Weight::from_parts(176_106_420, 11598)
			// Standard Error: 55_096
			.saturating_add(Weight::from_parts(105_487_775, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(14_u64))
			.saturating_add(T::DbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(T::DbWeight::get().writes(11_u64))
			.saturating_add(T::DbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
	/// Storage: `AssetRegistry::Assets` (r:4 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:4 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:5 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:1 w:1)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::NextPositionId` (r:1 w:1)
	/// Proof: `Omnipool::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:2)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:2 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:7 w:7)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:0 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn add_liquidity_and_join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `20294 + c * (507 ±0)`
		//  Estimated: `14250 + c * (2680 ±0)`
		// Minimum execution time: 581_705_000 picoseconds.
		Weight::from_parts(478_307_602, 14250)
			// Standard Error: 146_548
			.saturating_add(Weight::from_parts(109_071_545, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(35_u64))
			.saturating_add(T::DbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(T::DbWeight::get().writes(24_u64))
			.saturating_add(T::DbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:1 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:7 w:7)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:2)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn exit_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `11264 + c * (518 ±0)`
		//  Estimated: `6294 + c * (2680 ±0)`
		// Minimum execution time: 280_522_000 picoseconds.
		Weight::from_parts(120_221_173, 6294)
			// Standard Error: 269_274
			.saturating_add(Weight::from_parts(162_898_545, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(T::DbWeight::get().writes(2_u64))
			.saturating_add(T::DbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
	/// Storage: `Stableswap::Pools` (r:1 w:0)
	/// Proof: `Stableswap::Pools` (`max_values`: None, `max_size`: Some(57), added: 2532, mode: `MaxEncodedLen`)
	/// Storage: `Stableswap::AssetTradability` (r:5 w:0)
	/// Proof: `Stableswap::AssetTradability` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:8 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:13 w:13)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:2 w:2)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `Stableswap::PoolPegs` (r:1 w:0)
	/// Proof: `Stableswap::PoolPegs` (`max_values`: None, `max_size`: Some(351), added: 2826, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:7 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:7 w:7)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:5 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:1 w:1)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::NextPositionId` (r:1 w:1)
	/// Proof: `Omnipool::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:2)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:2 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:0 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn add_liquidity_stableswap_omnipool_and_join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `22745 + c * (507 ±0)`
		//  Estimated: `34569 + c * (2680 ±0)`
		// Minimum execution time: 1_757_692_000 picoseconds.
		Weight::from_parts(1_670_182_766, 34569)
			// Standard Error: 193_366
			.saturating_add(Weight::from_parts(105_264_963, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(62_u64))
			.saturating_add(T::DbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(T::DbWeight::get().writes(35_u64))
			.saturating_add(T::DbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
}