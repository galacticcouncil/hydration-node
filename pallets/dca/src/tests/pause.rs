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
use crate::Bond;
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
use sp_runtime::FixedU128;

#[test]
fn pause_should_remove_storage_entry_for_planned_execution_when_there_is_only_one_planned() {
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
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert!(DCA::schedule_ids_per_block(501).is_none());

			expect_events(vec![Event::Paused { id: 1, who: ALICE }.into()]);
		});
}

#[test]
fn pause_should_remove_planned_schedule_from_next_execution_when_there_are_multiple_entries_planned() {
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

			let schedule2 = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::None));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let scheduled_ids_for_next_block = DCA::schedule_ids_per_block(501).unwrap();
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![2]);
			assert_eq!(scheduled_ids_for_next_block, expected_scheduled_ids_for_next_block);
		});
}

#[test]
fn pause_should_mark_schedule_suspended() {
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
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert!(DCA::suspended(1).is_some());
		});
}

#[test]
fn pause_should_mark_schedule_suspended_for_perpetual_order() {
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
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Perpetual).build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert!(DCA::suspended(1).is_some());
		});
}

#[test]
fn pause_should_fail_when_when_called_with_nonsigned_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 1;
			assert_noop!(DCA::pause(Origin::none(), schedule_id, 501), BadOrigin);
		});
}

#[test]
fn pause_should_fail_when_when_schedule_is_not_planned_for_next_execution_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 1;
			assert_noop!(
				DCA::pause(Origin::signed(ALICE), schedule_id, 502),
				Error::<Test>::ScheduleNotExist
			);
		});
}

#[test]
fn pause_should_fail_when_when_schedule_is_not_planned_for_next_execution_block_but_exec_block_contains_other_schedules(
) {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();
			let schedule2 = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(502)));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule2, Option::Some(503)));

			//Act and assert
			let schedule_id2 = 2;
			assert_noop!(
				DCA::pause(Origin::signed(ALICE), schedule_id2, 502),
				Error::<Test>::ScheduleExecutionNotPlannedOnBlock
			);
		});
}

#[test]
fn pause_should_fail_when_paused_by_not_schedule_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			let schedule_id = 1;
			assert_noop!(
				DCA::pause(Origin::signed(BOB), schedule_id, 501),
				Error::<Test>::NotScheduleOwner
			);
		});
}

#[test]
fn pause_should_fail_when_schedule_not_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Act and assert
			let non_existing_schedule_id = 9999;
			assert_noop!(
				DCA::pause(Origin::signed(BOB), non_existing_schedule_id, 501),
				Error::<Test>::ScheduleNotExist
			);
		});
}

#[test]
fn pause_should_unreserve_execution_bond_when_native_token_set_as_user_currency() {
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

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let storage_and_execution_bond = 3_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: storage_and_execution_bond
				}
			);

			assert_eq!(
				storage_and_execution_bond,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let execution_bond = 1_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: storage_and_execution_bond - execution_bond,
				}
			);

			assert_eq!(
				storage_and_execution_bond - execution_bond,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn pause_should_not_unreserve_execution_bond_with_native_token_when_storage_bond_config_greatly_increased_by_admins() {
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

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let total_bond = 3_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: total_bond
				}
			);

			assert_eq!(total_bond, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_storage_bond_config(*OriginalStorageBondInNative * 10);
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let execution_bond = 1_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: total_bond,
				}
			);

			assert_eq!(total_bond, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
		});
}

#[test]
fn pause_should_unreserve_a_part_of_execution_bond_with_native_token_when_storage_bond_config_slightly_increased_by_admins(
) {
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

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let total_bond = 3_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: total_bond
				}
			);

			assert_eq!(total_bond, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_storage_bond_config(*OriginalStorageBondInNative * 11 / 10);
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let not_full_execution_bond = 800_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: total_bond - not_full_execution_bond,
				}
			);

			assert_eq!(
				total_bond - not_full_execution_bond,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn pause_should_unreserve_execution_bond_when_nonnative_token_set_as_user_currency() {
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
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let storage_and_execution_bond = 6_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: storage_and_execution_bond
				}
			);

			assert_eq!(
				storage_and_execution_bond,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);

			//Act
			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let execution_bond = 2_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: storage_and_execution_bond - execution_bond,
				}
			);

			assert_eq!(
				storage_and_execution_bond - execution_bond,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);
		});
}

#[test]
fn pause_should_unreserve_with_original_bond_asset_when_user_changes_set_currency_after_scheduling() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let storage_and_execution_bond = 3_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: storage_and_execution_bond
				}
			);

			assert_eq!(
				storage_and_execution_bond,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);

			//Act
			MultiTransactionPayment::add_currency(Origin::root(), DAI, FixedU128::from_float(0.80));
			MultiTransactionPayment::set_currency(Origin::signed(ALICE), DAI);

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let execution_bond = 1_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: HDX,
					amount: storage_and_execution_bond - execution_bond,
				}
			);

			assert_eq!(
				storage_and_execution_bond - execution_bond,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn pause_should_make_sure_to_keep_storage_bond_when_stored_total_bond_is_less_than_currenct_storage_bond() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, HDX, 500000 * ONE),
			(LP2, DAI, 500000 * ONE),
		])
		.with_fee_asset(vec![(ALICE, DAI)])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let total_bond_before = 6_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond_before
				}
			);

			assert_eq!(
				total_bond_before,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);

			//Act
			//We change the price of the currency, so making storage bond big
			assert_ok!(Tokens::transfer(
				Origin::signed(LP2),
				Omnipool::protocol_account(),
				DAI,
				4000 * ONE
			));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond_before,
				}
			);

			assert_eq!(
				total_bond_before,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);
		});
}

#[test]
fn pause_should_make_sure_to_keep_storage_bond_when_execution_bond_has_been_much_increased_by_admin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, HDX, 500000 * ONE),
			(LP2, DAI, 500000 * ONE),
		])
		.with_fee_asset(vec![(ALICE, DAI)])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let total_bond_before = 6_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond_before
				}
			);

			assert_eq!(
				total_bond_before,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);

			//Act
			set_storage_bond_config(*OriginalStorageBondInNative * 10);

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond_before,
				}
			);

			assert_eq!(
				total_bond_before,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);
		});
}

#[test]
fn pause_should_unreserve_less_to_keep_original_storage_bond_when_when_price_changes_slightly() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, DAI, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
			(LP2, HDX, 500000 * ONE),
			(LP2, DAI, 500000 * ONE),
		])
		.with_fee_asset(vec![(ALICE, DAI)])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_recurrence(Recurrence::Fixed(5)).build();

			set_block_number(500);

			let schedule_id = 1;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			let total_bond_before = 6_000_000;
			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond_before
				}
			);

			assert_eq!(
				total_bond_before,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);

			//Act
			//We change the price of the currency
			assert_ok!(Tokens::transfer(
				Origin::signed(LP2),
				Omnipool::protocol_account(),
				DAI,
				200 * ONE
			));

			let schedule_id = 1;
			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 501));

			//Assert
			let not_full_execution_bond = 1200000;

			assert_eq!(
				DCA::bond(schedule_id).unwrap(),
				Bond {
					asset: DAI,
					amount: total_bond_before - not_full_execution_bond,
				}
			);

			assert_eq!(
				total_bond_before - not_full_execution_bond,
				Currencies::reserved_balance(DAI.into(), &ALICE.into())
			);
		});
}

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
