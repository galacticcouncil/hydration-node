// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::{Decode, Encode};
use frame_support::{
	pallet_prelude::OptionQuery, storage_alias, traits::OnRuntimeUpgrade, weights::Weight, Blake2_128Concat, BoundedVec,
};
use pallet_scheduler::{pallet, BlockNumberFor, TaskAddress};
use sp_core::Get;
use sp_runtime::{traits::BlockNumberProvider, Saturating};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationScheduler2sBlockMigrationDone";
// 30% above the 470 agenda entries observed on mainnet.
const MAX_AGENDA_ENTRIES: u64 = 611;

mod scheduler_storage {
	use super::*;

	// Mirrors pallet-scheduler's private fields so retry periods can be scaled in place.
	#[derive(Decode, Encode)]
	pub struct RetryConfig<BlockNumber> {
		pub total_retries: u8,
		pub remaining: u8,
		pub period: BlockNumber,
	}

	#[storage_alias]
	pub type Retries<T: pallet::Config> = StorageMap<
		pallet_scheduler::Pallet<T>,
		Blake2_128Concat,
		TaskAddress<BlockNumberFor<T>>,
		RetryConfig<BlockNumberFor<T>>,
		OptionQuery,
	>;
}

// This migration migrates the Scheduler to 2s block times by multiplying by 3 the spread between
// stored scheduler block numbers and the current block, multiplying periodic and retry intervals by 3,
// and updating task addresses in Lookup and Retries.
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
		let mut agenda_len = 0u64;
		let mut destination_lengths = BTreeMap::new();

		for (old_block, schedules) in pallet_scheduler::Agenda::<T>::iter() {
			agenda_len = agenda_len.saturating_add(1);
			let new_block = Self::scale_block(old_block, current_block);
			let destination_len = destination_lengths.entry(new_block).or_insert(0usize);
			*destination_len = destination_len.saturating_add(schedules.len());
		}

		log::info!("MigrateSchedulerTo2sBlocks found Agenda entries: {agenda_len:?}, cap: {MAX_AGENDA_ENTRIES:?}",);

		if agenda_len > MAX_AGENDA_ENTRIES {
			log::error!(
				"MigrateSchedulerTo2sBlocks skipped because Agenda has {agenda_len:?} entries, cap: {MAX_AGENDA_ENTRIES:?}",
			);
			return T::DbWeight::get().reads(agenda_len.saturating_add(1));
		}

		if destination_lengths
			.values()
			.any(|len| *len > T::MaxScheduledPerBlock::get() as usize)
		{
			log::error!("MigrateSchedulerTo2sBlocks skipped because merged Agenda exceeds MaxScheduledPerBlock");
			return T::DbWeight::get().reads(agenda_len.saturating_add(1));
		}

		// Remove every old key before inserting scaled keys, as a destination may still be an original key.
		let agenda = pallet_scheduler::Agenda::<T>::drain().collect::<Vec<_>>();
		let mut migrated_agenda: BTreeMap<
			BlockNumberFor<T>,
			BoundedVec<Option<pallet_scheduler::ScheduledOf<T>>, T::MaxScheduledPerBlock>,
		> = BTreeMap::new();
		let mut migrated_addresses: BTreeMap<TaskAddress<BlockNumberFor<T>>, TaskAddress<BlockNumberFor<T>>> =
			BTreeMap::new();

		for (old_block, mut schedules) in agenda {
			for scheduled in schedules.iter_mut().flatten() {
				if let Some((period, _remaining)) = scheduled.maybe_periodic.as_mut() {
					*period = period.saturating_mul(3u32.into());
				}
			}

			let new_block = Self::scale_block(old_block, current_block);
			let destination = migrated_agenda.entry(new_block).or_default();
			let destination_offset = destination.len() as u32;

			for old_index in 0..schedules.len() as u32 {
				migrated_addresses.insert(
					(old_block, old_index),
					(new_block, destination_offset.saturating_add(old_index)),
				);
			}

			if destination.try_extend(schedules.into_iter()).is_err() {
				unreachable!("merged Agenda capacity was checked before draining storage")
			}
		}

		for (new_block, schedules) in migrated_agenda {
			pallet_scheduler::Agenda::<T>::insert(new_block, schedules);
		}

		let lookup = pallet_scheduler::Lookup::<T>::drain().collect::<Vec<_>>();
		let lookup_len = lookup.len() as u64;
		for (name, old_address) in lookup {
			let new_address = migrated_addresses
				.get(&old_address)
				.copied()
				.unwrap_or_else(|| (Self::scale_block(old_address.0, current_block), old_address.1));
			pallet_scheduler::Lookup::<T>::insert(name, new_address);
		}

		let retries = scheduler_storage::Retries::<T>::drain().collect::<Vec<_>>();
		let retries_len = retries.len() as u64;
		for (old_address, mut retry) in retries {
			retry.period = retry.period.saturating_mul(3u32.into());
			let new_address = migrated_addresses
				.get(&old_address)
				.copied()
				.unwrap_or_else(|| (Self::scale_block(old_address.0, current_block), old_address.1));
			scheduler_storage::Retries::<T>::insert(new_address, retry);
		}

		Self::mark_done();

		log::info!("MigrateSchedulerTo2sBlocks processed agenda items: {agenda_len:?}",);
		T::DbWeight::get().reads_writes(
			agenda_len
				.saturating_mul(2)
				.saturating_add(lookup_len)
				.saturating_add(retries_len)
				.saturating_add(1),
			agenda_len
				.saturating_mul(2)
				.saturating_add(lookup_len.saturating_mul(2))
				.saturating_add(retries_len.saturating_mul(2))
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

	#[test]
	fn migration_should_merge_agendas_when_multiple_keys_scale_to_same_block() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(0);

			let first_call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![1],
			}));
			assert_ok!(Scheduler::schedule(RuntimeOrigin::root(), 10, None, 3, first_call));

			let anonymous_call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![2],
			}));
			assert_ok!(Scheduler::schedule(RuntimeOrigin::root(), 20, None, 3, anonymous_call,));

			System::set_block_number(100);
			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(10));
			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(20));
			let merged_agenda = pallet_scheduler::Agenda::<Runtime>::get(100);
			assert_eq!(merged_agenda.iter().flatten().count(), 2);
		})
	}

	#[test]
	fn migration_should_remap_lookup_when_named_schedule_is_migrated() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(0);
			let task_name = [1u8; 32];
			let call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![1],
			}));
			assert_ok!(Scheduler::schedule_named(
				RuntimeOrigin::root(),
				task_name,
				10,
				None,
				3,
				call,
			));

			System::set_block_number(5);
			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(10));
			let migrated_address = pallet_scheduler::Lookup::<Runtime>::get(task_name).unwrap();
			assert_eq!(migrated_address.0, 20);
			let migrated_schedule = pallet_scheduler::Agenda::<Runtime>::get(migrated_address.0)
				.get(migrated_address.1 as usize)
				.cloned()
				.flatten()
				.unwrap();
			assert_eq!(migrated_schedule.maybe_id, Some(task_name));
		})
	}

	#[test]
	fn migration_should_remap_retry_and_scale_period_when_schedule_is_migrated() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(0);
			let call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
				remark: vec![1],
			}));
			assert_ok!(Scheduler::schedule(RuntimeOrigin::root(), 10, None, 3, call));
			assert_ok!(Scheduler::set_retry(RuntimeOrigin::root(), (10, 0), 2, 5));

			System::set_block_number(5);
			MigrateSchedulerTo2sBlocks::<Runtime>::on_runtime_upgrade();

			assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(10));
			assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(20));
			assert!(!scheduler_storage::Retries::<Runtime>::contains_key((10, 0)));
			let migrated_retry = scheduler_storage::Retries::<Runtime>::get((20, 0)).unwrap();
			assert_eq!(migrated_retry.total_retries, 2);
			assert_eq!(migrated_retry.remaining, 2);
			assert_eq!(migrated_retry.period, 15);
		})
	}
}
