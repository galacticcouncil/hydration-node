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
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:1 w:1)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `EVMAccounts::AccountExtension` (r:1 w:0)
	/// Proof: `EVMAccounts::AccountExtension` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Liquidation::BorrowingContract` (r:1 w:0)
	/// Proof: `Liquidation::BorrowingContract` (`max_values`: Some(1), `max_size`: Some(20), added: 515, mode: `MaxEncodedLen`)
	fn liquidate() -> Weight {
		Weight::from_parts(112_237_000, 6190)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `Liquidation::BorrowingContract` (r:0 w:1)
	/// Proof: `Liquidation::BorrowingContract` (`max_values`: Some(1), `max_size`: Some(20), added: 515, mode: `MaxEncodedLen`)
	fn set_borrowing_contract() -> Weight {
		Weight::from_parts(4_696_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Liquidation::OracleSigners` (r:0 w:1)
	/// Proof: `Liquidation::OracleSigners` (`max_values`: Some(1), `max_size`: Some(101), added: 596, mode: `MaxEncodedLen`)
	fn set_oracle_signers() -> Weight {
		Weight::from_parts(5_688_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Liquidation::OracleCallAddresses` (r:0 w:1)
	/// Proof: `Liquidation::OracleCallAddresses` (`max_values`: Some(1), `max_size`: Some(101), added: 596, mode: `MaxEncodedLen`)
	fn set_oracle_call_addresses() -> Weight {
		Weight::from_parts(5_725_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Liquidation::UnsignedLiquidationPriority` (r:0 w:1)
	/// Proof: `Liquidation::UnsignedLiquidationPriority` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn set_unsigned_liquidation_priority() -> Weight {
		Weight::from_parts(4_451_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Liquidation::OracleUpdatePriority` (r:0 w:1)
	/// Proof: `Liquidation::OracleUpdatePriority` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn set_oracle_update_priority() -> Weight {
		Weight::from_parts(4_430_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
