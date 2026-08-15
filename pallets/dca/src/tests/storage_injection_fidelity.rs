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

//! Guards `insert_schedule_into_storage` against drift from the `schedule` extrinsic: an order the
//! extrinsic still accepts must land in identical state either way.

use crate::assert_scheduled_ids;
use crate::tests::schedule::set_block_number;
use crate::tests::*;
use crate::{Event as DcaEvent, ScheduleId};
use frame_support::assert_ok;
use orml_traits::MultiReservableCurrency;
use pretty_assertions::assert_eq;

#[test]
fn storage_injected_schedule_should_match_extrinsic_created_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().with_total_amount(1000 * ONE).build();

			//Act
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			let reserved_by_extrinsic = Currencies::reserved_balance(HDX, &ALICE);
			let injected_id = insert_schedule_into_storage(ALICE, schedule.clone(), Option::None);

			//Assert
			assert_schedules_are_identical(0, injected_id, &schedule, reserved_by_extrinsic);
		});
}

#[test]
fn storage_injected_rolling_schedule_should_match_extrinsic_created_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let rolling_budget = 0;
			let schedule = ScheduleBuilder::new().with_total_amount(rolling_budget).build();

			//Act
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			let reserved_by_extrinsic = Currencies::reserved_balance(HDX, &ALICE);
			let injected_id = insert_schedule_into_storage(ALICE, schedule.clone(), Option::None);

			//Assert
			assert_schedules_are_identical(0, injected_id, &schedule, reserved_by_extrinsic);
		});
}

#[test]
fn storage_injected_schedule_should_match_extrinsic_created_schedule_when_start_block_is_given() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().with_total_amount(1000 * ONE).build();
			let start_execution_block = 511;

			//Act
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::Some(start_execution_block)
			));
			let reserved_by_extrinsic = Currencies::reserved_balance(HDX, &ALICE);
			let injected_id =
				insert_schedule_into_storage(ALICE, schedule.clone(), Option::Some(start_execution_block));

			//Assert
			assert_eq!(DCA::schedule_execution_block(injected_id), Some(515));
			assert_schedules_are_identical(0, injected_id, &schedule, reserved_by_extrinsic);
		});
}

fn assert_schedules_are_identical(
	extrinsic_id: ScheduleId,
	injected_id: ScheduleId,
	schedule: &Schedule<AccountId, AssetId, BlockNumber>,
	reserved_by_extrinsic: Balance,
) {
	assert_eq!(injected_id, extrinsic_id + 1);
	assert_eq!(DCA::schedules(injected_id), DCA::schedules(extrinsic_id));
	assert_eq!(
		DCA::owner_of(schedule.owner, injected_id),
		DCA::owner_of(schedule.owner, extrinsic_id)
	);
	assert_eq!(
		DCA::remaining_amounts(injected_id),
		DCA::remaining_amounts(extrinsic_id)
	);
	assert_eq!(DCA::retries_on_error(injected_id), DCA::retries_on_error(extrinsic_id));
	assert_eq!(
		DCA::schedule_extra_gas(injected_id),
		DCA::schedule_extra_gas(extrinsic_id)
	);

	let execution_block = DCA::schedule_execution_block(extrinsic_id).unwrap();
	assert_eq!(DCA::schedule_execution_block(injected_id), Some(execution_block));
	assert_scheduled_ids!(execution_block, vec![extrinsic_id, injected_id]);

	assert_eq!(
		Currencies::reserved_balance(schedule.order.get_asset_in(), &schedule.owner),
		2 * reserved_by_extrinsic
	);

	expect_events(vec![
		DcaEvent::Scheduled {
			id: injected_id,
			who: schedule.owner,
			period: schedule.period,
			total_amount: schedule.total_amount,
			order: schedule.order.clone(),
		}
		.into(),
		DcaEvent::ExecutionPlanned {
			id: injected_id,
			who: schedule.owner,
			block: execution_block,
		}
		.into(),
	]);
}
