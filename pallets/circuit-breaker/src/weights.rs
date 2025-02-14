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
	fn on_finalize(m: u32, n: u32) -> Weight;
	fn on_finalize_single_liquidity_limit_entry() -> Weight;
	fn on_finalize_single_trade_limit_entry() -> Weight;
	fn on_finalize_empty() -> Weight;
	fn set_trade_volume_limit() -> Weight;
	fn set_add_liquidity_limit() -> Weight;
	fn set_remove_liquidity_limit() -> Weight;
	fn ensure_pool_state_change_limit() -> Weight;
	fn ensure_add_liquidity_limit() -> Weight;
	fn ensure_remove_liquidity_limit() -> Weight;
}

// For backwards compatibility and tests
impl WeightInfo for () {
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
}
