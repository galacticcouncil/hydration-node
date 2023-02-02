// This file is part of https://github.com/galacticcouncil/HydraDX-node

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
// use crate::tests::{assert_scheduled_ids, ScheduleBuilder};
// use crate::Bond;
use crate::{Error, Event, Order, OrderId};
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::{BoundedVec, FixedU128};
use std::ops::RangeInclusive;
pub type Price = FixedU128;
use orml_traits::MultiReservableCurrency;
use test_case::test_case;

#[test]
fn create_order_should_store_order_when_happy_path() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
      // let order = Order {
      //   asset_sell: 1,

      // }
			//Arrange
			// let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			//Act
			// set_block_number(500);
      let asset_id = 3_u32.into();
			assert_ok!(OTC::create_order(Origin::signed(ALICE), asset_id));

			//Assert
			// let schedule_id = 1;
			// let stored_schedule = DCA::schedules(schedule_id).unwrap();
			// assert_eq!(
			// 	stored_schedule,
			// 	ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build()
			// );

			// //Check if schedule ids are stored
			// let schedule_ids = DCA::schedule_ids_per_block(501);
			// assert!(DCA::schedule_ids_per_block(501).is_some());
			// let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
			// assert_eq!(schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);

			// //Check if schedule ownership is created
			// assert!(DCA::owner_of(schedule_id).is_some());
			// assert_eq!(DCA::owner_of(schedule_id).unwrap(), ALICE);

			// //Check if the recurrances have been stored
			// assert_eq!(DCA::remaining_recurrences(schedule_id).unwrap(), 5);
		});
}
