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
use crate::tests::ScheduleBuilder;
use crate::{AssetId, Bond};
use crate::{Error, Event, Order, PoolType, Recurrence, Schedule, ScheduleId, Trade};
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::{BoundedVec, FixedU128};
pub type Price = FixedU128;

#[test]
fn schedule_should_store_schedule_for_next_block_when_no_blocknumber_specified() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

		//Act
		set_block_number(500);
		assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

		//Assert
		let schedule_id = 1;
		let stored_schedule = DCA::schedules(schedule_id).unwrap();
		assert_eq!(
			stored_schedule,
			ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build()
		);

		//Check if schedule ids are stored
		let schedule_ids = DCA::schedule_ids_per_block(501);
		assert!(DCA::schedule_ids_per_block(501).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
		assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

		//Check if schedule ownership is created
		assert!(DCA::schedule_ownership(schedule_id).is_some());
		assert_eq!(DCA::schedule_ownership(schedule_id).unwrap(), ALICE);

		//Check if the recurrances have been stored
		assert_eq!(DCA::remaining_recurrences(schedule_id).unwrap(), 5);
	});
}

#[test]
fn schedule_should_work_when_multiple_schedules_stored() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

		//Act
		set_block_number(500);

		assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule.clone(), Option::None));
		assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

		//Assert
		assert!(DCA::schedules(1).is_some());
		assert!(DCA::schedules(2).is_some());

		let scheduled_ids_for_next_block = DCA::schedule_ids_per_block(501).unwrap();

		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1, 2]);
		assert_eq!(scheduled_ids_for_next_block, expected_scheduled_ids_for_next_block);
	});
}

#[test]
fn schedule_should_work_when_block_is_specified_by_user() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

		//Act
		set_block_number(500);
		assert_ok!(DCA::schedule(
			Origin::signed(ALICE),
			schedule.clone(),
			Option::Some(600)
		));

		//Assert
		let schedule_id = 1;
		let stored_schedule = DCA::schedules(schedule_id).unwrap();
		assert_eq!(stored_schedule, schedule);

		//Check if schedule ids are stored
		let schedule_ids = DCA::schedule_ids_per_block(600);
		assert!(DCA::schedule_ids_per_block(600).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
		assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

		//Check if schedule ownership is created
		assert!(DCA::schedule_ownership(schedule_id).is_some());
		assert_eq!(DCA::schedule_ownership(schedule_id).unwrap(), ALICE);
	});
}

#[test]
fn schedule_should_work_when_perpetual_schedule_is_specified() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Perpetual).build();

		//Act
		set_block_number(500);
		assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

		//Assert
		let schedule_id = 1;
		let stored_schedule = DCA::schedules(schedule_id).unwrap();
		assert_eq!(
			stored_schedule,
			ScheduleBuilder::new().with_recurrence(Recurrence::Perpetual).build()
		);

		//Check if schedule ids are stored
		let schedule_ids = DCA::schedule_ids_per_block(501);
		assert!(DCA::schedule_ids_per_block(501).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
		assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

		//Check if schedule ownership is created
		assert!(DCA::schedule_ownership(schedule_id).is_some());
		assert_eq!(DCA::schedule_ownership(schedule_id).unwrap(), ALICE);

		//Check if the recurrances have been stored
		assert!(DCA::remaining_recurrences(schedule_id).is_none());
	});
}

#[test]
fn schedule_creation_should_store_bond_taken_from_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, DAI, 5000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(MultiTransactionPayment::add_currency(
				Origin::root(),
				DAI,
				Price::from_float(1.1)
			));
			assert_ok!(MultiTransactionPayment::set_currency(Origin::signed(ALICE.into()), DAI));

			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Assert
			let schedule_id = 1;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: 1,
					amount: 3_000_000
				}
			)
		});
}

#[test]
fn schedule_should_fail_when_not_called_by_user() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

		//Act and assert
		assert_noop!(DCA::schedule(Origin::none(), schedule, Option::None), BadOrigin);
	});
}

//TODO: add negative case for validating block numbers

fn create_bounded_vec(trades: Vec<Trade>) -> BoundedVec<Trade, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
