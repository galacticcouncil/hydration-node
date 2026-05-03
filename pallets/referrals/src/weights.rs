#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_bonds.
pub trait WeightInfo {
	fn register_code() -> Weight;
	fn link_code() -> Weight;
	fn claim_rewards() -> Weight;
	fn set_reward_percentage() -> Weight;
}

/// Weights for pallet_referrals using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `Referrals::ReferralAccounts` (r:1 w:1)
	/// Proof: `Referrals::ReferralAccounts` (`max_values`: None, `max_size`: Some(59), added: 2534, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::ReferralCodes` (r:1 w:1)
	/// Proof: `Referrals::ReferralCodes` (`max_values`: None, `max_size`: Some(59), added: 2534, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::Referrer` (r:0 w:1)
	/// Proof: `Referrals::Referrer` (`max_values`: None, `max_size`: Some(65), added: 2540, mode: `MaxEncodedLen`)
	fn register_code() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `318`
		//  Estimated: `6196`
		// Minimum execution time: 66_083_000 picoseconds.
		Weight::from_parts(66_613_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `Referrals::ReferralCodes` (r:1 w:0)
	/// Proof: `Referrals::ReferralCodes` (`max_values`: None, `max_size`: Some(59), added: 2534, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::LinkedAccounts` (r:1 w:1)
	/// Proof: `Referrals::LinkedAccounts` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	fn link_code() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `228`
		//  Estimated: `3545`
		// Minimum execution time: 21_152_000 picoseconds.
		Weight::from_parts(21_578_000, 3545)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Referrals::ReferrerShares` (r:1 w:1)
	/// Proof: `Referrals::ReferrerShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TraderShares` (r:1 w:1)
	/// Proof: `Referrals::TraderShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TotalShares` (r:1 w:1)
	/// Proof: `Referrals::TotalShares` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::Referrer` (r:1 w:1)
	/// Proof: `Referrals::Referrer` (`max_values`: None, `max_size`: Some(65), added: 2540, mode: `MaxEncodedLen`)
	fn claim_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `689`
		//  Estimated: `6196`
		// Minimum execution time: 87_419_000 picoseconds.
		Weight::from_parts(88_072_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Referrals::AssetRewards` (r:1 w:1)
	/// Proof: `Referrals::AssetRewards` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	fn set_reward_percentage() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `3514`
		// Minimum execution time: 14_014_000 picoseconds.
		Weight::from_parts(14_317_000, 3514)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
