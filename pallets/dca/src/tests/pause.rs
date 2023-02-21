// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::tests::mock::*;
use crate::tests::*;
use crate::{Error, Event, Order, PoolType, Schedule, ScheduleId, Trade};
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::MultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::FixedU128;

#[test]
fn pause_should_remove_storage_entry_for_planned_execution_when_there_is_only_one_planned() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert!(DCA::schedule_ids_per_block(501).is_none());

			expect_events(vec![Event::Paused { id: 1, who: ALICE }.into()]);
		});
}

#[test]
fn pause_should_remove_planned_schedule_from_next_execution_when_there_are_multiple_entries_planned() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			let schedule2 = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::None));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let scheduled_ids_for_next_block = DCA::schedule_ids_per_block(501).unwrap();
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![2]);
			assert_eq!(scheduled_ids_for_next_block, expected_scheduled_ids_for_next_block);
		});
}

#[test]
fn pause_should_mark_schedule_suspended() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert!(DCA::suspended(1).is_some());
		});
}

#[test]
fn pause_should_fail_when_when_called_with_nonsigned_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 1;
			assert_noop!(DCA::pause(Origin::none(), schedule_id, 501), BadOrigin);
		});
}

#[test]
fn pause_should_fail_when_when_schedule_is_not_planned_for_next_execution_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 1;
			assert_noop!(
				DCA::pause(Origin::signed(ALICE), schedule_id, 502),
				Error::<Test>::ScheduleNotExist
			);
		});
}

#[test]
fn pause_should_fail_when_when_schedule_is_not_planned_for_next_execution_block_but_exec_block_contains_other_schedules(
) {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			let schedule2 = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(502)));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::Some(503)));

			//Act and assert
			let schedule_id2 = 2;
			assert_noop!(
				DCA::pause(Origin::signed(ALICE), schedule_id2, 502),
				Error::<Test>::ScheduleExecutionNotPlannedOnBlock
			);
		});
}

#[test]
fn pause_should_fail_when_paused_by_not_schedule_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let schedule_id = 1;
			assert_noop!(
				DCA::pause(Origin::signed(BOB), schedule_id, 501),
				Error::<Test>::NotScheduleOwner
			);
		});
}

#[test]
fn pause_should_fail_when_schedule_not_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Act and assert
			let non_existing_schedule_id = 9999;
			assert_noop!(
				DCA::pause(Origin::signed(BOB), non_existing_schedule_id, 501),
				Error::<Test>::ScheduleNotExist
			);
		});
}

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
