use crate::pallet;
use frame_support::{
	pallet_prelude::ValueQuery,
	storage_alias,
	traits::{Get, GetStorageVersion, OnRuntimeUpgrade, StorageVersion},
	Blake2_128Concat, BoundedVec,
};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, ConstU32, One},
	Saturating,
};
use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

// 30% above the 38 schedules observed on mainnet, rounded up.
const MAX_SCHEDULES: u64 = 50;
const OLD_MAX_SCHEDULES_PER_BLOCK: u32 = 6;

mod v2 {
	use super::*;

	#[storage_alias]
	pub type ScheduleIdsPerBlock<T: pallet::Config> = StorageMap<
		crate::Pallet<T>,
		Blake2_128Concat,
		frame_system::pallet_prelude::BlockNumberFor<T>,
		BoundedVec<crate::types::ScheduleId, ConstU32<OLD_MAX_SCHEDULES_PER_BLOCK>>,
		ValueQuery,
	>;
}

fn redistribute_schedule_ids<BlockNumber>(
	mut schedule_ids_per_block: Vec<(BlockNumber, Vec<crate::types::ScheduleId>)>,
	max_schedules_per_block: u32,
) -> Option<BTreeMap<BlockNumber, Vec<crate::types::ScheduleId>>>
where
	BlockNumber: AtLeast32BitUnsigned + Copy + Ord,
{
	if max_schedules_per_block == 0 {
		return None;
	}

	schedule_ids_per_block.sort_by_key(|(block, _)| *block);
	let mut redistributed = BTreeMap::<BlockNumber, Vec<crate::types::ScheduleId>>::new();

	for (block, schedule_ids) in schedule_ids_per_block {
		for schedule_id in schedule_ids {
			let mut destination = block;
			loop {
				let destination_ids = redistributed.entry(destination).or_default();
				if destination_ids.len() < max_schedules_per_block as usize {
					destination_ids.push(schedule_id);
					break;
				}

				let next_destination = destination.saturating_add(One::one());
				if next_destination == destination {
					return None;
				}
				destination = next_destination;
			}
		}
	}

	Some(redistributed)
}

