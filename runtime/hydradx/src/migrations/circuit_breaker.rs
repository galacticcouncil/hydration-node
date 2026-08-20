// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_circuit_breaker::{
	pallet, types::LockdownStatus, AssetLockdownState, LiquidityAddLimitPerAsset, LiquidityRemoveLimitPerAsset,
	TradeVolumeLimitPerAsset,
};
use sp_core::Get;
use sp_runtime::Saturating;
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationCircuitBreaker2sMigrationDone";
// 30% above the largest map's 3 entries observed on mainnet, rounded up.
const MAX_LIMIT_ENTRIES_PER_MAP: u64 = 4;
// A defensive bound for the per-asset deposit-fuse state checked by try-runtime before release.
const MAX_LOCKDOWN_STATE_ENTRIES: u64 = 100;

/// Preserves circuit-breaker wall-clock limits and deposit-fuse windows across the 6s to 2s transition.
pub struct MigrateCircuitBreakerTo2sBlocks<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> MigrateCircuitBreakerTo2sBlocks<T> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_limit((numerator, denominator): (u32, u32)) -> (u32, u32) {
		(numerator.saturating_div(3).max(1), denominator)
	}

	fn scale_future_block(
		block: frame_system::pallet_prelude::BlockNumberFor<T>,
		current_block: frame_system::pallet_prelude::BlockNumberFor<T>,
	) -> frame_system::pallet_prelude::BlockNumberFor<T> {
		if block <= current_block {
			block
		} else {
			current_block.saturating_add(block.saturating_sub(current_block).saturating_mul(3u32.into()))
		}
	}

	fn scale_past_block(
		block: frame_system::pallet_prelude::BlockNumberFor<T>,
		current_block: frame_system::pallet_prelude::BlockNumberFor<T>,
	) -> frame_system::pallet_prelude::BlockNumberFor<T> {
		if block >= current_block {
			block
		} else {
			// The configured period becomes 3x longer, so the elapsed age must also become 3x longer.
			current_block.saturating_sub(current_block.saturating_sub(block).saturating_mul(3u32.into()))
		}
	}

	fn is_within_limit(len: u64, map: &str) -> bool {
		if len <= MAX_LIMIT_ENTRIES_PER_MAP {
			true
		} else {
			log::error!(
				"MigrateCircuitBreakerTo2sBlocks skipped because {map:?} has {len:?} entries, cap: {MAX_LIMIT_ENTRIES_PER_MAP:?}",
			);
			false
		}
	}
}

impl<T: pallet::Config> OnRuntimeUpgrade for MigrateCircuitBreakerTo2sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateCircuitBreakerTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let trade_limits: Vec<_> = TradeVolumeLimitPerAsset::<T>::iter().collect();
		let add_limits: Vec<_> = LiquidityAddLimitPerAsset::<T>::iter().collect();
		let remove_limits: Vec<_> = LiquidityRemoveLimitPerAsset::<T>::iter().collect();
		let lockdown_states: Vec<_> = AssetLockdownState::<T>::iter().collect();
		let trade_limits_len = trade_limits.len();
		let add_limits_len = add_limits.len();
		let remove_limits_len = remove_limits.len();
		let lockdown_states_len = lockdown_states.len();

		let reads = 1u64
			.saturating_add(trade_limits_len as u64)
			.saturating_add(add_limits_len as u64)
			.saturating_add(remove_limits_len as u64)
			.saturating_add(lockdown_states_len as u64);

		log::info!(
			"MigrateCircuitBreakerTo2sBlocks found entries - trade: {trade_limits_len:?}, add liquidity: {add_limits_len:?}, remove liquidity: {remove_limits_len:?}, lockdown states: {lockdown_states_len:?}",
		);

		if !Self::is_within_limit(trade_limits_len as u64, "TradeVolumeLimitPerAsset")
			|| !Self::is_within_limit(add_limits_len as u64, "LiquidityAddLimitPerAsset")
			|| !Self::is_within_limit(remove_limits_len as u64, "LiquidityRemoveLimitPerAsset")
			|| lockdown_states_len as u64 > MAX_LOCKDOWN_STATE_ENTRIES
		{
			if lockdown_states_len as u64 > MAX_LOCKDOWN_STATE_ENTRIES {
				log::error!(
					"MigrateCircuitBreakerTo2sBlocks skipped because AssetLockdownState has {lockdown_states_len:?} entries, cap: {MAX_LOCKDOWN_STATE_ENTRIES:?}",
				);
			}
			return T::DbWeight::get().reads(reads);
		}

		let mut writes = 0u64;
		let current_block = frame_system::Pallet::<T>::block_number();

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

		for (asset, state) in lockdown_states {
			let state = match state {
				LockdownStatus::Locked(until) => LockdownStatus::Locked(Self::scale_future_block(until, current_block)),
				LockdownStatus::Unlocked((last_reset_at, issuance)) => {
					LockdownStatus::Unlocked((Self::scale_past_block(last_reset_at, current_block), issuance))
				}
			};
			AssetLockdownState::<T>::insert(asset, state);
			writes.saturating_inc();
		}

		Self::mark_done();
		writes.saturating_inc();

		log::info!(
			"MigrateCircuitBreakerTo2sBlocks migrated trade: {trade_limits_len:?}, add liquidity: {add_limits_len:?}, remove liquidity: {remove_limits_len:?}, lockdown states: {lockdown_states_len:?}",
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{AssetId, Balance, Runtime, System};

	#[test]
	fn migration_should_scale_limits_and_lockdown_state_when_moving_to_2s_blocks() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			let limits_asset: AssetId = 1;
			let locked_asset: AssetId = 2;
			let unlocked_asset: AssetId = 3;
			let expired_asset: AssetId = 4;
			System::set_block_number(100);

			TradeVolumeLimitPerAsset::<Runtime>::insert(limits_asset, (5_000, 10_000));
			LiquidityAddLimitPerAsset::<Runtime>::insert(limits_asset, Some((500, 10_000)));
			LiquidityRemoveLimitPerAsset::<Runtime>::insert(limits_asset, None::<(u32, u32)>);
			AssetLockdownState::<Runtime>::insert(locked_asset, LockdownStatus::Locked(110));
			AssetLockdownState::<Runtime>::insert(unlocked_asset, LockdownStatus::Unlocked((90, 1_000 as Balance)));
			AssetLockdownState::<Runtime>::insert(expired_asset, LockdownStatus::Locked(90));

			MigrateCircuitBreakerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert_eq!(TradeVolumeLimitPerAsset::<Runtime>::get(limits_asset), (1_666, 10_000));
			assert_eq!(
				LiquidityAddLimitPerAsset::<Runtime>::get(limits_asset),
				Some((166, 10_000))
			);
			assert_eq!(
				LiquidityRemoveLimitPerAsset::<Runtime>::get(limits_asset),
				None::<(u32, u32)>
			);
			assert_eq!(
				AssetLockdownState::<Runtime>::get(locked_asset),
				Some(LockdownStatus::Locked(130))
			);
			assert_eq!(
				AssetLockdownState::<Runtime>::get(unlocked_asset),
				Some(LockdownStatus::Unlocked((70, 1_000)))
			);
			assert_eq!(
				AssetLockdownState::<Runtime>::get(expired_asset),
				Some(LockdownStatus::Locked(90))
			);

			MigrateCircuitBreakerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert_eq!(TradeVolumeLimitPerAsset::<Runtime>::get(limits_asset), (1_666, 10_000));
			assert_eq!(
				AssetLockdownState::<Runtime>::get(locked_asset),
				Some(LockdownStatus::Locked(130))
			);
		});
	}
}
