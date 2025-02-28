#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for module_currencies.
pub trait WeightInfo {
	fn transfer_non_native_currency() -> Weight;
	fn transfer_native_currency() -> Weight;
	fn update_balance_non_native_currency() -> Weight;
	fn update_balance_native_currency_creating() -> Weight;
	fn update_balance_native_currency_killing() -> Weight;
}

/// Default weights.
impl WeightInfo for () {
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::AssetLocations` (r:1 w:0)
	/// Proof: `AssetRegistry::AssetLocations` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `EVMAccounts::AccountExtension` (r:2 w:0)
	/// Proof: `EVMAccounts::AccountExtension` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `System::Digest` (r:1 w:0)
	/// Proof: `System::Digest` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `EVM::AccountCodesMetadata` (r:1 w:0)
	/// Proof: `EVM::AccountCodesMetadata` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `EVM::AccountCodes` (r:1 w:0)
	/// Proof: `EVM::AccountCodes` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `AssetRegistry::NextAssetId` (r:1 w:0)
	/// Proof: `AssetRegistry::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::LocationAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::LocationAssets` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `EVM::AccountStorages` (r:2 w:2)
	/// Proof: `EVM::AccountStorages` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn transfer_non_native_currency() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6621`
		//  Estimated: `12561`
		// Minimum execution time: 169_467_000 picoseconds.
		Weight::from_parts(170_737_000, 12561)
			.saturating_add(RocksDbWeight::get().reads(12_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn transfer_native_currency() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1638`
		//  Estimated: `3593`
		// Minimum execution time: 82_926_000 picoseconds.
		Weight::from_parts(83_990_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:1 w:1)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn update_balance_non_native_currency() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2275`
		//  Estimated: `3593`
		// Minimum execution time: 82_797_000 picoseconds.
		Weight::from_parts(83_651_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_balance_native_currency_creating() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1498`
		//  Estimated: `3593`
		// Minimum execution time: 50_335_000 picoseconds.
		Weight::from_parts(51_372_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_balance_native_currency_killing() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1586`
		//  Estimated: `3593`
		// Minimum execution time: 51_950_000 picoseconds.
		Weight::from_parts(52_523_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
