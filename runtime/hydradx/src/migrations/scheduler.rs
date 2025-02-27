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
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight, BoundedVec};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_scheduler::{pallet, ScheduledOf};
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::Saturating;


// This migration migrates the Scheduler to 6s block times by multiplying by 2 the spread between
// the scheduled block and the current block.
//
// The migration does not use a StorageVersion, make sure it is removed from the Runtime Executive
// after it has been run.
pub struct MigrateTo6sBlocks<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MigrateTo6sBlocks<T> {
    fn on_runtime_upgrade() -> Weight {
        let current_block = frame_system::Pallet::<T>::current_block_number();
        let agenda: Vec<(
            BlockNumberFor<T>,
            BoundedVec<Option<ScheduledOf<T>>, T::MaxScheduledPerBlock>,
        )> = pallet_scheduler::Agenda::<T>::iter().collect();
        let mut agenda_len = 0;

        for (old_block, schedules) in agenda {
            let old_spread = old_block.saturating_sub(current_block);
            let new_spread = old_spread.saturating_mul(2u32.into());
            let new_block = current_block.saturating_add(new_spread);

            pallet_scheduler::Agenda::<T>::remove(old_block);
            pallet_scheduler::Agenda::<T>::insert(new_block, schedules);

            agenda_len.saturating_inc();

            // At the time before the migration there are ~60 items in the Agenda.
            // Setting a safe limit which can be executed in 1 block
            if agenda_len == 150 {
                log::info!("Hit limit of 150 Agenda items, exiting loop");
                break;
            }
        }

        log::info!("MigrateSchedulerTo6sBlocks processed agenda items: {:?}", agenda_len);
        T::DbWeight::get().reads_writes(agenda_len, agenda_len.saturating_mul(2))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use frame_support::assert_ok;

    #[test]
    fn migrate_to_6s_blocks_works() {
        let mut ext = sp_io::TestExternalities::new_empty();

        ext.execute_with(|| {
            System::set_block_number(0);

            // Arrange
            let call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
                remark: vec![1],
            }));

            assert_ok!(Scheduler::schedule(RuntimeOrigin::root(), 200, None, 3, call));
            assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(200));

            // Act
            System::set_block_number(100);
            MigrateTo6sBlocks::<Runtime>::on_runtime_upgrade();

            // Assert
            assert!(!pallet_scheduler::Agenda::<Runtime>::contains_key(200));
            assert!(pallet_scheduler::Agenda::<Runtime>::contains_key(300));
        })
    }
}
