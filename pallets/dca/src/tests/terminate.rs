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
use crate::{assert_scheduled_ids, assert_that_schedule_has_been_removed_from_storages, reserve_identifier};
use crate::{Error, Event, Order, PoolType, Schedule, ScheduleId, Trade};
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::MultiReservableCurrency;
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::{BoundedVec, FixedU128};
use test_case::test_case;

#[test]
fn terminate_should_remove_schedule_from_storage() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(600)));

			//Assert
			assert_that_schedule_has_been_removed_from_storages!(schedule_id);

			expect_events(vec![Event::Terminated { id: 1, who: ALICE }.into()]);
		});
}

#[test]
fn terminate_should_unreserve_all_named_reserved() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new().with_total_amount(total_amount).build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));

			let schedule_id = 1;
			let named_reserve_id = reserve_identifier(schedule_id);
			assert_eq!(
				total_amount,
				Currencies::reserved_balance_named(&named_reserve_id, HDX.into(), &ALICE.into())
			);

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(600)));

			//Assert
			assert_eq!(
				0,
				Currencies::reserved_balance_named(&named_reserve_id, HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn terminate_should_remove_planned_execution_when_there_is_only_single_execution_on_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(600)));

			//Assert
			assert!(DCA::schedule_ids_per_block(600).is_none());
		});
}

#[test]
fn terminate_should_remove_planned_execution_when_there_are_multiple_planned_executions_on_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule2 = ScheduleBuilder::new().build();
			let schedule_id = 1;
			let block = 600;

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(block)));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::Some(block)));

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(block)));

			//Assert
			assert_scheduled_ids!(block, vec![2]);
		});
}

#[test]
fn terminate_should_remove_suspended_schedule_when_no_block_specified_by_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 600));

			//Act
			assert_ok!(DCA::terminate(Origin::signed(ALICE), schedule_id, Option::None));

			//Assert
			assert!(DCA::schedule_ids_per_block(600).is_none());
			assert!(DCA::suspended(schedule_id).is_none());
		});
}

#[test]
fn terminate_should_throw_error_when_schedule_is_not_suspended_and_next_exec_block_not_specified_by_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(600)));

			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(ALICE), schedule_id, Option::None),
				Error::<Test>::ScheduleMustBeSuspended
			);
		});
}

#[test]
fn terminate_should_fail_when_called_by_non_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(BOB), schedule_id, Option::Some(500)),
				Error::<Test>::NotScheduleOwner
			);
		});
}

#[test]
fn terminate_should_fail_when_called_by_non_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(DCA::terminate(Origin::none(), schedule_id, Option::None), BadOrigin);
		});
}

#[test]
fn terminate_should_fail_when_no_planned_execution_in_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
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
fn terminate_should_fail_when_there_is_planned_execution_in_block_not_not_for_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			let schedule2 = ScheduleBuilder::new().build();
			set_block_number(500);
			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(1000)));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(Origin::signed(ALICE), schedule_id, Option::Some(501)),
				Error::<Test>::ScheduleExecutionNotPlannedOnBlock
			);
		});
}

#[test]
fn terminate_should_fail_when_with_nonexisting_schedule() {
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
