#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_staking.
pub trait WeightInfo {
	fn initialize_staking() -> Weight;
	fn stake() -> Weight;
	fn increase_stake() -> Weight;
	fn claim() -> Weight;
	fn unstake() -> Weight;
}

/// Weights for pallet_staking using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ClassAccount` (r:0 w:1)
	/// Proof: `Uniques::ClassAccount` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	fn initialize_staking() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `516`
		//  Estimated: `3655`
		// Minimum execution time: 40_743_000 picoseconds.
		Weight::from_parts(41_335_000, 3655)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:1 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Staking::NextPositionId` (r:1 w:1)
	/// Proof: `Staking::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:0 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	fn stake() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1274`
		//  Estimated: `6196`
		// Minimum execution time: 102_939_000 picoseconds.
		Weight::from_parts(103_966_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Staking::VotesRewarded` (r:1 w:0)
	/// Proof: `Staking::VotesRewarded` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:1 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Votes` (r:1 w:1)
	/// Proof: `Staking::Votes` (`max_values`: None, `max_size`: Some(558), added: 3033, mode: `MaxEncodedLen`)
	/// Storage: `Referenda::ReferendumInfoFor` (r:25 w:0)
	/// Proof: `Referenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(936), added: 3411, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	fn increase_stake() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1872`
		//  Estimated: `86265`
		// Minimum execution time: 182_931_000 picoseconds.
		Weight::from_parts(185_120_000, 86265)
			.saturating_add(RocksDbWeight::get().reads(34_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Staking::VotesRewarded` (r:1 w:0)
	/// Proof: `Staking::VotesRewarded` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:1 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Votes` (r:1 w:1)
	/// Proof: `Staking::Votes` (`max_values`: None, `max_size`: Some(558), added: 3033, mode: `MaxEncodedLen`)
	/// Storage: `Referenda::ReferendumInfoFor` (r:25 w:0)
	/// Proof: `Referenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(936), added: 3411, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1872`
		//  Estimated: `86265`
		// Minimum execution time: 176_595_000 picoseconds.
		Weight::from_parts(178_810_000, 86265)
			.saturating_add(RocksDbWeight::get().reads(34_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Staking::Staking` (r:1 w:1)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Positions` (r:1 w:1)
	/// Proof: `Staking::Positions` (`max_values`: None, `max_size`: Some(132), added: 2607, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Votes` (r:1 w:1)
	/// Proof: `Staking::Votes` (`max_values`: None, `max_size`: Some(558), added: 3033, mode: `MaxEncodedLen`)
	/// Storage: `Staking::VotesRewarded` (r:1 w:0)
	/// Proof: `Staking::VotesRewarded` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:1)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:0)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn unstake() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1429`
		//  Estimated: `6196`
		// Minimum execution time: 150_537_000 picoseconds.
		Weight::from_parts(152_599_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
}
