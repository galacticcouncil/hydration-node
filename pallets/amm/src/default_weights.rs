//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn create_pool() -> Weight {
		(98_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(11 as Weight))
	}
	fn add_liquidity() -> Weight {
		(99_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
	fn remove_liquidity() -> Weight {
		(67_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn sell() -> Weight {
		(74_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
	fn buy() -> Weight {
		(73_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
}
