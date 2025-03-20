#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_xyk_liquidity_mining.
pub trait WeightInfo {
	fn create_global_farm() -> Weight;
	fn update_global_farm() -> Weight;
	fn terminate_global_farm() -> Weight;
	fn create_yield_farm() -> Weight;
	fn update_yield_farm() -> Weight;
	fn stop_yield_farm() -> Weight;
	fn terminate_yield_farm() -> Weight;
	fn deposit_shares() -> Weight;
	fn redeposit_shares() -> Weight;
	fn claim_rewards() -> Weight;
	fn withdraw_shares() -> Weight;
	fn resume_yield_farm() -> Weight;
	fn join_farms(c: u32) -> Weight;	
	fn add_liquidity_and_join_farms(c: u32) -> Weight;
	fn exit_farms(c: u32) -> Weight;
}

/// Weights for `pallet_xyk_liquidity_mining` using the HydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:1)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:3 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:3 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:4 w:4)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::ExistentialDepositCounter` (r:1 w:1)
	/// Proof: `AssetRegistry::ExistentialDepositCounter` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::FarmSequencer` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::FarmSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:0 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	fn create_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3406`
		//  Estimated: `11402`
		// Minimum execution time: 330_268_000 picoseconds.
		Weight::from_parts(332_630_000, 11402)
			.saturating_add(RocksDbWeight::get().reads(20_u64))
			.saturating_add(RocksDbWeight::get().writes(12_u64))
	}
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4654`
		//  Estimated: `6156`
		// Minimum execution time: 134_959_000 picoseconds.
		Weight::from_parts(136_338_000, 6156)
			.saturating_add(RocksDbWeight::get().reads(9_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:1)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::ExistentialDepositCounter` (r:1 w:1)
	/// Proof: `AssetRegistry::ExistentialDepositCounter` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	fn terminate_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4118`
		//  Estimated: `6196`
		// Minimum execution time: 210_698_000 picoseconds.
		Weight::from_parts(213_046_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(13_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::FarmSequencer` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::FarmSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:0 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	fn create_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1615`
		//  Estimated: `3670`
		// Minimum execution time: 58_967_000 picoseconds.
		Weight::from_parts(59_286_000, 3670)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::ActiveYieldFarm` (r:1 w:0)
	/// Proof: `XYKWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5170`
		//  Estimated: `6156`
		// Minimum execution time: 162_220_000 picoseconds.
		Weight::from_parts(164_499_000, 6156)
			.saturating_add(RocksDbWeight::get().reads(12_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: `XYKWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn stop_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4970`
		//  Estimated: `6156`
		// Minimum execution time: 157_583_000 picoseconds.
		Weight::from_parts(158_719_000, 6156)
			.saturating_add(RocksDbWeight::get().reads(11_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `XYKWarehouseLM::ActiveYieldFarm` (r:1 w:0)
	/// Proof: `XYKWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn terminate_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5017`
		//  Estimated: `6156`
		// Minimum execution time: 131_664_000 picoseconds.
		Weight::from_parts(132_762_000, 6156)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:3 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:6 w:4)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `XYK::PoolAssets` (r:1 w:0)
	/// Proof: `XYK::PoolAssets` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	/// Storage: `XYK::TotalLiquidity` (r:1 w:0)
	/// Proof: `XYK::TotalLiquidity` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	fn deposit_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6357`
		//  Estimated: `16488`
		// Minimum execution time: 283_636_000 picoseconds.
		Weight::from_parts(286_604_000, 16488)
			.saturating_add(RocksDbWeight::get().reads(26_u64))
			.saturating_add(RocksDbWeight::get().writes(11_u64))
	}
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:4 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `XYK::PoolAssets` (r:1 w:0)
	/// Proof: `XYK::PoolAssets` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	/// Storage: `XYK::TotalLiquidity` (r:1 w:0)
	/// Proof: `XYK::TotalLiquidity` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	fn redeposit_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7286`
		//  Estimated: `11322`
		// Minimum execution time: 223_433_000 picoseconds.
		Weight::from_parts(224_674_000, 11322)
			.saturating_add(RocksDbWeight::get().reads(18_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:3 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7110`
		//  Estimated: `8739`
		// Minimum execution time: 224_682_000 picoseconds.
		Weight::from_parts(226_222_000, 8739)
			.saturating_add(RocksDbWeight::get().reads(15_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:3 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:4 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `XYK::PoolAssets` (r:1 w:0)
	/// Proof: `XYK::PoolAssets` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::ExistentialDepositCounter` (r:1 w:1)
	/// Proof: `AssetRegistry::ExistentialDepositCounter` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn withdraw_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6783`
		//  Estimated: `13905`
		// Minimum execution time: 441_189_000 picoseconds.
		Weight::from_parts(444_785_000, 13905)
			.saturating_add(RocksDbWeight::get().reads(29_u64))
			.saturating_add(RocksDbWeight::get().writes(16_u64))
	}
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn resume_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5462`
		//  Estimated: `6156`
		// Minimum execution time: 163_963_000 picoseconds.
		Weight::from_parts(165_035_000, 6156)
			.saturating_add(RocksDbWeight::get().reads(12_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:3 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:10 w:8)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:6 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:6 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `XYK::PoolAssets` (r:1 w:0)
	/// Proof: `XYK::PoolAssets` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	/// Storage: `XYK::TotalLiquidity` (r:1 w:0)
	/// Proof: `XYK::TotalLiquidity` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6878 + c * (672 ±0)`
		//  Estimated: `13905 + c * (2701 ±0)`
		// Minimum execution time: 293_151_000 picoseconds.
		Weight::from_parts(202_764_318, 13905)
			// Standard Error: 71_458
			.saturating_add(Weight::from_parts(94_309_018, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(21_u64))
			.saturating_add(RocksDbWeight::get().reads((5_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2701).saturating_mul(c.into()))
	}
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:3 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:12 w:12)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `XYK::TotalLiquidity` (r:1 w:1)
	/// Proof: `XYK::TotalLiquidity` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:6 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:3 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:6 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::WhitelistedAssets` (r:1 w:0)
	/// Proof: `EmaOracle::WhitelistedAssets` (`max_values`: Some(1), `max_size`: Some(641), added: 1136, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `XYK::PoolAssets` (r:1 w:0)
	/// Proof: `XYK::PoolAssets` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn add_liquidity_and_join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7086 + c * (672 ±0)`
		//  Estimated: `19071 + c * (2701 ±0)`
		// Minimum execution time: 430_597_000 picoseconds.
		Weight::from_parts(340_717_139, 19071)
			// Standard Error: 102_863
			.saturating_add(Weight::from_parts(94_542_857, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(26_u64))
			.saturating_add(RocksDbWeight::get().reads((5_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(14_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2701).saturating_mul(c.into()))
	}
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `XYK::ShareToken` (r:1 w:0)
	/// Proof: `XYK::ShareToken` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `XYKWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(413), added: 2888, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `XYKWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(226), added: 2701, mode: `MaxEncodedLen`)
	/// Storage: `XYKWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `XYKWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:9 w:9)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:1 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:7 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:8 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `XYK::PoolAssets` (r:1 w:0)
	/// Proof: `XYK::PoolAssets` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::ExistentialDepositCounter` (r:1 w:1)
	/// Proof: `AssetRegistry::ExistentialDepositCounter` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn exit_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6479 + c * (692 ±0)`
		//  Estimated: `5788 + c * (2792 ±0)`
		// Minimum execution time: 296_635_000 picoseconds.
		Weight::from_parts(113_520_180, 5788)
			// Standard Error: 894_486
			.saturating_add(Weight::from_parts(180_465_455, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(11_u64))
			.saturating_add(RocksDbWeight::get().reads((5_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2792).saturating_mul(c.into()))
	}
}
