// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use frame_support::storage::IterableStorageDoubleMap;
use frame_support::traits::OnRuntimeUpgrade;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_democracy::{pallet, Voting};
use sp_runtime::{traits::BlockNumberProvider, Saturating};

pub struct MigrateDemocracyTo6sBlocks<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MigrateDemocracyTo6sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		let calculate_new_block =
			|current_block: BlockNumberFor<T>, unlock_block: BlockNumberFor<T>| -> BlockNumberFor<T> {
				let old_spread = unlock_block.saturating_sub(current_block);
				let new_spread = old_spread.saturating_mul(2u32.into());
				current_block.saturating_add(new_spread)
			};

		let current_block = frame_system::Pallet::<T>::block_number();
		let mut reads: u64 = 0;
		let mut writes: u64 = 0;

		pallet_democracy::VotingOf::<T>::iter().for_each(|(account, voting)| {
			reads = reads.saturating_add(1);

			let mut voting = voting;
			let mut write_to_storage = false;

			match &mut voting {
				Voting::Direct { ref mut prior, .. } => {
					let unlock_block = prior.0;

					if unlock_block > current_block {
						prior.0 = calculate_new_block(current_block, unlock_block);
						write_to_storage = true;
					};
				}
				Voting::Delegating { ref mut prior, .. } => {
					let unlock_block = prior.0;

					if unlock_block > current_block {
						prior.0 = calculate_new_block(current_block, unlock_block);
						write_to_storage = true;
					};
				}
			};

			if write_to_storage {
				pallet_democracy::VotingOf::<T>::insert(&account, voting);
				writes = writes.saturating_add(1);
			}
		});

		log::info!(
			"MigrateDemocracyTo6sBlocks complete!  Reads: {:?}, Writes: {:?}",
			reads,
			writes
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::Runtime;
	use pallet_democracy::{Delegations, PriorLock, VotingOf};
	use sp_core::crypto::AccountId32;
	use sp_core::H256;

	type VotingForStorage = VotingOf<Runtime>;

	fn mock_account_id() -> AccountId {
		AccountId32::new(H256::random().into())
	}

	#[test]
	fn migrate_democracy_to_6s_blocks_works() {
		let alice = mock_account_id();
		let bob = mock_account_id();

		let mut ext = sp_io::TestExternalities::new_empty();
		ext.execute_with(|| {
			System::set_block_number(0);

			// Arrange
			let voting_1 = Voting::Direct {
				votes: Default::default(),
				delegations: Delegations::default(),
				prior: PriorLock(50, 1_000_000),
			};

			let voting_2 = Voting::Direct {
				votes: Default::default(),
				delegations: Delegations::default(),
				prior: PriorLock(200, 1_000_000),
			};

			VotingForStorage::insert(&alice, voting_1);
			VotingForStorage::insert(&bob, voting_2);

			// Act
			System::set_block_number(100);
			MigrateDemocracyTo6sBlocks::<Runtime>::on_runtime_upgrade();

			// Assert
			// Alice voting with unlock block height in the past should not be updated
			let record = VotingForStorage::get(&alice);
			let unlock_block = match record {
				Voting::Direct { prior, .. } => prior.0,
				_ => panic!("Test case guarantees this is Voting::Direct"),
			};
			assert_eq!(unlock_block, 50);

			// Bob voting with unlock block height in the future should be updated
			let record = VotingForStorage::get(&bob);
			let unlock_block = match record {
				Voting::Direct { prior, .. } => prior.0,
				_ => panic!("Test case guarantees this is Voting::Direct"),
			};
			assert_eq!(unlock_block, 300);
		})
	}
}
