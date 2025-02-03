#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_route_executor`.
pub trait WeightInfo {
	fn skip_ed_handling_for_trade_with_insufficient_assets() -> Weight;
	fn calculate_and_execute_sell_in_lbp(c: u32, ) -> Weight;
	fn calculate_and_execute_buy_in_lbp(c: u32, b: u32, ) -> Weight;
	fn set_route_for_xyk() -> Weight;
	fn force_insert_route() -> Weight;
	fn get_oracle_price_for_xyk() -> Weight;
	fn get_oracle_price_for_omnipool() -> Weight;
	fn get_route() -> Weight;
	fn calculate_spot_price_with_fee_in_lbp() -> Weight;
}

/// Weights for `pallet_route_executor` using the HydraDX node and recommended hardware.
pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for HydraWeight<T> {
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:0 w:1)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	fn skip_ed_handling_for_trade_with_insufficient_assets() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `944`
		//  Estimated: `3590`
		// Minimum execution time: 18_351_000 picoseconds.
		Weight::from_parts(18_623_000, 3590)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `LBP::PoolData` (r:1 w:0)
	/// Proof: `LBP::PoolData` (`max_values`: None, `max_size`: Some(163), added: 2638, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Locks` (r:1 w:1)
	/// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:1)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[0, 1]`.
	fn calculate_and_execute_sell_in_lbp(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3809`
		//  Estimated: `13905`
		// Minimum execution time: 403_889_000 picoseconds.
		Weight::from_parts(408_439_763, 13905)
			// Standard Error: 288_679
			.saturating_add(Weight::from_parts(78_440_436, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(17_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: `LBP::PoolData` (r:1 w:0)
	/// Proof: `LBP::PoolData` (`max_values`: None, `max_size`: Some(163), added: 2638, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Locks` (r:1 w:1)
	/// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:1)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 2]`.
	/// The range of component `b` is `[0, 1]`.
	fn calculate_and_execute_buy_in_lbp(c: u32, b: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1992 + b * (1842 ±0)`
		//  Estimated: `8799 + b * (7749 ±0)`
		// Minimum execution time: 116_165_000 picoseconds.
		Weight::from_parts(117_686_000, 8799)
			// Standard Error: 232_510
			.saturating_add(Weight::from_parts(962_142, 0).saturating_mul(c.into()))
			// Standard Error: 517_865
			.saturating_add(Weight::from_parts(293_725_885, 0).saturating_mul(b.into()))
			.saturating_add(RocksDbWeight::get().reads(5_u64))
			.saturating_add(RocksDbWeight::get().reads((12_u64).saturating_mul(b.into())))
			.saturating_add(RocksDbWeight::get().writes((8_u64).saturating_mul(b.into())))
			.saturating_add(Weight::from_parts(0, 7749).saturating_mul(b.into()))
	}
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::Routes` (r:1 w:1)
	/// Proof: `Router::Routes` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:7 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:6 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:15 w:0)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `XYK::ShareToken` (r:6 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:7 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:5 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:5 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:0)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:0)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn set_route_for_xyk() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7071`
		//  Estimated: `39735`
		// Minimum execution time: 2_238_560_000 picoseconds.
		Weight::from_parts(2_251_837_000, 39735)
			.saturating_add(RocksDbWeight::get().reads(58_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Router::Routes` (r:0 w:1)
	/// Proof: `Router::Routes` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	fn force_insert_route() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1012`
		//  Estimated: `0`
		// Minimum execution time: 30_739_000 picoseconds.
		Weight::from_parts(31_254_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Router::Routes` (r:1 w:0)
	/// Proof: `Router::Routes` (`max_values`: None, `max_size`: Some(90), added: 2565, mode: `MaxEncodedLen`)
	fn get_route() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `800`
		//  Estimated: `3555`
		// Minimum execution time: 9_385_000 picoseconds.
		Weight::from_parts(9_633_000, 3555)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
	}
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn get_oracle_price_for_xyk() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1452`
		//  Estimated: `6294`
		// Minimum execution time: 32_684_000 picoseconds.
		Weight::from_parts(33_157_000, 6294)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
	}
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn get_oracle_price_for_omnipool() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1814`
		//  Estimated: `11598`
		// Minimum execution time: 48_768_000 picoseconds.
		Weight::from_parts(49_167_000, 11598)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
	}
	/// Storage: `LBP::PoolData` (r:1 w:0)
	/// Proof: `LBP::PoolData` (`max_values`: None, `max_size`: Some(163), added: 2638, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Locks` (r:1 w:0)
	/// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:0)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	fn calculate_spot_price_with_fee_in_lbp() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2029`
		//  Estimated: `6190`
		// Minimum execution time: 57_184_000 picoseconds.
		Weight::from_parts(57_713_000, 6190)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
	}
}
