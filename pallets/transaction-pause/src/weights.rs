
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for module_transaction_pause.
pub trait WeightInfo {
	fn pause_transaction() -> Weight;
	fn unpause_transaction() -> Weight;
}

/// Weights for module_transaction_pause using the Acala node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `TransactionPause::PausedTransactions` (r:1 w:1)
	/// Proof: `TransactionPause::PausedTransactions` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	fn pause_transaction() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `109`
		//  Estimated: `3555`
		// Minimum execution time: 13_268_000 picoseconds.
		Weight::from_parts(13_653_000, 3555)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `TransactionPause::PausedTransactions` (r:1 w:1)
	/// Proof: `TransactionPause::PausedTransactions` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	fn unpause_transaction() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `160`
		//  Estimated: `3555`
		// Minimum execution time: 16_169_000 picoseconds.
		Weight::from_parts(16_547_000, 3555)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
