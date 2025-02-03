#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_ema_oracle.
pub trait WeightInfo {
	fn add_oracle() -> Weight;
	fn remove_oracle() -> Weight;
	fn on_finalize_no_entry() -> Weight;
	fn on_finalize_multiple_tokens(b: u32) -> Weight;
	fn on_trade_multiple_tokens(b: u32) -> Weight;
	fn on_liquidity_changed_multiple_tokens(b: u32) -> Weight;
	fn get_entry() -> Weight;
}

/// Weights for `pallet_ema_oracle` using the HydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `EmaOracle::WhitelistedAssets` (r:1 w:1)
	/// Proof: `EmaOracle::WhitelistedAssets` (`max_values`: Some(1), `max_size`: Some(641), added: 1136, mode: `MaxEncodedLen`)
	fn add_oracle() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `860`
		//  Estimated: `2126`
		// Minimum execution time: 18_453_000 picoseconds.
		Weight::from_parts(18_728_000, 2126)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EmaOracle::WhitelistedAssets` (r:1 w:1)
	/// Proof: `EmaOracle::WhitelistedAssets` (`max_values`: Some(1), `max_size`: Some(641), added: 1136, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:0 w:3)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn remove_oracle() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `876`
		//  Estimated: `7406`
		// Minimum execution time: 35_110_000 picoseconds.
		Weight::from_parts(35_479_000, 7406)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `EmaOracle::Accumulator` (r:1 w:0)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	fn on_finalize_no_entry() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `208`
		//  Estimated: `7406`
		// Minimum execution time: 3_428_000 picoseconds.
		Weight::from_parts(3_551_000, 7406)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
	}
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:117 w:117)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// The range of component `b` is `[1, 39]`.
	fn on_finalize_multiple_tokens(b: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `305 + b * (626 ±0)`
		//  Estimated: `7406 + b * (7956 ±0)`
		// Minimum execution time: 50_796_000 picoseconds.
		Weight::from_parts(13_853_333, 7406)
			// Standard Error: 19_995
			.saturating_add(Weight::from_parts(36_619_947, 0).saturating_mul(b.into()))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().reads((3_u64).saturating_mul(b.into())))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(b.into())))
			.saturating_add(Weight::from_parts(0, 7956).saturating_mul(b.into()))
	}
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// The range of component `b` is `[1, 39]`.
	fn on_trade_multiple_tokens(b: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `821 + b * (164 ±0)`
		//  Estimated: `7406`
		// Minimum execution time: 21_833_000 picoseconds.
		Weight::from_parts(22_673_078, 7406)
			// Standard Error: 3_367
			.saturating_add(Weight::from_parts(503_352, 0).saturating_mul(b.into()))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// The range of component `b` is `[1, 39]`.
	fn on_liquidity_changed_multiple_tokens(b: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `821 + b * (164 ±0)`
		//  Estimated: `7406`
		// Minimum execution time: 22_117_000 picoseconds.
		Weight::from_parts(22_701_600, 7406)
			// Standard Error: 3_410
			.saturating_add(Weight::from_parts(500_409, 0).saturating_mul(b.into()))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn get_entry() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `638`
		//  Estimated: `6294`
		// Minimum execution time: 21_346_000 picoseconds.
		Weight::from_parts(21_721_000, 6294)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
	}
}
