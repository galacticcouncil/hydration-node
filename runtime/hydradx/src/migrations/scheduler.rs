// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight, BoundedVec};
use pallet_scheduler::{pallet, BlockNumberFor, ScheduledOf};
use sp_core::Get;
use sp_runtime::{traits::BlockNumberProvider, Saturating};
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationScheduler2sBlockMigrationDone";

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
		let agenda: Vec<(
			BlockNumberFor<T>,
			BoundedVec<Option<ScheduledOf<T>>, T::MaxScheduledPerBlock>,
		)> = pallet_scheduler::Agenda::<T>::iter().collect();
		let agenda_len = agenda.len() as u64;

		let lookup: Vec<_> = pallet_scheduler::Lookup::<T>::iter().collect();
		let lookup_len = lookup.len() as u64;

		if agenda_len >= 150 {
			log::error!("Error: more than 150 agendas exist, len: {:?}", agenda_len);
			return T::DbWeight::get().reads_writes(agenda_len.saturating_add(lookup_len).saturating_add(1), 0);
		}

		// We expect Lookup to be empty on-chain, but migrate up to 5 entries defensively in case
		// any named schedules exist at upgrade time. If there are more, skip only Lookup migration.
		let migrate_lookup = lookup_len <= 5;
		if !migrate_lookup {
			log::error!(
				"Skipping Scheduler Lookup migration because more than 5 entries exist, len: {:?}",
				lookup_len
			);
		}

		for (old_block, mut schedules) in agenda {
			for scheduled in schedules.iter_mut().flatten() {
				if let Some((period, _remaining)) = scheduled.maybe_periodic.as_mut() {
					*period = period.saturating_mul(3u32.into());
				}
			}

			let new_block = Self::scale_block(old_block, current_block);

			pallet_scheduler::Agenda::<T>::remove(old_block);
			pallet_scheduler::Agenda::<T>::insert(new_block, schedules);
		}

		let lookup_writes = if migrate_lookup { lookup_len } else { 0 };
		if migrate_lookup {
			for (name, (block, index)) in lookup {
				pallet_scheduler::Lookup::<T>::insert(name, (Self::scale_block(block, current_block), index));
			}
		}

		Self::mark_done();

		log::info!(
			"MigrateSchedulerTo2sBlocks processed agenda items: {:?}, lookup entries: {:?}, lookup migrated: {:?}",
			agenda_len,
			lookup_len,
			migrate_lookup
		);
		T::DbWeight::get().reads_writes(
			agenda_len.saturating_add(lookup_len).saturating_add(1),
			agenda_len
				.saturating_mul(2)
				.saturating_add(lookup_writes)
				.saturating_add(1),
		)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{Runtime, RuntimeCall, RuntimeOrigin, Scheduler, System};
	use frame_support::assert_ok;

	#[test]
	fn migrate_scheduler_to_2s_blocks_works() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(0);

			let periodic_call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![1],
			}));
			let named_call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![2],
			}));
			let named_id = [7u8; 32];

			assert_ok!(Scheduler::schedule(
				RuntimeOrigin::root(),
				200,
				Some((10, 3)),
				3,
				periodic_call
			));
			assert_ok!(Scheduler::schedule_named(
				RuntimeOrigin::root(),
				named_id,
				220,
				None,
				3,
				named_call
			));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(200));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(220));
			assert_eq!(pallet_scheduler::Lookup::<Runtime>::get(named_id), Some((220, 0)));

			System::set_block_number(100);
			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(200));
			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(220));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(400));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(460));
			let migrated_agenda = pallet_scheduler::Agenda::<Runtime>::get(400);
			let migrated_schedule = migrated_agenda.get(0).and_then(Option::as_ref).unwrap();
			assert_eq!(migrated_schedule.maybe_periodic, Some((30, 2)));
			assert_eq!(pallet_scheduler::Lookup::<Runtime>::get(named_id), Some((460, 0)));

			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(400));
			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(1000));
		})
	}
}
