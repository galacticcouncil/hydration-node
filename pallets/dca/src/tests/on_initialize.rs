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
use std::io::empty;
use std::ops::RangeInclusive;

use sp_runtime::traits::CheckedMul;
use sp_runtime::FixedPointNumber;

use crate::tests::*;
use crate::{assert_balance, Event, Order, Schedule, ScheduleId, Trade};
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use hydradx_traits::pools::SpotPriceProvider;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::{BoundedVec, FixedU128};

#[test]
fn one_sell_dca_execution_should_unreserve_amount_in() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, initial_alice_hdx_balance),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 5 * ONE;
			let amount_to_sell = 1 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_to_blocknumber(501);

			//Assert
			let remaining_named_reserve = total_amount - amount_to_sell;
			assert_balance!(
				ALICE,
				HDX,
				initial_alice_hdx_balance - amount_to_sell - remaining_named_reserve
			);
			assert_balance!(ALICE, BTC, 1533639373243);
			assert_eq!(
				remaining_named_reserve,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn one_buy_dca_execution_should_unreserve_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 5 * ONE;
			let amount_to_buy = 1 * ONE;
			let max_limit = 1 * ONE * 4 / 5;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: max_limit,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, BTC, amount_to_buy);
			assert_balance!(ALICE, HDX, 9995149632542385);
			let fee = 2269868000;
			assert_eq!(
				total_amount - max_limit - fee,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn sell_dca_should_be_completed_when_not_enough_reserved_amount_present() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 5 * ONE;
			let amount_to_sell = 6 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
			assert_balance!(ALICE, BTC, 0);
			let schedule_id = 1;
			assert_that_dca_is_completed(schedule_id);
			assert!(
				DCA::schedule_ids_per_block(601).is_none(),
				"There should be no schedule for the block, but there is"
			);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_some_successfull_dca_execution_happened_but_no_more_reserved_amount_left() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 16 * ONE;
			let amount_to_sell = 5 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
			assert_balance!(ALICE, BTC, 22769699685998);
			let schedule_id = 1;
			let schedule_id = 1;
			assert_that_dca_is_completed(schedule_id);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_with_exact_total_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			env_logger::init();
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 15 * ONE;
			let amount_to_sell = 5 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
			assert_balance!(ALICE, BTC, 22769699685998);
			let schedule_id = 1;
			let schedule_id = 1;
			assert_that_dca_is_completed(schedule_id);
		});
}

#[test]
fn full_buy_dca_should_be_completed_when_not_enough_resered_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = ONE / 1000;
			let amount_to_buy = 1 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, BTC, 0);
			assert_balance!(ALICE, HDX, 10000 * ONE);
			assert_eq!(0, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
			let schedule_id = 1;
			assert_that_dca_is_completed(schedule_id);
		});
}

#[test]
fn full_buy_dca_should_be_completed_when_some_exeuction_is_successfull_but_not_enough_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 5 * ONE;
			let amount_to_buy = 1 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			proceed_to_blocknumber(501, 2001);

			//Assert
			assert_balance!(ALICE, BTC, 7 * ONE);
			assert_balance!(ALICE, HDX, 9995416044220179);
			assert_eq!(0, Currencies::reserved_balance(HDX.into(), &ALICE.into()));
			let schedule_id = 1;
			assert_that_dca_is_completed(schedule_id);
		});
}

#[test]
fn one_buy_dca_execution_should_unreserve_max_limit_with_slippage_when_bigger_than_specified_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let total_amount = 5 * ONE;
			let amount_to_buy = 1 * ONE;
			//let spent_amount = 4852637325615;
			let max_limit = 1 * ONE * 4 / 5;
			let max_limit_calculated_from_spot_price = 682500000000;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX.into(), &ALICE.into()));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, BTC, amount_to_buy);
			assert_balance!(ALICE, HDX, 9995032132542385);
			let fee = 2269868000;

			assert_eq!(
				total_amount - max_limit_calculated_from_spot_price - fee,
				Currencies::reserved_balance(HDX.into(), &ALICE.into())
			);
		});
}

#[test]
fn nothing_should_happen_when_no_schedule_in_storage_for_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Act
			proceed_to_blocknumber(1, 500);

			//Assert
			assert_balance!(ALICE, BTC, 0);
			let schedule_id = 1;
			assert!(DCA::schedules(schedule_id).is_none());
		});
}