// This migration updates DCA schedules for 2s blocks and redistributes execution queues to the new
// per-block limit before they are decoded with the lower bound.
pub struct MigrateSchedulesTo2sBlocks<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MigrateSchedulesTo2sBlocks<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(2),
			"DCA storage version must be v2 before migrating schedules to 2s blocks"
		);

		Ok(sp_std::vec::Vec::new())
	}

	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		let mut reads = 0u64;
		let mut writes = 0u64;

		let on_chain_version = StorageVersion::get::<crate::Pallet<T>>();
		let in_code_version = crate::Pallet::<T>::in_code_storage_version();
		reads.saturating_inc();

		if on_chain_version >= in_code_version {
			// Already migrated
			return T::DbWeight::get().reads(reads);
		}

		if on_chain_version != StorageVersion::new(2) {
			log::warn!("DCA 2s block migration skipped: expected storage version 2, got {on_chain_version:?}");
			return T::DbWeight::get().reads(reads);
		}

		let collection_limit = MAX_SCHEDULES.saturating_add(1) as usize;
		let schedules: Vec<_> = crate::Schedules::<T>::iter().take(collection_limit).collect();
		let schedules_len = schedules.len() as u64;
		reads.saturating_accrue(schedules_len);

		if schedules_len > MAX_SCHEDULES {
			log::error!("MigrateSchedulesTo2sBlocks skipped because Schedules exceeds the cap: {MAX_SCHEDULES:?}",);
			return T::DbWeight::get().reads(reads);
		}

		let schedule_ids_per_block: Vec<_> = v2::ScheduleIdsPerBlock::<T>::iter()
			.take(collection_limit)
			.map(|(block, schedule_ids)| (block, schedule_ids.into_inner()))
			.collect();
		let schedule_blocks_len = schedule_ids_per_block.len() as u64;
		let old_schedule_blocks = schedule_ids_per_block
			.iter()
			.map(|(block, _)| *block)
			.collect::<Vec<_>>();
		let scheduled_ids_len = schedule_ids_per_block
			.iter()
			.map(|(_, schedule_ids)| schedule_ids.len() as u64)
			.fold(0u64, u64::saturating_add);
		reads.saturating_accrue(schedule_blocks_len);

		log::info!(
			"MigrateSchedulesTo2sBlocks found schedules: {schedules_len:?}, scheduled ids: {scheduled_ids_len:?}, processing cap: {MAX_SCHEDULES:?}",
		);

		if schedule_blocks_len > MAX_SCHEDULES || scheduled_ids_len > MAX_SCHEDULES {
			log::error!(
				"MigrateSchedulesTo2sBlocks skipped because Schedules has {schedules_len:?} entries and ScheduleIdsPerBlock has {schedule_blocks_len:?} blocks with {scheduled_ids_len:?} ids, cap: {MAX_SCHEDULES:?}",
			);
			return T::DbWeight::get().reads(reads);
		}

		let Some(redistributed) = redistribute_schedule_ids(schedule_ids_per_block, T::MaxSchedulePerBlock::get())
		else {
			log::error!("MigrateSchedulesTo2sBlocks skipped because schedule ids could not be redistributed");
			return T::DbWeight::get().reads(reads);
		};

		for (key, mut schedule) in schedules {
			schedule.period = schedule.period.saturating_mul(3u32.into());
			crate::Schedules::<T>::insert(key, schedule);
			writes.saturating_inc();
		}

		for block in old_schedule_blocks {
			v2::ScheduleIdsPerBlock::<T>::remove(block);
			writes.saturating_inc();
		}

		for (block, schedule_ids) in redistributed {
			let Ok(bounded_schedule_ids) = BoundedVec::<_, T::MaxSchedulePerBlock>::try_from(schedule_ids) else {
				unreachable!("redistributed schedule ids respect MaxSchedulePerBlock")
			};
			for schedule_id in bounded_schedule_ids.iter() {
				crate::ScheduleExecutionBlock::<T>::insert(schedule_id, block);
				writes.saturating_inc();
			}
			crate::ScheduleIdsPerBlock::<T>::insert(block, bounded_schedule_ids);
			writes.saturating_inc();
		}

		// Increase on-chain StorageVersion
		StorageVersion::new(3).put::<crate::Pallet<T>>();
		writes.saturating_inc();

		log::info!("MigrateSchedulesTo2sBlocks processed schedules: {schedules_len:?}");
		T::DbWeight::get().reads_writes(reads, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(3),
			"DCA storage version must be v3 after migrating schedules to 2s blocks"
		);
		assert!(
			crate::ScheduleIdsPerBlock::<T>::iter_values()
				.all(|schedule_ids| schedule_ids.len() <= T::MaxSchedulePerBlock::get() as usize),
			"DCA execution queues must respect MaxSchedulePerBlock"
		);

		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::tests::mock::{RuntimeOrigin, Test, ALICE, DCA};
	use crate::tests::schedule::set_block_number;
	use crate::tests::{
		mock::{ExtBuilder, HDX, ONE},
		ScheduleBuilder,
	};
	use frame_support::assert_ok;

	#[test]
	fn redistribution_should_spread_schedules_when_old_block_exceeds_new_limit() {
		let redistributed =
			redistribute_schedule_ids(vec![(600u64, vec![0, 1, 2, 3, 4]), (601u64, vec![5])], 2).unwrap();

		assert_eq!(redistributed.get(&600), Some(&vec![0, 1]));
		assert_eq!(redistributed.get(&601), Some(&vec![2, 3]));
		assert_eq!(redistributed.get(&602), Some(&vec![4, 5]));
	}

	#[test]
	fn migration_should_update_schedules_for_2s_blocks() {
		ExtBuilder::default()
			.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
			.build()
			.execute_with(|| {
				// Arrange
				let schedule = ScheduleBuilder::new().build();
				set_block_number(500);
				assert_ok!(DCA::schedule(
					RuntimeOrigin::signed(ALICE),
					schedule.clone(),
					Option::None
				));

				let stored_schedule = DCA::schedules(0).unwrap();
				assert_eq!(stored_schedule.period, 100);
				let execution_block = DCA::schedule_execution_block(0).unwrap();
				StorageVersion::new(2).put::<DCA>();

				// Act
				MigrateSchedulesTo2sBlocks::<Test>::on_runtime_upgrade();
				let updated_schedule = DCA::schedules(0).unwrap();

				// Assert
				assert_eq!(updated_schedule.period, 300);
				assert_eq!(DCA::schedule_execution_block(0), Some(execution_block));
				assert_eq!(DCA::schedule_ids_per_block(execution_block).to_vec(), vec![0]);

				// Storage version has been updated
				let on_chain_version = StorageVersion::get::<DCA>();
				assert_eq!(on_chain_version, StorageVersion::new(3));
			});
	}
}
