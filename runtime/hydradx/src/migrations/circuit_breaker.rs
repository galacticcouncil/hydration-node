// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_circuit_breaker::{
	pallet, LiquidityAddLimitPerAsset, LiquidityRemoveLimitPerAsset, TradeVolumeLimitPerAsset,
};
use sp_core::Get;
use sp_runtime::Saturating;
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationCircuitBreaker2sLimitMigrationDone";
const MAX_LIMIT_ENTRIES_PER_MAP: u64 = 10;

pub struct MigrateCircuitBreakerLimitsTo2sBlocks<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> MigrateCircuitBreakerLimitsTo2sBlocks<T> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_limit((numerator, denominator): (u32, u32)) -> (u32, u32) {
		(numerator.saturating_div(3).max(1), denominator)
	}

	fn is_within_limit(len: u64, map: &str) -> bool {
		if len <= MAX_LIMIT_ENTRIES_PER_MAP {
			true
		} else {
			log::error!(
				"MigrateCircuitBreakerLimitsTo2sBlocks skipped because {:?} has more than {:?} entries",
				map,
				MAX_LIMIT_ENTRIES_PER_MAP
			);
			false
		}
	}
}

impl<T: pallet::Config> OnRuntimeUpgrade for MigrateCircuitBreakerLimitsTo2sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateCircuitBreakerLimitsTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let trade_limits: Vec<_> = TradeVolumeLimitPerAsset::<T>::iter()
			.take(MAX_LIMIT_ENTRIES_PER_MAP.saturating_add(1) as usize)
			.collect();
		let add_limits: Vec<_> = LiquidityAddLimitPerAsset::<T>::iter()
			.take(MAX_LIMIT_ENTRIES_PER_MAP.saturating_add(1) as usize)
			.collect();
		let remove_limits: Vec<_> = LiquidityRemoveLimitPerAsset::<T>::iter()
			.take(MAX_LIMIT_ENTRIES_PER_MAP.saturating_add(1) as usize)
			.collect();

		let reads = 1u64
			.saturating_add(trade_limits.len() as u64)
			.saturating_add(add_limits.len() as u64)
			.saturating_add(remove_limits.len() as u64);

		if !Self::is_within_limit(trade_limits.len() as u64, "TradeVolumeLimitPerAsset")
			|| !Self::is_within_limit(add_limits.len() as u64, "LiquidityAddLimitPerAsset")
			|| !Self::is_within_limit(remove_limits.len() as u64, "LiquidityRemoveLimitPerAsset")
		{
			return T::DbWeight::get().reads(reads);
		}

		let trade_limits_len = trade_limits.len();
		let add_limits_len = add_limits.len();
		let remove_limits_len = remove_limits.len();
		let mut writes = 0u64;

		for (asset, limit) in trade_limits {
			TradeVolumeLimitPerAsset::<T>::insert(asset, Self::scale_limit(limit));
			writes.saturating_inc();
		}

		for (asset, limit) in add_limits {
			LiquidityAddLimitPerAsset::<T>::insert(asset, limit.map(Self::scale_limit));
			writes.saturating_inc();
		}

		for (asset, limit) in remove_limits {
			LiquidityRemoveLimitPerAsset::<T>::insert(asset, limit.map(Self::scale_limit));
			writes.saturating_inc();
		}

		Self::mark_done();
		writes.saturating_inc();

		log::info!(
			"MigrateCircuitBreakerLimitsTo2sBlocks migrated trade: {:?}, add liquidity: {:?}, remove liquidity: {:?}",
			trade_limits_len,
			add_limits_len,
			remove_limits_len,
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{AssetId, Runtime};

	#[test]
	fn migrate_circuit_breaker_limits_to_2s_blocks_works() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			let asset: AssetId = 1;

			TradeVolumeLimitPerAsset::<Runtime>::insert(asset, (5_000, 10_000));
			LiquidityAddLimitPerAsset::<Runtime>::insert(asset, Some((500, 10_000)));
			LiquidityRemoveLimitPerAsset::<Runtime>::insert(asset, None::<(u32, u32)>);

			MigrateCircuitBreakerLimitsTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert_eq!(TradeVolumeLimitPerAsset::<Runtime>::get(asset), (1_666, 10_000));
			assert_eq!(LiquidityAddLimitPerAsset::<Runtime>::get(asset), Some((166, 10_000)));
			assert_eq!(LiquidityRemoveLimitPerAsset::<Runtime>::get(asset), None::<(u32, u32)>);

			MigrateCircuitBreakerLimitsTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert_eq!(TradeVolumeLimitPerAsset::<Runtime>::get(asset), (1_666, 10_000));
			assert_eq!(LiquidityAddLimitPerAsset::<Runtime>::get(asset), Some((166, 10_000)));
		});
	}
}
