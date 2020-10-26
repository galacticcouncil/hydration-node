//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	// WARNING! Some components were not used: ["t"]
	fn known_overhead_for_on_finalize() -> Weight {
		(9_906_000 as Weight).saturating_add(DbWeight::get().reads(1 as Weight))
	}
	fn sell_intention() -> Weight {
		(56_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn buy_intention() -> Weight {
		(55_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn on_finalize(t: u32) -> Weight {
		(37_982_000 as Weight)
			.saturating_add((71_444_000 as Weight).saturating_mul(t as Weight))
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().reads((2 as Weight).saturating_mul(t as Weight)))
			.saturating_add(DbWeight::get().writes(3 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(t as Weight)))
	}
	fn on_finalize_buys_no_matches(t: u32) -> Weight {
		(26_588_000 as Weight)
			.saturating_add((64_431_000 as Weight).saturating_mul(t as Weight))
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().reads((2 as Weight).saturating_mul(t as Weight)))
			.saturating_add(DbWeight::get().writes(3 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(t as Weight)))
	}
	fn on_finalize_sells_no_matches(t: u32) -> Weight {
		(35_977_000 as Weight)
			.saturating_add((53_001_000 as Weight).saturating_mul(t as Weight))
			.saturating_add(DbWeight::get().reads(8 as Weight))
			.saturating_add(DbWeight::get().reads((2 as Weight).saturating_mul(t as Weight)))
			.saturating_add(DbWeight::get().writes(3 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(t as Weight)))
	}
	fn sell_extrinsic() -> Weight {
		(85_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
	fn on_finalize_for_one_sell_extrinsic() -> Weight {
		(124_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn buy_extrinsic() -> Weight {
		(85_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
	fn on_finalize_for_one_buy_extrinsic() -> Weight {
		(136_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(13 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
}
