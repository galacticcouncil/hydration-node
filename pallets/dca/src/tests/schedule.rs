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
use crate::{Error, Event, Order, PoolType, Recurrence, Schedule, ScheduleId, Trade};
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn schedule_should_store_schedule_for_next_block_when_no_blocknumber_specified() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = schedule_fake(Recurrence::Fixed(5));

		//Act
		set_block_number(500);
		assert_ok!(Dca::schedule(Origin::signed(ALICE), schedule, Option::None));

		//Assert
		let schedule_id = 1;
		let stored_schedule = Dca::schedules(schedule_id).unwrap();
		assert_eq!(stored_schedule, schedule_fake(Recurrence::Fixed(5)));

		//Check if schedule ids are stored
		let schedule_ids = Dca::schedule_ids_per_block(501);
		assert!(Dca::schedule_ids_per_block(501).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
		assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

		//Check if schedule ownership is created
		assert!(Dca::schedule_ownership(schedule_id).is_some());
		assert_eq!(Dca::schedule_ownership(schedule_id).unwrap(), ALICE);

		//Check if the recurrances have been stored
		assert_eq!(Dca::remaining_recurrences(schedule_id).unwrap(), 5);
	});
}

#[test]
fn schedule_should_work_when_multiple_schedules_stored() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = schedule_fake(Recurrence::Fixed(5));

		//Act
		set_block_number(500);

		assert_ok!(Dca::schedule(Origin::signed(ALICE), schedule.clone(), Option::None));
		assert_ok!(Dca::schedule(Origin::signed(ALICE), schedule, Option::None));

		//Assert
		assert!(Dca::schedules(1).is_some());
		assert!(Dca::schedules(2).is_some());

		let scheduled_ids_for_next_block = Dca::schedule_ids_per_block(501).unwrap();

		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1, 2]);
		assert_eq!(scheduled_ids_for_next_block, expected_scheduled_ids_for_next_block);
	});
}

#[test]
fn schedule_should_work_when_block_is_specified_by_user() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = schedule_fake(Recurrence::Fixed(5));

		//Act
		set_block_number(500);
		assert_ok!(Dca::schedule(
			Origin::signed(ALICE),
			schedule.clone(),
			Option::Some(600)
		));

		//Assert
		let schedule_id = 1;
		let stored_schedule = Dca::schedules(schedule_id).unwrap();
		assert_eq!(stored_schedule, schedule);

		//Check if schedule ids are stored
		let schedule_ids = Dca::schedule_ids_per_block(600);
		assert!(Dca::schedule_ids_per_block(600).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
		assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

		//Check if schedule ownership is created
		assert!(Dca::schedule_ownership(schedule_id).is_some());
		assert_eq!(Dca::schedule_ownership(schedule_id).unwrap(), ALICE);
	});
}

#[test]
fn schedule_should_work_when_perpetual_schedule_is_specified() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = schedule_fake(Recurrence::Perpetual);

		//Act
		set_block_number(500);
		assert_ok!(Dca::schedule(Origin::signed(ALICE), schedule, Option::None));

		//Assert
		let schedule_id = 1;
		let stored_schedule = Dca::schedules(schedule_id).unwrap();
		assert_eq!(stored_schedule, schedule_fake(Recurrence::Perpetual));

		//Check if schedule ids are stored
		let schedule_ids = Dca::schedule_ids_per_block(501);
		assert!(Dca::schedule_ids_per_block(501).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
		assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

		//Check if schedule ownership is created
		assert!(Dca::schedule_ownership(schedule_id).is_some());
		assert_eq!(Dca::schedule_ownership(schedule_id).unwrap(), ALICE);

		//Check if the recurrances have been stored
		assert!(Dca::remaining_recurrences(schedule_id).is_none());
	});
}

#[test]
fn schedule_should_fail_when_not_called_by_user() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = schedule_fake(Recurrence::Fixed(5));

		//Act and assert
		assert_noop!(Dca::schedule(Origin::none(), schedule, Option::None), BadOrigin);
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

fn schedule_fake(recurrence: Recurrence) -> Schedule {
	let trades = create_bounded_vec(vec![Trade {
		asset_in: 3,
		asset_out: 4,
		pool: PoolType::XYK,
	}]);

	let schedule = Schedule {
		period: 10,
		order: Order {
			asset_in: 3,
			asset_out: 4,
			amount_in: 1000,
			amount_out: 2000,
			limit: 0,
			route: trades,
		},
		recurrence: recurrence,
	};
	schedule
}
