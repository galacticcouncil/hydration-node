#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_liquidation.
pub trait WeightInfo {
	fn liquidate() -> Weight;
	fn set_borrowing_contract() -> Weight;
	fn set_oracle_signers() -> Weight;
	fn set_oracle_call_addresses() -> Weight;
	fn set_unsigned_liquidation_priority() -> Weight;
	fn set_oracle_update_priority() -> Weight;
}
/// Weights for `pallet_liquidation` using the HydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:1 w:1)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Liquidation::BorrowingContract` (r:1 w:0)
	/// Proof: `Liquidation::BorrowingContract` (`max_values`: Some(1), `max_size`: Some(20), added: 515, mode: `MaxEncodedLen`)
	fn liquidate() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `916`
		//  Estimated: `3593`
		// Minimum execution time: 97_283_000 picoseconds.
		Weight::from_parts(97_901_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `Liquidation::BorrowingContract` (r:0 w:1)
	/// Proof: `Liquidation::BorrowingContract` (`max_values`: Some(1), `max_size`: Some(20), added: 515, mode: `MaxEncodedLen`)
	fn set_borrowing_contract() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_117_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn set_oracle_signers() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_117_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn set_oracle_call_addresses() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_117_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn set_unsigned_liquidation_priority() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_117_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn set_oracle_update_priority() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 4_000_000 picoseconds.
		Weight::from_parts(4_117_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
