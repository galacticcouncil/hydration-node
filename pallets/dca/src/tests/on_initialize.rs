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

use frame_support::traits::OnInitialize;

use crate::tests::mock::*;
use crate::tests::*;
use crate::{assert_balance, AssetId, BlockNumber, Order, Recurrence, Schedule, ScheduleId, Trade};
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::{BoundedVec, FixedU128};
const ALICE: AccountId = 1000;
const BOB: AccountId = 1001;

#[test]
fn schedule_is_executed_in_block_when_user_has_fixed_schedule_planned() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, DAI, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_registered_asset(DAI)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = schedule_fake(
				ONE_HUNDRED_BLOCKS,
				AssetPair {
					asset_out: BTC,
					asset_in: DAI,
				},
				ONE,
				Recurrence::Fixed(5),
			);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let current_block = 501;
			DCA::on_initialize(current_block);

			//Assert
			assert_balance!(ALICE, BTC, ONE);
			let schedule_id = 1;
			assert_eq!(DCA::remaining_recurrences(schedule_id).unwrap(), 4);

			let scheduled_ids_for_next_planned_block = DCA::schedule_ids_per_block(601).unwrap();
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
			assert_eq!(
				scheduled_ids_for_next_planned_block,
				expected_scheduled_ids_for_next_block
			);
		});
}

#[test]
fn schedule_is_planned_with_period_when_block_has_already_planned_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, DAI, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_registered_asset(DAI)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange

			let schedule = schedule_fake(
				ONE_HUNDRED_BLOCKS,
				AssetPair {
					asset_out: BTC,
					asset_in: DAI,
				},
				ONE,
				Recurrence::Fixed(5),
			);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(601)));

			set_block_number(500);
			let schedule = schedule_fake(
				ONE_HUNDRED_BLOCKS,
				AssetPair {
					asset_out: BTC,
					asset_in: DAI,
				},
				ONE,
				Recurrence::Fixed(5),
			);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let current_block = 501;
			DCA::on_initialize(current_block);

			//Assert
			let scheduled_ids_for_next_planned_block = DCA::schedule_ids_per_block(601).unwrap();
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1, 2]);
			assert_eq!(
				scheduled_ids_for_next_planned_block,
				expected_scheduled_ids_for_next_block
			);
		});
}

#[test]
fn schedule_is_suspended_in_block_when_error_happens() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, BTC, 5000 * ONE),
			(LP2, DAI, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_registered_asset(DAI)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = schedule_fake(
				ONE_HUNDRED_BLOCKS,
				AssetPair {
					asset_out: BTC,
					asset_in: DAI,
				},
				ONE,
				Recurrence::Fixed(5),
			);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			DCA::on_initialize(501);

			//Assert
			assert_balance!(ALICE, BTC, 0);
			let schedule_id = 1;
			assert_eq!(DCA::remaining_recurrences(schedule_id).unwrap(), 5);
			assert!(DCA::suspended(schedule_id).is_some());
		});
}

#[test]
fn schedule_is_executed_in_block_when_user_has_perpetual_schedule_planned() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, DAI, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_registered_asset(DAI)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = schedule_fake(
				ONE_HUNDRED_BLOCKS,
				AssetPair {
					asset_out: BTC,
					asset_in: DAI,
				},
				ONE,
				Recurrence::Perpetual,
			);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let current_block = 501;
			DCA::on_initialize(current_block);

			//Assert
			assert_balance!(ALICE, BTC, ONE);
			let schedule_id = 1;
			let s = DCA::remaining_recurrences(schedule_id);
			assert!(DCA::remaining_recurrences(schedule_id).is_none());

			let scheduled_ids_for_next_planned_block = DCA::schedule_ids_per_block(601).unwrap();
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![1]);
			assert_eq!(
				scheduled_ids_for_next_planned_block,
				expected_scheduled_ids_for_next_block
			);
		});
}

#[test]
fn schedule_should_not_be_planned_again_when_there_is_no_more_recurrences() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, DAI, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_registered_asset(DAI)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = schedule_fake(
				ONE_HUNDRED_BLOCKS,
				AssetPair {
					asset_out: BTC,
					asset_in: DAI,
				},
				ONE,
				Recurrence::Fixed(1),
			);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let current_block = 501;
			DCA::on_initialize(current_block);

			//Assert
			assert_balance!(ALICE, BTC, ONE);
			let schedule_id = 1;
			assert!(DCA::remaining_recurrences(schedule_id).is_none());

			assert!(
				DCA::schedule_ids_per_block(601).is_none(),
				"There should be no schedule for the block, but there is"
			);
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
