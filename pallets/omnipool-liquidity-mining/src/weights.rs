#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_omnipool_liquidity_mining.
pub trait WeightInfo {
	fn create_global_farm() -> Weight;
	fn update_global_farm() -> Weight;
	fn terminate_global_farm() -> Weight;
	fn create_yield_farm() -> Weight;
	fn update_yield_farm() -> Weight;
	fn stop_yield_farm() -> Weight;
	fn resume_yield_farm() -> Weight;
	fn terminate_yield_farm() -> Weight;
	fn deposit_shares() -> Weight;
	fn redeposit_shares() -> Weight;
	fn claim_rewards() -> Weight;
	fn withdraw_shares() -> Weight;	
	fn join_farms(c: u32) -> Weight;	
	fn add_liquidity_and_join_farms(c: u32) -> Weight;
	fn add_liquidity_stableswap_omnipool_and_join_farms(c: u32) -> Weight;

	fn exit_farms(c: u32) -> Weight;
}

/// Weights for pallet_omnipool_liquidity_mining using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::FarmSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::FarmSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:0 w:1)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	fn create_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `590`
		//  Estimated: `6196`
		// Minimum execution time: 91_333_000 picoseconds.
		Weight::from_parts(92_028_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(5_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `606`
		//  Estimated: `3670`
		// Minimum execution time: 26_702_000 picoseconds.
		Weight::from_parts(27_121_000, 3670)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:1)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	fn terminate_global_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1030`
		//  Estimated: `6196`
		// Minimum execution time: 89_722_000 picoseconds.
		Weight::from_parts(90_451_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::FarmSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::FarmSequencer` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	fn create_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2469`
		//  Estimated: `6294`
		// Minimum execution time: 143_071_000 picoseconds.
		Weight::from_parts(143_895_000, 6294)
			.saturating_add(RocksDbWeight::get().reads(9_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:0)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn update_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2639`
		//  Estimated: `6294`
		// Minimum execution time: 147_575_000 picoseconds.
		Weight::from_parts(148_554_000, 6294)
			.saturating_add(RocksDbWeight::get().reads(9_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn stop_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2212`
		//  Estimated: `6294`
		// Minimum execution time: 138_873_000 picoseconds.
		Weight::from_parts(140_176_000, 6294)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn resume_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2675`
		//  Estimated: `6294`
		// Minimum execution time: 144_442_000 picoseconds.
		Weight::from_parts(145_798_000, 6294)
			.saturating_add(RocksDbWeight::get().reads(9_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `OmnipoolWarehouseLM::ActiveYieldFarm` (r:1 w:0)
	/// Proof: `OmnipoolWarehouseLM::ActiveYieldFarm` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn terminate_yield_farm() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `957`
		//  Estimated: `6196`
		// Minimum execution time: 85_822_000 picoseconds.
		Weight::from_parts(86_361_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(5_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	fn deposit_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4276`
		//  Estimated: `11598`
		// Minimum execution time: 239_939_000 picoseconds.
		Weight::from_parts(241_587_000, 11598)
			.saturating_add(RocksDbWeight::get().reads(17_u64))
			.saturating_add(RocksDbWeight::get().writes(14_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:1 w:0)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn redeposit_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4513`
		//  Estimated: `11598`
		// Minimum execution time: 204_467_000 picoseconds.
		Weight::from_parts(207_022_000, 11598)
			.saturating_add(RocksDbWeight::get().reads(15_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:3)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3012`
		//  Estimated: `8799`
		// Minimum execution time: 192_783_000 picoseconds.
		Weight::from_parts(196_174_000, 8799)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:1 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:3)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:2)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn withdraw_shares() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4105`
		//  Estimated: `8799`
		// Minimum execution time: 285_929_000 picoseconds.
		Weight::from_parts(289_072_000, 8799)
			.saturating_add(RocksDbWeight::get().reads(15_u64))
			.saturating_add(RocksDbWeight::get().writes(15_u64))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:6 w:6)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:4 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4248 + c * (474 ±0)`
		//  Estimated: `11598 + c * (2680 ±0)`
		// Minimum execution time: 244_981_000 picoseconds.
		Weight::from_parts(147_330_577, 11598)
			// Standard Error: 69_233
			.saturating_add(Weight::from_parts(102_226_732, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(14_u64))
			.saturating_add(RocksDbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(11_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
	/// Storage: `AssetRegistry::Assets` (r:3 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:3 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:5 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::NextPositionId` (r:1 w:1)
	/// Proof: `Omnipool::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:2)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:2 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:7 w:7)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:0 w:1)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:0 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn add_liquidity_and_join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5748 + c * (474 ±0)`
		//  Estimated: `14250 + c * (2680 ±0)`
		// Minimum execution time: 453_628_000 picoseconds.
		Weight::from_parts(351_105_889, 14250)
			// Standard Error: 118_417
			.saturating_add(Weight::from_parts(106_095_381, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(33_u64))
			.saturating_add(RocksDbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(24_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:1 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:0)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:8 w:7)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:2)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn exit_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3755 + c * (486 ±0)`
		//  Estimated: `8799 + c * (2680 ±0)`
		// Minimum execution time: 247_754_000 picoseconds.
		Weight::from_parts(90_316_650, 8799)
			// Standard Error: 240_623
			.saturating_add(Weight::from_parts(159_106_526, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(11_u64))
			.saturating_add(RocksDbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}

	/// Storage: `Stableswap::Pools` (r:1 w:0)
	/// Proof: `Stableswap::Pools` (`max_values`: None, `max_size`: Some(57), added: 2532, mode: `MaxEncodedLen`)
	/// Storage: `Stableswap::AssetTradability` (r:5 w:0)
	/// Proof: `Stableswap::AssetTradability` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:8 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:13 w:13)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:2 w:2)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:7 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:7 w:7)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:5 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::NextPositionId` (r:1 w:1)
	/// Proof: `Omnipool::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:2 w:2)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:2 w:2)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:2 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::YieldFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::YieldFarm` (`max_values`: None, `max_size`: Some(198), added: 2673, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::GlobalFarm` (r:5 w:5)
	/// Proof: `OmnipoolWarehouseLM::GlobalFarm` (`max_values`: None, `max_size`: Some(205), added: 2680, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::DepositSequencer` (r:1 w:1)
	/// Proof: `OmnipoolWarehouseLM::DepositSequencer` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:3)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolLiquidityMining::OmniPositionId` (r:0 w:1)
	/// Proof: `OmnipoolLiquidityMining::OmniPositionId` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:0 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `OmnipoolWarehouseLM::Deposit` (r:0 w:1)
	/// Proof: `OmnipoolWarehouseLM::Deposit` (`max_values`: None, `max_size`: Some(385), added: 2860, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 5]`.
	fn add_liquidity_stableswap_omnipool_and_join_farms(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `22068 + c * (507 ±0)`
		//  Estimated: `34569 + c * (2680 ±0)`
		// Minimum execution time: 1_247_561_000 picoseconds.
		Weight::from_parts(1_200_125_260, 34569)
			// Standard Error: 863_778
			.saturating_add(Weight::from_parts(91_357_644, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(62_u64))
			.saturating_add(RocksDbWeight::get().reads((3_u64).saturating_mul(c.into())))
			.saturating_add(RocksDbWeight::get().writes(35_u64))
			.saturating_add(RocksDbWeight::get().writes((3_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2680).saturating_mul(c.into()))
	}
}
