// SPDX-License-Identifier: Apache-2.0

//! Placeholder weights for `pallet-gigahdx-rewards`. Replaced by
//! `cargo run --release --features runtime-benchmarks -- benchmark pallet ...`
//! during runtime upgrades.
//!
//! Magnitude tracks `pallet-gigahdx::giga_stake` (the compound path inside
//! `claim_rewards` is essentially a `do_stake` call plus one extra
//! `PendingRewards` write and one HDX pot → user transfer).

use frame_support::weights::{constants::RocksDbWeight, Weight};

pub trait WeightInfo {
	fn claim_rewards() -> Weight;
}

impl WeightInfo for () {
	fn claim_rewards() -> Weight {
		// Ballpark: giga_stake (~122ms, 13 reads, 5 writes) + PendingRewards take
		// (1 read, 1 write) + System::Account transfer (1 read, 1 write).
		Weight::from_parts(140_000_000, 4764)
			.saturating_add(RocksDbWeight::get().reads(15_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
}