#[test]
fn schedule_is_planned_for_next_block_when_user_one_execution_finished() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_limit: 1 * ONE,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, BTC, ONE);

			let schedule_id = 1;
			let scheduled_ids_for_next_planned_block = DCA::schedule_ids_per_block(601).unwrap();
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![schedule_id]);
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
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			let schedule_id = 1;
			let schedule = ScheduleBuilder::new().with_period(ONE_HUNDRED_BLOCKS).build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(601)));

			proceed_to_blocknumber(1, 500);
			let schedule_id_2 = 2;
			let schedule_2 = ScheduleBuilder::new().with_period(ONE_HUNDRED_BLOCKS).build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule_2, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let scheduled_ids_for_next_planned_block = DCA::schedule_ids_per_block(601).unwrap();
			let expected_scheduled_ids_for_next_block =
				create_bounded_vec_with_schedule_ids(vec![schedule_id, schedule_id_2]);
			assert_eq!(
				scheduled_ids_for_next_planned_block,
				expected_scheduled_ids_for_next_block
			);
		});
}

#[test]
fn dca_schedule_is_suspended_in_block_when_trade_fails_with_insufficient_trade_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 5000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_min_trade_amount(ONE)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: 0,
					max_limit: 5 * ONE,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, BTC, 0);
			let schedule_id = 1;
			assert!(DCA::suspended(schedule_id).is_some());

			expect_events(vec![Event::Suspended {
				id: schedule_id,
				who: ALICE,
			}
			.into()]);
		});
}

#[test]
fn dca_should_not_be_executed_when_schedule_is_paused_after_one_execution() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(LP2, BTC, 5000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_limit: 5 * ONE,
					route: empty_vec(),
				})
				.build();

			let schedule_id: ScheduleId = 1u32;
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_balance!(ALICE, BTC, 0);

			//Act
			set_to_blocknumber(501);

			assert_ok!(DCA::pause(Origin::signed(ALICE), schedule_id, 601));

			proceed_to_blocknumber(502, 901);

			//Assert
			assert_balance!(ALICE, BTC, 1 * ONE);
		});
}

#[test]
fn execution_fee_should_be_taken_from_user_in_sold_currency_in_case_of_successful_buy_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 5000 * ONE),
			(ALICE, DAI, 5000 * ONE),
			(LP2, BTC, 10000 * ONE),
			(LP2, DAI, 10000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: DAI,
					asset_out: BTC,
					amount_out: ONE,
					max_limit: 5 * ONE,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			assert_balance!(ALICE, BTC, 0);

			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), DAI, 4539736000);
			assert_balance!(ALICE, BTC, ONE);
		});
}

#[test]
fn execution_fee_should_be_still_taken_from_user_in_sold_currency_in_case_of_failed_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 5000 * ONE),
			(ALICE, DAI, 5000 * ONE),
			(LP2, BTC, 10000 * ONE),
			(LP2, DAI, 10000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_min_trade_amount(10 * ONE)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: DAI,
					asset_out: BTC,
					amount_out: 0,
					max_limit: 5 * ONE,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			assert_balance!(ALICE, BTC, 0);

			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), DAI, 4539736000);
		});
}

#[test]
fn execution_fee_should_be_taken_from_user_in_sold_currency_in_case_of_successful_sell_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 5000 * ONE),
			(ALICE, DAI, 5000 * ONE),
			(LP2, BTC, 10000 * ONE),
			(LP2, DAI, 10000 * ONE),
		])
		.with_registered_asset(BTC)
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: DAI,
					asset_out: BTC,
					amount_in: 100 * ONE,
					min_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			assert_balance!(ALICE, BTC, 0);

			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), DAI, 4539736000);
			assert_balance!(ALICE, BTC, 67564873224348);
		});
}

//TODO: add somet ests referring to the 5% calculation?!
#[test]
fn slippage_limit_should_be_used_for_sell_dca_when_it_is_smaller_than_specified_trade_min_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, 100, 2000 * ONE),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let sell_amount = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: 100,
					asset_out: 200,
					amount_in: sell_amount,
					min_limit: Balance::MAX,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, 200, 9897566790814);
		});
}

#[test]
fn slippage_limit_should_be_used_for_buy_dca_when_it_is_bigger_than_specified_trade_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, HDX, 10000 * ONE),
			(ALICE, 100, 2000 * ONE),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let buy_amount = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: 100,
					asset_out: 200,
					amount_out: buy_amount,
					max_limit: Balance::MIN,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(ALICE, 200, buy_amount);
		});
}

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn proceed_to_blocknumber(from: u64, to: u64) {
	for block_number in RangeInclusive::new(from, to) {
		System::set_block_number(block_number);
		DCA::on_initialize(block_number);
	}
}

pub fn set_to_blocknumber(to: u64) {
	System::set_block_number(to);
	DCA::on_initialize(to);
}

fn assert_that_dca_is_completed(schedule_id: ScheduleId) {
	assert!(DCA::schedules(schedule_id).is_none());
	assert!(DCA::suspended(schedule_id).is_none());
	assert!(DCA::owner_of(schedule_id).is_none());

	expect_events(vec![Event::Completed {
		id: schedule_id,
		who: ALICE,
	}
	.into()]);
}
