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
use crate::{assert_scheduled_ids, assert_that_schedule_has_been_removed_from_storages};
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn terminate_should_remove_schedule_from_storage() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(600)));

			//Assert
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);

			expect_events(vec![Event::Terminated {
				id: 0,
				who: ALICE,
				error: Error::<Test>::ManuallyTerminated.into(),
			}
			.into()]);
		});
}

#[test]
fn terminate_should_terminate_schedule_planned_in_next_block_when_no_block_specified() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Option::None));

			//Assert
			expect_events(vec![Event::Terminated {
				id: 0,
				who: ALICE,
				error: Error::<Test>::ManuallyTerminated.into(),
			}
			.into()]);
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

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));

			let schedule_id = 0;
			assert_eq!(
				total_amount,
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE)
			);

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(600)));

			//Assert
			assert_eq!(
				0,
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE)
			);
		});
}

#[test]
fn terminate_should_unreserve_all_named_reserved_only_for_single_dca_when_there_are_multiple() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new().with_total_amount(total_amount).build();
			let schedule2 = ScheduleBuilder::new().with_total_amount(total_amount).build();
			let total_reserved = total_amount * 2;

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule2,
				Option::Some(700)
			));

			let schedule_id = 0;
			assert_eq!(
				total_reserved,
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE)
			);

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(600)));

			//Assert
			assert_eq!(
				total_reserved / 2,
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE)
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
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(600)));

			//Assert
			assert!(DCA::schedule_ids_per_block(600).is_empty());
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
			let schedule_id = 0;
			let block = 600;

			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule,
				Option::Some(block)
			));
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule2,
				Option::Some(block)
			));

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(block)));

			//Assert
			assert_scheduled_ids!(block, vec![1]);
		});
}

#[test]
fn terminate_should_pass_when_called_by_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			set_block_number(500);
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_ok!(DCA::terminate(RuntimeOrigin::signed(ALICE), schedule_id, Some(501)));
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
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(RuntimeOrigin::signed(BOB), schedule_id, Some(501)),
				Error::<Test>::Forbidden
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
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(DCA::terminate(RuntimeOrigin::none(), schedule_id, Some(501)), BadOrigin);
		});
}

#[test]
fn terminate_should_pass_when_called_by_technical_origin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			set_block_number(500);
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(501)));
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
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(9999)),
				Error::<Test>::ScheduleNotFound
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
			let schedule_id = 0;
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule,
				Option::Some(1000)
			));
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule2, Option::None));

			//Act and assert
			assert_noop!(
				DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(501)),
				Error::<Test>::ScheduleNotFound
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
				DCA::terminate(RuntimeOrigin::root(), 999, Some(501)),
				Error::<Test>::ScheduleNotFound
			);
		});
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
