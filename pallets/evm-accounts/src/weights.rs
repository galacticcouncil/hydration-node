#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn bind_evm_address() -> Weight;
	fn add_contract_deployer() -> Weight;
	fn remove_contract_deployer() -> Weight;
	fn renounce_contract_deployer() -> Weight;
	fn approve_contract() -> Weight;
	fn disapprove_contract() -> Weight;
	fn claim_account() -> Weight;
}

/// Weights for `pallet_evm_accounts` using the HydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `EVMAccounts::AccountExtension` (r:1 w:1)
	/// Proof: `EVMAccounts::AccountExtension` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::NextAssetId` (r:1 w:0)
	/// Proof: `AssetRegistry::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::LocationAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::LocationAssets` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:1 w:0)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	fn bind_evm_address() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2022`
		//  Estimated: `6196`
		// Minimum execution time: 73_410_000 picoseconds.
		Weight::from_parts(73_410_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `EVMAccounts::ContractDeployer` (r:0 w:1)
	/// Proof: `EVMAccounts::ContractDeployer` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn add_contract_deployer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1012`
		//  Estimated: `0`
		// Minimum execution time: 31_091_000 picoseconds.
		Weight::from_parts(31_091_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ContractDeployer` (r:0 w:1)
	/// Proof: `EVMAccounts::ContractDeployer` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn remove_contract_deployer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1012`
		//  Estimated: `0`
		// Minimum execution time: 25_519_000 picoseconds.
		Weight::from_parts(25_519_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ContractDeployer` (r:0 w:1)
	/// Proof: `EVMAccounts::ContractDeployer` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn renounce_contract_deployer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1012`
		//  Estimated: `0`
		// Minimum execution time: 26_950_000 picoseconds.
		Weight::from_parts(26_950_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ApprovedContract` (r:0 w:1)
	/// Proof: `EVMAccounts::ApprovedContract` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn approve_contract() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1012`
		//  Estimated: `0`
		// Minimum execution time: 30_515_000 picoseconds.
		Weight::from_parts(30_515_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ApprovedContract` (r:0 w:1)
	/// Proof: `EVMAccounts::ApprovedContract` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn disapprove_contract() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1012`
		//  Estimated: `0`
		// Minimum execution time: 25_334_000 picoseconds.
		Weight::from_parts(25_334_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `System::Account` (r:3 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::AssetLocations` (r:1 w:0)
	/// Proof: `AssetRegistry::AssetLocations` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `EVMAccounts::AccountExtension` (r:3 w:1)
	/// Proof: `EVMAccounts::AccountExtension` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
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
	/// Storage: `EVM::AccountStorages` (r:1 w:0)
	/// Proof: `EVM::AccountStorages` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `Tokens::Accounts` (r:1 w:0)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:0 w:1)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	fn claim_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7276`
		//  Estimated: `10741`
		// Minimum execution time: 230_456_000 picoseconds.
		Weight::from_parts(230_456_000, 10741)
			.saturating_add(RocksDbWeight::get().reads(17_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
}
