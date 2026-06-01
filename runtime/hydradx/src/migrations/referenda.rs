// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{
	traits::{schedule::DispatchTime, OnRuntimeUpgrade},
	weights::Weight,
};
use pallet_referenda::{pallet, BlockNumberFor, ReferendumInfo, ReferendumInfoFor, ScheduleAddressOf};
use sp_core::Get;
use sp_runtime::traits::{BlockNumberProvider, Saturating};
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationReferenda2sBlockMigrationDone";
const MAX_ACTIVE_REFERENDA: u64 = 30;

// Minimal OpenGov active-state migration for the 6s -> 2s block-time change.
//
// `TrackQueue` does not store block numbers in this SDK version; it stores referendum indices and
// votes only. This migration updates only ongoing `ReferendumInfoFor` statuses and keeps their
// wall-clock lifecycle timing roughly unchanged.
pub struct MigrateReferendaTo2sBlocks<T: pallet::Config<I>, I: 'static = ()>(PhantomData<(T, I)>);

impl<T: pallet::Config<I>, I: 'static> MigrateReferendaTo2sBlocks<T, I> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_future_block(block: BlockNumberFor<T, I>, current_block: BlockNumberFor<T, I>) -> BlockNumberFor<T, I> {
		if block <= current_block {
			block
		} else {
			current_block.saturating_add(block.saturating_sub(current_block).saturating_mul(3u32.into()))
		}
	}

	fn scale_anchor_block(block: BlockNumberFor<T, I>, current_block: BlockNumberFor<T, I>) -> BlockNumberFor<T, I> {
		if block <= current_block {
			current_block.saturating_sub(current_block.saturating_sub(block).saturating_mul(3u32.into()))
		} else {
			Self::scale_future_block(block, current_block)
		}
	}

	fn scale_dispatch_time(
		dispatch_time: DispatchTime<BlockNumberFor<T, I>>,
		current_block: BlockNumberFor<T, I>,
	) -> DispatchTime<BlockNumberFor<T, I>> {
		match dispatch_time {
			DispatchTime::At(block) => DispatchTime::At(Self::scale_future_block(block, current_block)),
			DispatchTime::After(blocks) => DispatchTime::After(blocks.saturating_mul(3u32.into())),
		}
	}
}

impl<T, I> OnRuntimeUpgrade for MigrateReferendaTo2sBlocks<T, I>
where
	T: pallet::Config<I>,
	I: 'static,
	ScheduleAddressOf<T, I>: Into<(BlockNumberFor<T, I>, u32)>,
	(BlockNumberFor<T, I>, u32): Into<ScheduleAddressOf<T, I>>,
{
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateReferendaTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let mut reads = 1u64;
		let mut ongoing_referenda = Vec::new();

		for (index, info) in ReferendumInfoFor::<T, I>::iter() {
			reads.saturating_inc();

			if let ReferendumInfo::Ongoing(status) = info {
				if ongoing_referenda.len() as u64 >= MAX_ACTIVE_REFERENDA {
					log::error!(
						"MigrateReferendaTo2sBlocks skipped because ReferendumInfoFor has more than {:?} ongoing referenda",
						MAX_ACTIVE_REFERENDA
					);
					return T::DbWeight::get().reads(reads);
				}

				ongoing_referenda.push((index, status));
			}
		}

		let current_block = <T as pallet::Config<I>>::BlockNumberProvider::current_block_number();
		let checked = reads.saturating_sub(1);
		let ongoing_len = ongoing_referenda.len();
		let mut migrated = 0u64;
		let mut writes = 0u64;

		for (index, mut status) in ongoing_referenda {
			status.enactment = Self::scale_dispatch_time(status.enactment, current_block);
			status.submitted = Self::scale_anchor_block(status.submitted, current_block);

			if let Some(deciding) = status.deciding.as_mut() {
				deciding.since = Self::scale_anchor_block(deciding.since, current_block);
				if let Some(confirming) = deciding.confirming.as_mut() {
					*confirming = Self::scale_future_block(*confirming, current_block);
				}
			}

			if let Some((alarm_at, address)) = status.alarm.as_mut() {
				*alarm_at = Self::scale_future_block(*alarm_at, current_block);

				let (address_block, address_index) = address.clone().into();
				*address = (Self::scale_future_block(address_block, current_block), address_index).into();
			}

			ReferendumInfoFor::<T, I>::insert(index, ReferendumInfo::Ongoing(status));
			writes.saturating_inc();
			migrated.saturating_inc();
		}

		Self::mark_done();
		writes.saturating_inc();

		log::info!(
			"MigrateReferendaTo2sBlocks checked referenda records: {:?}, ongoing: {:?}, migrated ongoing: {:?}",
			checked,
			ongoing_len,
			migrated,
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}
