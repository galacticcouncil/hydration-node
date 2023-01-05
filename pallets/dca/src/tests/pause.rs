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
use crate::{AssetId, Bond};
use crate::{Error, Event, Order, PoolType, Recurrence, Schedule, ScheduleId, Trade};
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::MultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn pause_should_remove_storage_entry_for_planned_execution_when_there_is_only_one_planned() {
	//TODO: add the same test when we execute the order with on_initialize, then we pause in later block
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

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
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			let schedule2 = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

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
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

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
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

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
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

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
fn pause_should_fail_when_paused_by_not_schedule_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

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
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
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

#[test]
fn pause_should_unreserve_execution_bond() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 10000 * ONE)])
		.with_fee_asset_for_all_users(DAI)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let storage_and_execution_bond = 1_950_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: storage_and_execution_bond
				}
			);

			assert_eq!(
				storage_and_execution_bond,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let execution_bond = 650_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: storage_and_execution_bond - execution_bond,
				}
			);

			assert_eq!(
				storage_and_execution_bond - execution_bond,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);
		});
}

//TODO: add test when there is multiple schedules, and we just then remove with pause, and not completely getting rid of the scheduleperblock

//TODO: Add test for pausing perpetual

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
