#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_dca.
pub trait WeightInfo {
	fn on_initialize_with_buy_trade() -> Weight;
	fn on_initialize_with_buy_trade_with_insufficient_fee_asset() -> Weight;
	fn on_initialize_with_sell_trade() -> Weight;
	fn on_initialize_with_sell_trade_with_insufficient_fee_asset() -> Weight;
	fn on_initialize_with_empty_block() -> Weight;
	fn schedule() -> Weight;
	fn terminate() -> Weight;
}

/// Weights for pallet_dca using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:12 w:2)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	/// Storage: `DCA::Schedules` (r:1 w:0)
	/// Proof: `DCA::Schedules` (`max_values`: None, `max_size`: Some(191), added: 2666, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RemainingAmounts` (r:1 w:1)
	/// Proof: `DCA::RemainingAmounts` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Reserves` (r:1 w:1)
	/// Proof: `Balances::Reserves` (`max_values`: None, `max_size`: Some(1249), added: 3724, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RetriesOnError` (r:0 w:1)
	/// Proof: `DCA::RetriesOnError` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	fn on_initialize_with_buy_trade() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `54972`
		//  Estimated: `31902`
		// Minimum execution time: 242_707_000 picoseconds.
		Weight::from_parts(246_178_000, 31902)
			.saturating_add(RocksDbWeight::get().reads(17_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:12 w:2)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	/// Storage: `DCA::Schedules` (r:1 w:0)
	/// Proof: `DCA::Schedules` (`max_values`: None, `max_size`: Some(191), added: 2666, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:2 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::NextAssetId` (r:1 w:0)
	/// Proof: `AssetRegistry::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::LocationAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::LocationAssets` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `Router::Routes` (r:1 w:0)
	/// Proof: `Router::Routes` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:4 w:4)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RemainingAmounts` (r:1 w:1)
	/// Proof: `DCA::RemainingAmounts` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Reserves` (r:1 w:1)
	/// Proof: `Tokens::Reserves` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::WhitelistedAssets` (r:1 w:0)
	/// Proof: `EmaOracle::WhitelistedAssets` (`max_values`: Some(1), `max_size`: Some(641), added: 1136, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RetriesOnError` (r:0 w:1)
	/// Proof: `DCA::RetriesOnError` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	fn on_initialize_with_buy_trade_with_insufficient_fee_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `60856`
		//  Estimated: `31902`
		// Minimum execution time: 422_463_000 picoseconds.
		Weight::from_parts(427_199_000, 31902)
			.saturating_add(RocksDbWeight::get().reads(37_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:12 w:2)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	/// Storage: `DCA::Schedules` (r:1 w:0)
	/// Proof: `DCA::Schedules` (`max_values`: None, `max_size`: Some(191), added: 2666, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RemainingAmounts` (r:1 w:1)
	/// Proof: `DCA::RemainingAmounts` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Reserves` (r:1 w:1)
	/// Proof: `Balances::Reserves` (`max_values`: None, `max_size`: Some(1249), added: 3724, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RetriesOnError` (r:0 w:1)
	/// Proof: `DCA::RetriesOnError` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	fn on_initialize_with_sell_trade() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `54720`
		//  Estimated: `31902`
		// Minimum execution time: 247_056_000 picoseconds.
		Weight::from_parts(250_269_000, 31902)
			.saturating_add(RocksDbWeight::get().reads(17_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:12 w:2)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	/// Storage: `DCA::Schedules` (r:1 w:0)
	/// Proof: `DCA::Schedules` (`max_values`: None, `max_size`: Some(191), added: 2666, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:2 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::NextAssetId` (r:1 w:0)
	/// Proof: `AssetRegistry::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::LocationAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::LocationAssets` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `Router::Routes` (r:1 w:0)
	/// Proof: `Router::Routes` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:4 w:4)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RemainingAmounts` (r:1 w:1)
	/// Proof: `DCA::RemainingAmounts` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Reserves` (r:1 w:1)
	/// Proof: `Tokens::Reserves` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::WhitelistedAssets` (r:1 w:0)
	/// Proof: `EmaOracle::WhitelistedAssets` (`max_values`: Some(1), `max_size`: Some(641), added: 1136, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RetriesOnError` (r:0 w:1)
	/// Proof: `DCA::RetriesOnError` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	fn on_initialize_with_sell_trade_with_insufficient_fee_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `61732`
		//  Estimated: `31902`
		// Minimum execution time: 422_605_000 picoseconds.
		Weight::from_parts(426_343_000, 31902)
			.saturating_add(RocksDbWeight::get().reads(37_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:1 w:0)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	fn on_initialize_with_empty_block() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1113`
		//  Estimated: `3566`
		// Minimum execution time: 20_282_000 picoseconds.
		Weight::from_parts(20_552_000, 3566)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
	}
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:2 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::NextAssetId` (r:1 w:0)
	/// Proof: `AssetRegistry::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::LocationAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::LocationAssets` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `Router::Routes` (r:1 w:0)
	/// Proof: `Router::Routes` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:3 w:1)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `DCA::ScheduleIdSequencer` (r:1 w:1)
	/// Proof: `DCA::ScheduleIdSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Reserves` (r:1 w:1)
	/// Proof: `Tokens::Reserves` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:11 w:1)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RetriesOnError` (r:0 w:1)
	/// Proof: `DCA::RetriesOnError` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	/// Storage: `DCA::Schedules` (r:0 w:1)
	/// Proof: `DCA::Schedules` (`max_values`: None, `max_size`: Some(191), added: 2666, mode: `MaxEncodedLen`)
	/// Storage: `DCA::ScheduleOwnership` (r:0 w:1)
	/// Proof: `DCA::ScheduleOwnership` (`max_values`: None, `max_size`: Some(60), added: 2535, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RemainingAmounts` (r:0 w:1)
	/// Proof: `DCA::RemainingAmounts` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn schedule() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `53355`
		//  Estimated: `29326`
		// Minimum execution time: 235_396_000 picoseconds.
		Weight::from_parts(239_535_000, 29326)
			.saturating_add(RocksDbWeight::get().reads(23_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: `DCA::Schedules` (r:1 w:1)
	/// Proof: `DCA::Schedules` (`max_values`: None, `max_size`: Some(191), added: 2666, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RemainingAmounts` (r:1 w:1)
	/// Proof: `DCA::RemainingAmounts` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Reserves` (r:1 w:1)
	/// Proof: `Balances::Reserves` (`max_values`: None, `max_size`: Some(1249), added: 3724, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `DCA::ScheduleIdsPerBlock` (r:1 w:1)
	/// Proof: `DCA::ScheduleIdsPerBlock` (`max_values`: None, `max_size`: Some(101), added: 2576, mode: `MaxEncodedLen`)
	/// Storage: `DCA::RetriesOnError` (r:0 w:1)
	/// Proof: `DCA::RetriesOnError` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	/// Storage: `DCA::ScheduleOwnership` (r:0 w:1)
	/// Proof: `DCA::ScheduleOwnership` (`max_values`: None, `max_size`: Some(60), added: 2535, mode: `MaxEncodedLen`)
	fn terminate() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2530`
		//  Estimated: `4714`
		// Minimum execution time: 87_473_000 picoseconds.
		Weight::from_parts(88_732_000, 4714)
			.saturating_add(RocksDbWeight::get().reads(5_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
}
