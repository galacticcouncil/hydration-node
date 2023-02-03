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
use crate::types::{Bond, Order, PoolType, Recurrence, Schedule, ScheduleId, Trade};
use crate::Error::ScheduleMustBeSuspended;
use crate::{Error, Event};
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
use test_case::test_case;

#[test]
fn resume_should_fail_when_called_by_non_owner() {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Act and assert
			assert_noop!(
				DCA::resume(Origin::signed(BOB), schedule_id, Option::None),
				Error::<Test>::NotScheduleOwner
			);
		});
}

#[test]
fn resume_should_schedule_to_next_block_when_next_execution_block_is_not_defined() {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::None));

			//Assert
			let schedule_ids = DCA::schedule_ids_per_block(501);
			assert!(DCA::schedule_ids_per_block(501).is_some());
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
			assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

			expect_events(vec![
				Event::Resumed { id: 1, who: ALICE }.into(),
				Event::ExecutionPlanned {
					id: 1,
					who: ALICE,
					block: 501,
				}
				.into(),
			]);
		});
}

#[test]
fn resume_should_schedule_to_next_block_when_there_is_already_existing_schedule_in_next_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(BOB, HDX, 10000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			let schedule2 = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_ok!(DCA::schedule(Origin::signed(BOB), schedule2, Option::None));
			assert_scheduled_ids(501, vec![1, 2]);

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));
			assert_scheduled_ids(501, vec![2]);

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::None));

			//Assert
			assert_scheduled_ids(501, vec![2, 1]);
		});
}

#[test_case(1)]
#[test_case(499)]
#[test_case(500)]
fn resume_should_fail_when_specified_next_block_is_not_greater_than_current_block(block: BlockNumberFor<Test>) {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Act
			set_block_number(501);
			let schedule_id = 1;
			assert_noop!(
				DCA::resume(Origin::signed(ALICE), schedule_id, Option::Some(block)),
				Error::<Test>::BlockNumberIsNotInFuture
			);
		});
}

#[test]
fn resume_should_schedule_to_next_block_when_next_execution_block_is_defined() {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::Some(1000)));

			//Assert
			let schedule_ids = DCA::schedule_ids_per_block(1000);
			assert!(DCA::schedule_ids_per_block(1000).is_some());
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
			assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

			expect_events(vec![
				Event::Resumed { id: 1, who: ALICE }.into(),
				Event::ExecutionPlanned {
					id: 1,
					who: ALICE,
					block: 1000,
				}
				.into(),
			]);
		});
}

#[test]
fn resume_should_schedule_to_next_block_when_there_is_already_existing_schedule_in_next_block_and_next_block_is_specified(
) {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(BOB, HDX, 10000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			let schedule2 = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_ok!(DCA::schedule(Origin::signed(BOB), schedule2, Option::Some(1000)));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::Some(1000)));

			//Assert
			assert_scheduled_ids(1000, vec![2, 1]);
		});
}

#[test]
fn resume_should_schedule_remove_schedule_from_suspended() {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));
			assert!(DCA::suspended(schedule_id).is_some());

			//Act
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::None));

			//Assert
			assert!(DCA::suspended(schedule_id).is_none());
		});
}

#[test]
fn resume_should_reserve_execution_bond() {
	let total_bond = 3000000;
	let execution_bond = 1000000;
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: total_bond - execution_bond
				}
			);

			assert_eq!(
				total_bond - execution_bond,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);

			//Act
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::None));

			//Assert
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: total_bond
				}
			);

			assert_eq!(total_bond, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
		});
}

#[test]
fn resume_should_reserve_execution_bond_when_nonnative_currency_is_used() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_fee_asset(vec![(ALICE, DAI)])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 5000 * ONE)
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));
			let total_bond = 6_000_000;
			let execution_bond = 2_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond - execution_bond
				}
			);

			assert_eq!(
				total_bond - execution_bond,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);

			//Act
			assert_ok!(DCA::resume(Origin::signed(ALICE), schedule_id, Option::None));

			//Assert
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond
				}
			);

			assert_eq!(total_bond, Currencies::reserved_balance(DAI.into(), &ALICE.into()));
		});
}

#[test]
fn resume_should_fail_when_schedule_is_not_suspended() {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let schedule_id = 1;
			assert_noop!(
				DCA::resume(Origin::signed(ALICE), schedule_id, Option::None),
				Error::<Test>::ScheduleMustBeSuspended
			);
		});
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
