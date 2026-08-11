// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_scheduler::{pallet, BlockNumberFor};
use sp_core::Get;
use sp_runtime::{traits::BlockNumberProvider, Saturating};
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationScheduler2sBlockMigrationDone";
// 30% above the 470 agenda entries observed on mainnet.
const MAX_AGENDA_ENTRIES: u64 = 611;

// This migration migrates the Scheduler to 2s block times by multiplying by 3 the spread between
// stored scheduler block numbers and the current block, and by multiplying periodic intervals by 3.
//
// The migration uses a raw storage marker to prevent accidental double execution. Make sure it is
// removed from the Runtime Executive after it has been run.
pub struct MigrateSchedulerTo2sBlocks<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> MigrateSchedulerTo2sBlocks<T> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_block(block: BlockNumberFor<T>, current_block: BlockNumberFor<T>) -> BlockNumberFor<T> {
		let old_spread = block.saturating_sub(current_block);
		let new_spread = old_spread.saturating_mul(3u32.into());
		current_block.saturating_add(new_spread)
	}
}

impl<T: pallet::Config> OnRuntimeUpgrade for MigrateSchedulerTo2sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateSchedulerTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let current_block = T::BlockNumberProvider::current_block_number();
		let agenda_len = pallet_scheduler::Agenda::<T>::iter().count() as u64;

		log::info!("MigrateSchedulerTo2sBlocks found Agenda entries: {agenda_len:?}, cap: {MAX_AGENDA_ENTRIES:?}",);

		if agenda_len > MAX_AGENDA_ENTRIES {
			log::error!(
				"MigrateSchedulerTo2sBlocks skipped because Agenda has {agenda_len:?} entries, cap: {MAX_AGENDA_ENTRIES:?}",
			);
			return T::DbWeight::get().reads_writes(agenda_len.saturating_add(1), 0);
		}

		// Remove every old key before inserting scaled keys, as a destination may still be an original key.
		let agenda = pallet_scheduler::Agenda::<T>::drain().collect::<Vec<_>>();

		for (old_block, mut schedules) in agenda {
			for scheduled in schedules.iter_mut().flatten() {
				if let Some((period, _remaining)) = scheduled.maybe_periodic.as_mut() {
					*period = period.saturating_mul(3u32.into());
				}
			}

			let new_block = Self::scale_block(old_block, current_block);

			pallet_scheduler::Agenda::<T>::insert(new_block, schedules);
		}

		Self::mark_done();

		log::info!("MigrateSchedulerTo2sBlocks processed agenda items: {agenda_len:?}",);
		T::DbWeight::get().reads_writes(
			agenda_len.saturating_mul(2).saturating_add(1),
			agenda_len.saturating_mul(2).saturating_add(1),
		)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{Runtime, RuntimeCall, RuntimeOrigin, Scheduler, System};
	use frame_support::assert_ok;

	#[test]
	fn migration_should_preserve_agenda_when_scaled_key_matches_original_key() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(0);

			let periodic_call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![1],
			}));
			assert_ok!(Scheduler::schedule(
				RuntimeOrigin::root(),
				200,
				Some((10, 3)),
				3,
				periodic_call
			));
			let colliding_call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![2],
			}));
			assert_ok!(Scheduler::schedule(RuntimeOrigin::root(), 400, None, 3, colliding_call));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(200));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(400));

			System::set_block_number(100);
			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(200));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(400));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(1000));
			let migrated_agenda = pallet_scheduler::Agenda::<Runtime>::get(400);
			let migrated_schedule = migrated_agenda.first().and_then(Option::as_ref).unwrap();
			assert_eq!(migrated_schedule.maybe_periodic, Some((30, 2)));

			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(400));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(1000));
		})
	}
}
