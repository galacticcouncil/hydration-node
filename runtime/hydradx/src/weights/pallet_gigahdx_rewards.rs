// This file is part of HydraDX.

// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
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


//! Placeholder weights for `pallet_gigahdx_rewards`.
//!
//! THIS FILE IS A HAND-WRITTEN STUB. It will be replaced by the output of
//! `./bin/hydradx benchmark pallet --pallet pallet_gigahdx_rewards ...` once
//! the benchmark is run on reference hardware. Numbers track
//! `pallet_gigahdx::giga_stake` (the compound path inside `claim_rewards` is
//! essentially `do_stake` plus one `PendingRewards` write and one
//! HDX pot → user transfer).

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;
use crate::*;

/// Weights for `pallet_gigahdx_rewards` using the HydraDX node and recommended hardware.
pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_gigahdx_rewards::WeightInfo for HydraWeight<T> {
	/// Storage: `GigaHdxRewards::PendingRewards` (r:1 w:1)
	/// Storage: `System::Account` (r:1 w:1)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Storage: `Tokens::Accounts` (r:1 w:1)
	/// Storage: `GigaHdx::Stakes` (r:1 w:1)
	/// Storage: `GigaHdx::TotalLocked` (r:1 w:1)
	/// Storage: `GigaHdx::PendingUnstakes` (r:1 w:0)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1200`
		//  Estimated: `4764`
		// Minimum execution time: 140_000_000 picoseconds (placeholder, tracks `giga_stake`).
		Weight::from_parts(140_000_000, 4764)
			.saturating_add(T::DbWeight::get().reads(15_u64))
			.saturating_add(T::DbWeight::get().writes(7_u64))
	}
}
