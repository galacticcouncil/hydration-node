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
	fn convert() -> Weight;
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
	/// Storage: `AssetRegistry::Assets` (r:3 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:2 w:2)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:2 w:0)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (r:2 w:2)
	/// Proof: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::TradeVolumeLimitPerAsset` (r:2 w:0)
	/// Proof: `CircuitBreaker::TradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Staking` (r:1 w:0)
	/// Proof: `Staking::Staking` (`max_values`: Some(1), `max_size`: Some(48), added: 543, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::PendingConversions` (r:1 w:1)
	/// Proof: `Referrals::PendingConversions` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::CounterForPendingConversions` (r:1 w:1)
	/// Proof: `Referrals::CounterForPendingConversions` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:0 w:1)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	fn convert() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2468`
		//  Estimated: `8790`
		// Minimum execution time: 302_462_000 picoseconds.
		Weight::from_parts(304_715_000, 8790)
			.saturating_add(RocksDbWeight::get().reads(28_u64))
			.saturating_add(RocksDbWeight::get().writes(15_u64))
	}
	/// Storage: `Referrals::PendingConversions` (r:1 w:0)
	/// Proof: `Referrals::PendingConversions` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
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
