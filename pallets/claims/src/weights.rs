#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for claims.
pub trait WeightInfo {
	fn claim() -> Weight;
}

/// Weights for claims using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `Claims::Claims` (r:1 w:1)
	/// Proof: `Claims::Claims` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `415`
		//  Estimated: `3593`
		// Minimum execution time: 77_185_000 picoseconds.
		Weight::from_parts(77_698_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
}
