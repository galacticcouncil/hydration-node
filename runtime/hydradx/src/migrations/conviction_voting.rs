// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_conviction_voting::{pallet, BlockNumberFor, PriorLock, VotingFor};
use sp_core::Get;
use sp_runtime::traits::{BlockNumberProvider, Saturating};
use sp_std::marker::PhantomData;

const MIGRATION_DONE_KEY: &[u8] = b"HydrationConvictionVoting2sBlockMigrationDone";

// This migration preserves existing conviction-voting prior-lock wall-clock unlock times when
// moving from 6s to 2s blocks. Only `VotingFor` stores block numbers; `ClassLocksFor` stores
// classes and balances only.
//
// The migration uses a raw storage marker to prevent accidental double execution. Make sure it is
// removed from the Runtime Executive after it has been run.
pub struct MigrateConvictionVotingTo2sBlocks<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> MigrateConvictionVotingTo2sBlocks<T> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_future_block(block: BlockNumberFor<T>, current_block: BlockNumberFor<T>) -> BlockNumberFor<T> {
		if block <= current_block {
			block
		} else {
			current_block.saturating_add(block.saturating_sub(current_block).saturating_mul(3u32.into()))
		}
	}
}

impl<T: pallet::Config> OnRuntimeUpgrade for MigrateConvictionVotingTo2sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateConvictionVotingTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let current_block = T::BlockNumberProvider::current_block_number();
		let mut reads = 1u64;
		let mut writes = 0u64;
		let mut migrated = 0u64;

		for (who, class, mut voting) in VotingFor::<T>::iter() {
			reads.saturating_inc();
			let prior: &mut PriorLock<BlockNumberFor<T>, _> = voting.as_mut();
			let old_until = prior.0;
			let new_until = Self::scale_future_block(old_until, current_block);

			if old_until != new_until {
				prior.0 = new_until;
				VotingFor::<T>::insert(who, class, voting);
				writes.saturating_inc();
				migrated.saturating_inc();
			}
		}

		Self::mark_done();
		writes.saturating_inc();

		log::info!("MigrateConvictionVotingTo2sBlocks migrated prior locks: {:?}", migrated);
		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{AccountId, Runtime, System};
	use pallet_conviction_voting::{Casting, Voting};

	fn prior_until(who: AccountId) -> BlockNumberFor<Runtime> {
		match VotingFor::<Runtime>::get(who, 0u16) {
			Voting::Casting(Casting { prior, .. }) => prior.0,
			Voting::Delegating(delegating) => delegating.prior.0,
		}
	}

	#[test]
	fn migrate_conviction_voting_to_2s_blocks_works() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			let who = AccountId::new([1; 32]);
			System::set_block_number(100);
			VotingFor::<Runtime>::insert(
				who.clone(),
				0u16,
				Voting::Casting(Casting {
					votes: Default::default(),
					delegations: Default::default(),
					prior: PriorLock(200, 1_000),
				}),
			);

			MigrateConvictionVotingTo2sBlocks::<Runtime>::on_runtime_upgrade();

			let voting = VotingFor::<Runtime>::get(who.clone(), 0u16);
			match voting {
				Voting::Casting(Casting { prior, .. }) => {
					assert_eq!(prior.0, 400);
					assert_eq!(prior.1, 1_000);
				}
				Voting::Delegating(_) => panic!("expected casting voting state"),
			}

			MigrateConvictionVotingTo2sBlocks::<Runtime>::on_runtime_upgrade();
			assert_eq!(prior_until(who), 400);
		})
	}
}
