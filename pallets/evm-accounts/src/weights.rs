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
}

/// Weights for `pallet_evm_accounts` using the HydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `EVMAccounts::AccountExtension` (r:1 w:1)
	/// Proof: `EVMAccounts::AccountExtension` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
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
		//  Measured:  `578`
		//  Estimated: `4087`
		// Minimum execution time: 40_452_000 picoseconds.
		Weight::from_parts(41_073_000, 4087)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ContractDeployer` (r:0 w:1)
	/// Proof: `EVMAccounts::ContractDeployer` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn add_contract_deployer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_172_000 picoseconds.
		Weight::from_parts(10_418_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ContractDeployer` (r:0 w:1)
	/// Proof: `EVMAccounts::ContractDeployer` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn remove_contract_deployer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_046_000 picoseconds.
		Weight::from_parts(10_361_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ContractDeployer` (r:0 w:1)
	/// Proof: `EVMAccounts::ContractDeployer` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn renounce_contract_deployer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_267_000 picoseconds.
		Weight::from_parts(10_459_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ApprovedContract` (r:0 w:1)
	/// Proof: `EVMAccounts::ApprovedContract` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn approve_contract() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_250_000 picoseconds.
		Weight::from_parts(10_466_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `EVMAccounts::ApprovedContract` (r:0 w:1)
	/// Proof: `EVMAccounts::ApprovedContract` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	fn disapprove_contract() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_131_000 picoseconds.
		Weight::from_parts(10_348_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
