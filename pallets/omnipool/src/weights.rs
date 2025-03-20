#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_omnipool.
pub trait WeightInfo {
	fn add_token() -> Weight;
	fn add_liquidity() -> Weight;
	fn remove_liquidity() -> Weight;
	fn sell() -> Weight;
	fn buy() -> Weight;
	fn set_asset_tradable_state() -> Weight;
	fn refund_refused_asset() -> Weight;
	fn sacrifice_position() -> Weight;
	fn set_asset_weight_cap() -> Weight;
	fn router_execution_sell(c: u32, e: u32) -> Weight;
	fn router_execution_buy(c: u32, e: u32) -> Weight;
	fn withdraw_protocol_liquidity() -> Weight;
	fn remove_token() -> Weight;
	fn calculate_spot_price_with_fee() -> Weight;
}

/// Weights for pallet_omnipool using the hydraDX node and recommended hardware.
impl WeightInfo for () {
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:1)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::NextPositionId` (r:1 w:1)
	/// Proof: `Omnipool::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:0 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	fn add_token() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3162`
		//  Estimated: `7406`
		// Minimum execution time: 160_328_000 picoseconds.
		Weight::from_parts(162_008_000, 7406)
			.saturating_add(RocksDbWeight::get().reads(13_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:3 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::NextPositionId` (r:1 w:1)
	/// Proof: `Omnipool::NextPositionId` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::CollectionMaxSupply` (r:1 w:0)
	/// Proof: `Uniques::CollectionMaxSupply` (`max_values`: None, `max_size`: Some(36), added: 2511, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
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
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:0 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	fn add_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4619`
		//  Estimated: `8739`
		// Minimum execution time: 262_382_000 picoseconds.
		Weight::from_parts(264_209_000, 8739)
			.saturating_add(RocksDbWeight::get().reads(24_u64))
			.saturating_add(RocksDbWeight::get().writes(14_u64))
	}
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Positions` (r:1 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:4 w:4)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Oracles` (r:2 w:0)
	/// Proof: `EmaOracle::Oracles` (`max_values`: None, `max_size`: Some(177), added: 2652, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:1)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn remove_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7531`
		//  Estimated: `11322`
		// Minimum execution time: 340_247_000 picoseconds.
		Weight::from_parts(342_981_000, 11322)
			.saturating_add(RocksDbWeight::get().reads(27_u64))
			.saturating_add(RocksDbWeight::get().writes(16_u64))
	}
	/// Storage: `AssetRegistry::Assets` (r:4 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:3 w:3)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:2 w:0)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:2 w:2)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (r:2 w:2)
	/// Proof: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::LinkedAccounts` (r:1 w:0)
	/// Proof: `Referrals::LinkedAccounts` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::Referrer` (r:1 w:0)
	/// Proof: `Referrals::Referrer` (`max_values`: None, `max_size`: Some(65), added: 2540, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::AssetRewards` (r:1 w:0)
	/// Proof: `Referrals::AssetRewards` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TotalShares` (r:1 w:1)
	/// Proof: `Referrals::TotalShares` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::ReferrerShares` (r:1 w:1)
	/// Proof: `Referrals::ReferrerShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TraderShares` (r:2 w:2)
	/// Proof: `Referrals::TraderShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::PendingConversions` (r:1 w:1)
	/// Proof: `Referrals::PendingConversions` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::CounterForPendingConversions` (r:1 w:1)
	/// Proof: `Referrals::CounterForPendingConversions` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn sell() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7891`
		//  Estimated: `13905`
		// Minimum execution time: 396_351_000 picoseconds.
		Weight::from_parts(400_745_000, 13905)
			.saturating_add(RocksDbWeight::get().reads(42_u64))
			.saturating_add(RocksDbWeight::get().writes(23_u64))
	}
	/// Storage: `Omnipool::Assets` (r:3 w:3)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:4 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:2 w:0)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:2 w:2)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (r:2 w:2)
	/// Proof: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::LinkedAccounts` (r:1 w:0)
	/// Proof: `Referrals::LinkedAccounts` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::Referrer` (r:1 w:0)
	/// Proof: `Referrals::Referrer` (`max_values`: None, `max_size`: Some(65), added: 2540, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::AssetRewards` (r:1 w:0)
	/// Proof: `Referrals::AssetRewards` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TotalShares` (r:1 w:1)
	/// Proof: `Referrals::TotalShares` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::ReferrerShares` (r:1 w:1)
	/// Proof: `Referrals::ReferrerShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TraderShares` (r:2 w:2)
	/// Proof: `Referrals::TraderShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::PendingConversions` (r:1 w:1)
	/// Proof: `Referrals::PendingConversions` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::CounterForPendingConversions` (r:1 w:1)
	/// Proof: `Referrals::CounterForPendingConversions` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn buy() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7891`
		//  Estimated: `13905`
		// Minimum execution time: 398_251_000 picoseconds.
		Weight::from_parts(401_486_000, 13905)
			.saturating_add(RocksDbWeight::get().reads(42_u64))
			.saturating_add(RocksDbWeight::get().writes(23_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	fn set_asset_tradable_state() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1338`
		//  Estimated: `3550`
		// Minimum execution time: 35_578_000 picoseconds.
		Weight::from_parts(35_952_000, 3550)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:2)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:1)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn refund_refused_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2918`
		//  Estimated: `6196`
		// Minimum execution time: 129_569_000 picoseconds.
		Weight::from_parts(130_479_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(11_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `Omnipool::Positions` (r:1 w:1)
	/// Proof: `Omnipool::Positions` (`max_values`: None, `max_size`: Some(100), added: 2575, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:1)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Class` (r:1 w:1)
	/// Proof: `Uniques::Class` (`max_values`: None, `max_size`: Some(190), added: 2665, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Account` (r:0 w:1)
	/// Proof: `Uniques::Account` (`max_values`: None, `max_size`: Some(112), added: 2587, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::ItemPriceOf` (r:0 w:1)
	/// Proof: `Uniques::ItemPriceOf` (`max_values`: None, `max_size`: Some(113), added: 2588, mode: `MaxEncodedLen`)
	fn sacrifice_position() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2943`
		//  Estimated: `3655`
		// Minimum execution time: 81_392_000 picoseconds.
		Weight::from_parts(82_324_000, 3655)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	fn set_asset_weight_cap() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1338`
		//  Estimated: `3550`
		// Minimum execution time: 35_864_000 picoseconds.
		Weight::from_parts(36_397_000, 3550)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:3 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	fn withdraw_protocol_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4613`
		//  Estimated: `8739`
		// Minimum execution time: 187_979_000 picoseconds.
		Weight::from_parts(189_239_000, 8739)
			.saturating_add(RocksDbWeight::get().reads(16_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: `Omnipool::Assets` (r:1 w:1)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:3 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:4 w:3)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:1 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:1 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:2 w:0)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn remove_token() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3721`
		//  Estimated: `11322`
		// Minimum execution time: 189_705_000 picoseconds.
		Weight::from_parts(191_023_000, 11322)
			.saturating_add(RocksDbWeight::get().reads(18_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: `Omnipool::Assets` (r:3 w:3)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:4 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:2 w:0)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:2 w:2)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (r:2 w:2)
	/// Proof: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::LinkedAccounts` (r:1 w:0)
	/// Proof: `Referrals::LinkedAccounts` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::AssetRewards` (r:1 w:0)
	/// Proof: `Referrals::AssetRewards` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TotalShares` (r:1 w:1)
	/// Proof: `Referrals::TotalShares` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TraderShares` (r:1 w:1)
	/// Proof: `Referrals::TraderShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::PendingConversions` (r:1 w:1)
	/// Proof: `Referrals::PendingConversions` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::CounterForPendingConversions` (r:1 w:1)
	/// Proof: `Referrals::CounterForPendingConversions` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 2]`.
	/// The range of component `e` is `[0, 1]`.
	fn router_execution_sell(_c: u32, e: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2403 + e * (4987 ±0)`
		//  Estimated: `8799 + e * (7749 ±0)`
		// Minimum execution time: 74_890_000 picoseconds.
		Weight::from_parts(77_078_522, 8799)
			// Standard Error: 123_826
			.saturating_add(Weight::from_parts(325_420_660, 0).saturating_mul(e.into()))
			.saturating_add(RocksDbWeight::get().reads(9_u64))
			.saturating_add(RocksDbWeight::get().reads((30_u64).saturating_mul(e.into())))
			.saturating_add(RocksDbWeight::get().writes((21_u64).saturating_mul(e.into())))
			.saturating_add(Weight::from_parts(0, 7749).saturating_mul(e.into()))
	}
	/// Storage: `Omnipool::Assets` (r:3 w:3)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:4 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:5 w:5)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::HubAssetImbalance` (r:1 w:1)
	/// Proof: `Omnipool::HubAssetImbalance` (`max_values`: Some(1), `max_size`: Some(17), added: 512, mode: `MaxEncodedLen`)
	/// Storage: `DynamicFees::AssetFee` (r:2 w:0)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Router::SkipEd` (r:1 w:0)
	/// Proof: `Router::SkipEd` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `Duster::AccountBlacklist` (r:2 w:0)
	/// Proof: `Duster::AccountBlacklist` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::BannedAssets` (r:2 w:0)
	/// Proof: `AssetRegistry::BannedAssets` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AccountCurrencyMap` (r:2 w:2)
	/// Proof: `MultiTransactionPayment::AccountCurrencyMap` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `MultiTransactionPayment::AcceptedCurrencies` (r:1 w:0)
	/// Proof: `MultiTransactionPayment::AcceptedCurrencies` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	/// Storage: `EmaOracle::Accumulator` (r:1 w:1)
	/// Proof: `EmaOracle::Accumulator` (`max_values`: Some(1), `max_size`: Some(5921), added: 6416, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (r:2 w:2)
	/// Proof: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::LinkedAccounts` (r:1 w:0)
	/// Proof: `Referrals::LinkedAccounts` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::AssetRewards` (r:1 w:0)
	/// Proof: `Referrals::AssetRewards` (`max_values`: None, `max_size`: Some(49), added: 2524, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TotalShares` (r:1 w:1)
	/// Proof: `Referrals::TotalShares` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::TraderShares` (r:1 w:1)
	/// Proof: `Referrals::TraderShares` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::PendingConversions` (r:1 w:1)
	/// Proof: `Referrals::PendingConversions` (`max_values`: None, `max_size`: Some(20), added: 2495, mode: `MaxEncodedLen`)
	/// Storage: `Referrals::CounterForPendingConversions` (r:1 w:1)
	/// Proof: `Referrals::CounterForPendingConversions` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 2]`.
	/// The range of component `e` is `[0, 1]`.
	fn router_execution_buy(c: u32, _e: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `7390`
		//  Estimated: `13905`
		// Minimum execution time: 381_802_000 picoseconds.
		Weight::from_parts(369_607_267, 13905)
			// Standard Error: 197_288
			.saturating_add(Weight::from_parts(16_716_881, 0).saturating_mul(c.into()))
			.saturating_add(RocksDbWeight::get().reads(39_u64))
			.saturating_add(RocksDbWeight::get().writes(21_u64))
	}
	/// Storage: `DynamicFees::AssetFee` (r:2 w:0)
	/// Proof: `DynamicFees::AssetFee` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `Omnipool::Assets` (r:2 w:0)
	/// Proof: `Omnipool::Assets` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:2 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:2 w:0)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	fn calculate_spot_price_with_fee() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2362`
		//  Estimated: `6190`
		// Minimum execution time: 68_827_000 picoseconds.
		Weight::from_parts(69_541_000, 6190)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
	}
}
