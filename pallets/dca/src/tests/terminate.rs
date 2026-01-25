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
use frame_support::traits::Hooks;
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
			assert_ok!(DCA::terminate(RuntimeOrigin::signed(ALICE), schedule_id, Some(502)));
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
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, Some(502)));
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

#[test]
fn terminate_should_work_when_no_block_specified() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().with_period(300).build();
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));
			set_block_number(600);

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, None));

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
fn terminate_should_work_when_no_block_specified_and_schedule_eeceuted_multiple_times() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().with_period(300).build();
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));
			set_block_number(600);
			set_block_number(900);

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, None));

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
fn terminate_should_work_with_no_blocknumber_when_just_scheduled() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().with_period(300).build();
			let schedule_id = 0;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_ok!(DCA::terminate(RuntimeOrigin::root(), schedule_id, None));

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
fn terminate_should_not_depend_on_schedule_ids_per_block_ordering() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE), (BOB, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			let (block_600, schedule_id_0, schedule_id_1) = arrange_unsorted_schedule_ids_per_block_600();

			let reserved_before = Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE);
			assert!(reserved_before > 0);

			assert_ok!(DCA::terminate(RuntimeOrigin::signed(ALICE), schedule_id_1, None));

			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id_1);
			assert_eq!(DCA::schedule_ids_per_block(block_600).to_vec(), vec![schedule_id_0]);
			assert_eq!(
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE),
				0
			);
		});
}

fn arrange_unsorted_schedule_ids_per_block_600() -> (u64, ScheduleId, ScheduleId) {
	// Builds `ScheduleIdsPerBlock[600] == [1, 0]` via normal flow.
	let block_502 = 502u64;
	let block_600 = 600u64;
	let schedule_id_0: ScheduleId = 0;
	let schedule_id_1: ScheduleId = 1;

	set_block_number(500);

	let schedule_replans_into_block_600 = ScheduleBuilder::new()
		.with_owner(BOB)
		.with_period(98)
		.with_order(Order::Sell {
			asset_in: HDX,
			asset_out: BTC,
			amount_in: ONE,
			min_amount_out: 0,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: BTC,
			}]),
		})
		.build();
	assert_ok!(DCA::schedule(
		RuntimeOrigin::signed(BOB),
		schedule_replans_into_block_600,
		None
	));
	assert!(DCA::schedules(schedule_id_0).is_some());

	let schedule_planned_for_block_600 = ScheduleBuilder::new()
		.with_total_amount(0)
		.with_order(Order::Sell {
			asset_in: HDX,
			asset_out: BTC,
			amount_in: ONE,
			min_amount_out: 0,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: BTC,
			}]),
		})
		.build();
	assert_ok!(DCA::schedule(
		RuntimeOrigin::signed(ALICE),
		schedule_planned_for_block_600,
		Some(block_600)
	));
	assert!(DCA::schedules(schedule_id_1).is_some());

	set_block_number(block_502);

	assert!(DCA::schedules(schedule_id_1).is_some());
	assert_eq!(DCA::schedule_execution_block(schedule_id_1), Some(block_600));
	assert_eq!(
		DCA::schedule_ids_per_block(block_600).to_vec(),
		vec![schedule_id_1, schedule_id_0]
	);
	assert!(DCA::schedule_ids_per_block(block_600)
		.iter()
		.any(|id| *id == schedule_id_1));

	(block_600, schedule_id_0, schedule_id_1)
}

pub fn set_block_number(to: u64) {
	System::set_block_number(to);
	DCA::on_initialize(to);
}
