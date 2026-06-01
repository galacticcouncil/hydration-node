// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_stableswap::{pallet, PoolPegs};
use sp_core::Get;
use sp_runtime::{Perbill, Saturating};
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationStableswapPegUpdate2sMigrationDone";
const MAX_POOL_PEG_ENTRIES: u64 = 15;

// `max_peg_update` is a per-block movement cap. Divide configured non-zero caps by 3
// to preserve roughly the same wall-clock peg movement when moving from 6s to 2s blocks.
pub struct MigrateStableswapMaxPegUpdateTo2sBlocks<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> MigrateStableswapMaxPegUpdateTo2sBlocks<T> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_max_peg_update(max_peg_update: Perbill) -> Perbill {
		let parts = max_peg_update.deconstruct();

		if parts == 0 {
			Perbill::zero()
		} else {
			Perbill::from_parts((parts / 3).max(1))
		}
	}
}

impl<T: pallet::Config> OnRuntimeUpgrade for MigrateStableswapMaxPegUpdateTo2sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateStableswapMaxPegUpdateTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let pool_pegs: Vec<_> = PoolPegs::<T>::iter()
			.take(MAX_POOL_PEG_ENTRIES.saturating_add(1) as usize)
			.collect();
		let reads = 1u64.saturating_add(pool_pegs.len() as u64);

		if pool_pegs.len() as u64 > MAX_POOL_PEG_ENTRIES {
			log::error!(
				"MigrateStableswapMaxPegUpdateTo2sBlocks skipped because PoolPegs has more than {:?} entries",
				MAX_POOL_PEG_ENTRIES
			);
			return T::DbWeight::get().reads(reads);
		}

		let pool_pegs_len = pool_pegs.len();
		let mut migrated = 0u64;
		let mut writes = 0u64;

		for (pool_id, mut peg_info) in pool_pegs {
			let old_max_peg_update = peg_info.max_peg_update;
			let new_max_peg_update = Self::scale_max_peg_update(old_max_peg_update);

			if old_max_peg_update != new_max_peg_update {
				peg_info.max_peg_update = new_max_peg_update;
				PoolPegs::<T>::insert(pool_id, peg_info);
				writes.saturating_inc();
				migrated.saturating_inc();
			}
		}

		Self::mark_done();
		writes.saturating_inc();

		log::info!(
			"MigrateStableswapMaxPegUpdateTo2sBlocks checked pools: {:?}, migrated: {:?}",
			pool_pegs_len,
			migrated,
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{AssetId, Runtime};
	use frame_support::BoundedVec;
	use pallet_stableswap::types::{PegSource, PoolPegInfo};

	fn peg_info(max_peg_update: Perbill) -> PoolPegInfo<BlockNumberFor<Runtime>, AssetId> {
		PoolPegInfo {
			source: BoundedVec::truncate_from(vec![PegSource::Value((1, 1))]),
			updated_at: 1,
			max_peg_update,
			current: BoundedVec::truncate_from(vec![(1, 1)]),
		}
	}

	type BlockNumberFor<T> = frame_system::pallet_prelude::BlockNumberFor<T>;

	#[test]
	fn migrate_stableswap_max_peg_update_to_2s_blocks_works() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			let non_zero_pool: AssetId = 690;
			let zero_pool: AssetId = 143;

			PoolPegs::<Runtime>::insert(non_zero_pool, peg_info(Perbill::from_percent(6)));
			PoolPegs::<Runtime>::insert(zero_pool, peg_info(Perbill::zero()));

			MigrateStableswapMaxPegUpdateTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert_eq!(
				PoolPegs::<Runtime>::get(non_zero_pool).unwrap().max_peg_update,
				Perbill::from_percent(2)
			);
			assert_eq!(
				PoolPegs::<Runtime>::get(zero_pool).unwrap().max_peg_update,
				Perbill::zero()
			);

			MigrateStableswapMaxPegUpdateTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert_eq!(
				PoolPegs::<Runtime>::get(non_zero_pool).unwrap().max_peg_update,
				Perbill::from_percent(2)
			);
		});
	}
}
