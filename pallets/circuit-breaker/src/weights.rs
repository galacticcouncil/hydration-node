#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weights for `pallet_circuit_breaker`.
pub trait WeightInfo {
	fn on_initialize_skip_lockdown_lifting() -> Weight;
	fn on_initialize_lift_lockdown() -> Weight;
	fn on_finalize(n: u32, m: u32) -> Weight;
	fn on_finalize_single_liquidity_limit_entry() -> Weight;
	fn on_finalize_single_trade_limit_entry() -> Weight;
	fn on_finalize_empty() -> Weight;
	fn set_trade_volume_limit() -> Weight;
	fn set_add_liquidity_limit() -> Weight;
	fn set_remove_liquidity_limit() -> Weight;
	fn set_global_withdraw_limit() -> Weight;
	fn reset_withdraw_lockdown() -> Weight;
	fn set_global_withdraw_lockdown() -> Weight;
	fn add_egress_accounts(n: u32) -> Weight;
	fn remove_egress_accounts(n: u32) -> Weight;
	fn set_asset_category() -> Weight;
	fn ensure_add_liquidity_limit() -> Weight;
	fn ensure_remove_liquidity_limit() -> Weight;
	fn ensure_pool_state_change_limit() -> Weight;
	fn lockdown_asset() -> Weight;
	fn force_lift_lockdown() -> Weight;
	fn release_deposit() -> Weight;
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: `CircuitBreaker::WithdrawLockdownUntil` (r:1 w:0)
	/// Proof: `CircuitBreaker::WithdrawLockdownUntil` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn on_initialize_skip_lockdown_lifting() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `1493`
		// Minimum execution time: 1_000_000 picoseconds.
		Weight::from_parts(1_000_000, 1493)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
	}
	/// Storage: `CircuitBreaker::WithdrawLockdownUntil` (r:1 w:1)
	/// Proof: `CircuitBreaker::WithdrawLockdownUntil` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn on_initialize_lift_lockdown() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `56`
		//  Estimated: `1493`
		// Minimum execution time: 7_000_000 picoseconds.
		Weight::from_parts(7_000_000, 1493)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// The range of component `n` is `[0, 400]`.
	/// The range of component `m` is `[0, 400]`.
	fn on_finalize(n: u32, m: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `64 + m * (113 ±0) + n * (56 ±0)`
		//  Estimated: `0`
		// Minimum execution time: 382_902_000 picoseconds.
		Weight::from_parts(384_768_000, 0)
			// Standard Error: 11_644
			.saturating_add(Weight::from_parts(337_059, 0).saturating_mul(n.into()))
			// Standard Error: 11_644
			.saturating_add(Weight::from_parts(1_228_732, 0).saturating_mul(m.into()))
	}
	fn on_finalize_single_liquidity_limit_entry() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `208`
		//  Estimated: `0`
		// Minimum execution time: 8_784_000 picoseconds.
		Weight::from_parts(8_921_000, 0)
	}
	fn on_finalize_single_trade_limit_entry() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `208`
		//  Estimated: `0`
		// Minimum execution time: 8_748_000 picoseconds.
		Weight::from_parts(8_886_000, 0)
	}
	fn on_finalize_empty() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `208`
		//  Estimated: `0`
		// Minimum execution time: 8_809_000 picoseconds.
		Weight::from_parts(8_951_000, 0)
	}
	/// Storage: `CircuitBreaker::TradeVolumeLimitPerAsset` (r:0 w:1)
	/// Proof: `CircuitBreaker::TradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn set_trade_volume_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_099_000 picoseconds.
		Weight::from_parts(10_316_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:0 w:1)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	fn set_add_liquidity_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_077_000 picoseconds.
		Weight::from_parts(10_248_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:0 w:1)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	fn set_remove_liquidity_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 10_037_000 picoseconds.
		Weight::from_parts(10_242_000, 0)
	}
	/// Storage: `CircuitBreaker::GlobalWithdrawLimit` (r:0 w:1)
	/// Proof: `CircuitBreaker::GlobalWithdrawLimit` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	fn set_global_withdraw_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 6_000_000 picoseconds.
		Weight::from_parts(6_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::WithdrawLimitAccumulator` (r:0 w:1)
	/// Proof: `CircuitBreaker::WithdrawLimitAccumulator` (`max_values`: Some(1), `max_size`: Some(24), added: 519, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::WithdrawLockdownUntil` (r:0 w:1)
	/// Proof: `CircuitBreaker::WithdrawLockdownUntil` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn reset_withdraw_lockdown() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4`
		//  Estimated: `1493`
		// Minimum execution time: 8_000_000 picoseconds.
		Weight::from_parts(8_000_000, 1493)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `CircuitBreaker::WithdrawLockdownUntil` (r:0 w:1)
	/// Proof: `CircuitBreaker::WithdrawLockdownUntil` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	fn set_global_withdraw_lockdown() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 7_000_000 picoseconds.
		Weight::from_parts(7_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `CircuitBreaker::EgressAccounts` (r:0 w:100)
	/// Proof: `CircuitBreaker::EgressAccounts` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// The range of component `n` is `[0, 100]`.
	fn add_egress_accounts(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 6_000_000 picoseconds.
		Weight::from_parts(6_445_521, 0)
			// Standard Error: 19_059
			.saturating_add(Weight::from_parts(1_379_712, 0).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().writes((1_u64).saturating_mul(n.into())))
	}
	/// Storage: `CircuitBreaker::EgressAccounts` (r:0 w:100)
	/// Proof: `CircuitBreaker::EgressAccounts` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// The range of component `n` is `[0, 100]`.
	fn remove_egress_accounts(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 14_000_000 picoseconds.
		Weight::from_parts(8_689_492, 0)
			// Standard Error: 41_506
			.saturating_add(Weight::from_parts(1_334_551, 0).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().writes((1_u64).saturating_mul(n.into())))
	}
	/// Storage: `CircuitBreaker::GlobalAssetOverrides` (r:0 w:1)
	/// Proof: `CircuitBreaker::GlobalAssetOverrides` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `MaxEncodedLen`)
	fn set_asset_category() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_000_000 picoseconds.
		Weight::from_parts(8_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	fn ensure_add_liquidity_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `262`
		//  Estimated: `3517`
		// Minimum execution time: 22_877_000 picoseconds.
		Weight::from_parts(23_259_000, 3517)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `CircuitBreaker::LiquidityAddLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityAddLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedAddLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (r:1 w:0)
	/// Proof: `CircuitBreaker::LiquidityRemoveLimitPerAsset` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (r:1 w:1)
	/// Proof: `CircuitBreaker::AllowedRemoveLiquidityAmountPerAsset` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
	fn ensure_remove_liquidity_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `208`
		//  Estimated: `3517`
		// Minimum execution time: 19_486_000 picoseconds.
		Weight::from_parts(19_647_000, 3517)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (r:2 w:2)
	/// Proof: `CircuitBreaker::AllowedTradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `CircuitBreaker::TradeVolumeLimitPerAsset` (r:2 w:0)
	/// Proof: `CircuitBreaker::TradeVolumeLimitPerAsset` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn ensure_pool_state_change_limit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `208`
		//  Estimated: `6076`
		// Minimum execution time: 19_731_000 picoseconds.
		Weight::from_parts(19_901_000, 6076)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `CircuitBreaker::AssetLockdownState` (r:0 w:1)
	/// Proof: `CircuitBreaker::AssetLockdownState` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	fn lockdown_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 5_000_000 picoseconds.
		Weight::from_parts(5_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `CircuitBreaker::AssetLockdownState` (r:1 w:1)
	/// Proof: `CircuitBreaker::AssetLockdownState` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::TotalIssuance` (r:1 w:0)
	/// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `MaxEncodedLen`)
	fn force_lift_lockdown() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `956`
		//  Estimated: `3590`
		// Minimum execution time: 25_000_000 picoseconds.
		Weight::from_parts(26_000_000, 3590)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: `CircuitBreaker::AssetLockdownState` (r:1 w:0)
	/// Proof: `CircuitBreaker::AssetLockdownState` (`max_values`: None, `max_size`: Some(41), added: 2516, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Reserves` (r:1 w:1)
	/// Proof: `Tokens::Reserves` (`max_values`: None, `max_size`: Some(1261), added: 3736, mode: `MaxEncodedLen`)
	/// Storage: `AssetRegistry::Assets` (r:1 w:0)
	/// Proof: `AssetRegistry::Assets` (`max_values`: None, `max_size`: Some(125), added: 2600, mode: `MaxEncodedLen`)
	/// Storage: `Tokens::Accounts` (r:1 w:1)
	/// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	fn release_deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1135`
		//  Estimated: `4726`
		// Minimum execution time: 31_000_000 picoseconds.
		Weight::from_parts(32_000_000, 4726)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
}
