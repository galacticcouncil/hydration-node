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
use test_case::test_case;

#[test]
fn resume_should_remove_schedule_from_storage() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(600)));

			//Assert
			assert!(DCA::schedules(schedule_id).is_none());
			assert!(DCA::schedule_ownership(schedule_id).is_none());
			assert!(DCA::remaining_recurrences(schedule_id).is_none());

			expect_events(vec![Event::Terminated { id: 1, who: ALICE }.into()]);
		});
}

#[test]
fn resume_should_discard_complete_bond() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(600)));

			//Assert
			assert!(DCA::bond(schedule_id).is_none());
			assert_eq!(0, Tokens::reserved_balance(HDX.into(), &ALICE.into()));
		});
}

#[test]
fn resume_should_fail_when_called_by_non_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(BOB), schedule_id, Option::None),
				Error::<Test>::NotScheduleOwner
			);
		});
}

#[test]
fn resume_should_fail_when_called_by_non_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(DCA::terminate(Origin::none(), schedule_id, Option::None), BadOrigin);
		});
}

#[test]
fn resume_should_fail_when_no_planned_execution_in_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(9999)),
				Error::<Test>::NoPlannedExecutionFoundOnBlock
			);
		});
}

#[test]
fn resume_should_fail_when_there_is_planned_execution_in_block_not_not_for_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			let schedule2 = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(1000)));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(501)),
				Error::<Test>::NoPlannedExecutionFoundOnBlock
			);
		});
}

#[test]
fn resume_should_fail_when_with_nonexisting_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(ALICE), 999, Option::Some(501)),
				Error::<Test>::ScheduleNotExist
			);
		});
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
