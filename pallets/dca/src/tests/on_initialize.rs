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

use crate::tests::*;
use crate::{
	assert_balance, assert_executed_buy_trades, assert_executed_sell_trades, assert_number_of_executed_buy_trades,
	assert_number_of_executed_sell_trades, assert_scheduled_ids, assert_that_schedule_has_been_removed_from_storages,
	Error, Event as DcaEvent, Order, Permill, ScheduleId,
};
use frame_support::assert_ok;
use frame_support::traits::OnInitialize;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_runtime::DispatchError;
use std::borrow::Borrow;
use std::ops::RangeInclusive;

#[test]
fn successfull_dca_execution_should_emit_trade_executed_event() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			expect_events(vec![
				DcaEvent::TradeExecuted {
					id: schedule_id,
					who: ALICE,
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 601,
				}
				.into(),
			]);
		});
}

#[test]
fn one_dca_execution_should_unreserve_amount_in() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, initial_alice_hdx_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			let remaining_named_reserve = total_amount - amount_to_sell;
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: amount_to_sell - FEE_FOR_ONE_DCA_EXECUTION,
				min_buy_amount: 792000000000,
			}]);

			assert_eq!(remaining_named_reserve, Currencies::reserved_balance(HDX, &ALICE));
		});
}

#[test]
fn one_buy_dca_execution_should_unreserve_user_specified_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;
			let max_limit = ONE / 2;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![BuyExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: ONE,
				max_sell_amount: max_limit,
			}]);

			assert_eq!(
				total_amount - max_limit - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);
		});
}

#[test]
fn one_buy_dca_execution_should_use_max_price_diff_as_slippage_limit_when_not_specified_by_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			let max_sell_amount = 968_000_000_000;
			assert_executed_buy_trades!(vec![BuyExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: ONE,
				max_sell_amount,
			}]);

			assert_eq!(
				total_amount - max_sell_amount - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);
		});
}

#[test]
fn one_buy_dca_execution_should_use_slippage_limit_when_specified_by_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: Some(Permill::from_percent(15)),
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			let max_sell_amount_calculated_by_slippage = 1012000000000;
			assert_executed_buy_trades!(vec![BuyExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: ONE,
				max_sell_amount: max_sell_amount_calculated_by_slippage,
			}]);

			assert_eq!(
				total_amount - max_sell_amount_calculated_by_slippage - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_some_successfull_dca_execution_happened_but_no_more_reserved_amount_left() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

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
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_sell_trades!(3);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_some_successfull_dca_execution_happened_but_less_than_fee_left() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let amount_to_sell = FEE_FOR_ONE_DCA_EXECUTION * 11 / 10;
			let total_amount = amount_to_sell + FEE_FOR_ONE_DCA_EXECUTION / 2;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_sell_trades!(1);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_sell_dca_should_be_completed_for_multiple_users() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE), (BOB, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 16 * ONE;
			let amount_to_sell = 5 * ONE;

			let schedule_for_alice = ScheduleBuilder::new()
				.with_owner(ALICE)
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			let schedule_for_bob = ScheduleBuilder::new()
				.with_owner(BOB)
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule_for_alice, Option::None));
			assert_ok!(DCA::schedule(Origin::signed(BOB), schedule_for_bob, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &BOB));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			assert_eq!(0, Currencies::reserved_balance(HDX, &BOB));

			assert_number_of_executed_sell_trades!(6);

			let schedule_id = 0;
			let schedule_id_2 = 1;
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			assert_that_schedule_has_been_removed_from_storages!(BOB, schedule_id_2);
		});
}

#[test]
fn multiple_sell_dca_should_be_completed_for_one_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE), (BOB, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 16 * ONE;
			let amount_to_sell = 5 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_owner(ALICE)
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule.clone(), Option::None));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount + total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));

			assert_number_of_executed_sell_trades!(6);

			let schedule_id = 0;
			let schedule_id_2 = 1;
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id_2);
		});
}

#[test]
fn full_sell_dca_should_be_completed_when_exact_total_amount_specified_for_the_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 15 * ONE + 3 * FEE_FOR_ONE_DCA_EXECUTION;
			let amount_to_sell = 5 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 801);

			//Assert
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			assert_number_of_executed_sell_trades!(3);

			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn full_buy_dca_should_be_completed_when_some_execution_is_successfull_but_not_enough_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			proceed_to_blocknumber(501, 2001);

			//Assert
			assert_number_of_executed_buy_trades!(5);
			assert_eq!(0, Currencies::reserved_balance(HDX, &ALICE));
			let schedule_id = 0;
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn one_buy_dca_execution_should_unreserve_max_limit_with_slippage_when_smaller_than_specified_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;
			let max_limit_calculated_from_spot_price = 968000000000;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit: Balance::MAX,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![BuyExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: ONE,
				max_sell_amount: max_limit_calculated_from_spot_price,
			}]);

			assert_eq!(
				total_amount - max_limit_calculated_from_spot_price - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);
		});
}

#[test]
fn nothing_should_happen_when_no_schedule_in_storage_for_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Act
			proceed_to_blocknumber(1, 500);

			//Assert
			let schedule_id = 1;
			assert!(DCA::schedules(schedule_id).is_none());
		});
}

#[test]
fn schedule_is_planned_for_next_block_when_user_one_execution_finished() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
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
					max_limit: ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_number_of_executed_buy_trades!(1);

			let schedule_id = 0;
			assert_scheduled_ids!(601, vec![schedule_id]);
		});
}

#[test]
fn schedule_is_planned_with_period_when_block_has_already_planned_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule_id = 0;
			let schedule = ScheduleBuilder::new().with_period(ONE_HUNDRED_BLOCKS).build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(601)));

			proceed_to_blocknumber(1, 500);
			let schedule_id_2 = 1;
			let schedule_2 = ScheduleBuilder::new().with_period(ONE_HUNDRED_BLOCKS).build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule_2, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_scheduled_ids!(601, vec![schedule_id, schedule_id_2]);
		});
}

#[test]
fn dca_schedule_should_continue_when_error_is_configured_to_continue_on() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: INVALID_BUY_AMOUNT_VALUE,
					max_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			set_to_blocknumber(501);

			assert_number_of_executed_buy_trades!(0);

			let schedule_id = 0;

			assert_scheduled_ids!(601, vec![schedule_id]);
			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries.unwrap());
			expect_events(vec![
				DcaEvent::TradeFailed {
					id: schedule_id,
					who: ALICE,
					error: pallet_omnipool::Error::<Test>::BuyLimitNotReached.into(),
				}
				.into(),
				DcaEvent::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 601,
				}
				.into(),
			]);
		});
}

#[test]
fn dca_trade_unallocation_should_be_rolled_back_when_trade_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 1000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: INVALID_BUY_AMOUNT_VALUE,
					max_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			let schedule_id = 0;

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), total_amount);
			assert_eq!(DCA::remaining_amounts(schedule_id).unwrap(), total_amount);

			set_to_blocknumber(501);

			assert_number_of_executed_buy_trades!(0);
			assert_scheduled_ids!(601, vec![schedule_id]);

			assert_eq!(
				Currencies::reserved_balance(HDX, &ALICE),
				total_amount - FEE_FOR_ONE_DCA_EXECUTION
			);
			assert_eq!(
				DCA::remaining_amounts(schedule_id).unwrap(),
				total_amount - FEE_FOR_ONE_DCA_EXECUTION
			);
		});
}

#[test]
fn dca_schedule_should_terminate_when_error_is_not_configured_to_continue_on() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, FORBIDDEN_ASSET, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: FORBIDDEN_ASSET,
					asset_out: BTC,
					amount_in: ONE,
					min_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;

			assert_number_of_executed_buy_trades!(0);
			assert!(DCA::schedule_ids_per_block(601).is_empty());
			assert_that_dca_is_terminated(ALICE, schedule_id, pallet_omnipool::Error::<Test>::NotAllowed.into());
		});
}

#[test]
fn dca_schedule_should_continue_on_multiple_failures_then_terminated() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: INVALID_BUY_AMOUNT_VALUE,
					max_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 0;
			set_to_blocknumber(501);
			assert_scheduled_ids!(601, vec![schedule_id]);

			set_to_blocknumber(601);
			assert_scheduled_ids!(701, vec![schedule_id]);

			set_to_blocknumber(701);
			assert_scheduled_ids!(701, vec![schedule_id]);

			set_to_blocknumber(801);
			assert!(DCA::schedules(schedule_id).is_none());
		});
}

#[test]
fn dca_schedule_retry_should_be_reset_when_successfull_trade_after_failed_ones() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: INVALID_BUY_AMOUNT_VALUE,
					max_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act and assert
			let schedule_id = 0;
			set_to_blocknumber(501);
			assert_scheduled_ids!(601, vec![schedule_id]);

			set_to_blocknumber(601);
			assert_scheduled_ids!(701, vec![schedule_id]);

			set_invalid_buy_amount(INVALID_BUY_AMOUNT_VALUE + ONE);

			set_to_blocknumber(701);
			assert_scheduled_ids!(701, vec![schedule_id]);
			assert_number_of_executed_buy_trades!(1);

			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(0, retries.unwrap());
		});
}

#[test]
fn execution_fee_should_be_taken_from_user_in_sold_currency_in_case_of_successful_buy_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE), (ALICE, DAI, 5000 * ONE)])
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
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), DAI, FEE_FOR_ONE_DCA_EXECUTION_IN_DAI);
			assert_number_of_executed_buy_trades!(1);
		});
}

#[test]
fn execution_fee_should_be_still_taken_from_user_in_sold_currency_in_case_of_failed_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE), (ALICE, DAI, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: DAI,
					asset_out: BTC,
					amount_out: INVALID_BUY_AMOUNT_VALUE,
					max_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);
			assert_balance!(ALICE, BTC, 0);

			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), DAI, FEE_FOR_ONE_DCA_EXECUTION_IN_DAI);
		});
}

#[test]
fn execution_fee_should_be_taken_from_user_in_sold_currency_in_case_of_successful_sell_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE), (ALICE, DAI, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let amount_in = 100 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: DAI,
					asset_out: BTC,
					amount_in,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			assert_balance!(TreasuryAccount::get(), DAI, 0);

			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), DAI, FEE_FOR_ONE_DCA_EXECUTION_IN_DAI);
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: DAI,
				asset_out: BTC,
				amount_in: amount_in - FEE_FOR_ONE_DCA_EXECUTION_IN_DAI,
				min_buy_amount: 79200000000000,
			}]);
		});
}

#[test]
fn native_execution_fee_should_be_sent_to_treasury() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

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
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));
			assert_balance!(TreasuryAccount::get(), HDX, 0);

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_balance!(TreasuryAccount::get(), HDX, FEE_FOR_ONE_DCA_EXECUTION);
			assert_number_of_executed_sell_trades!(1);
		});
}

#[test]
fn slippage_limit_should_be_used_for_sell_dca_when_it_is_smaller_than_specified_trade_min_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let sell_amount = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: sell_amount,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: sell_amount - FEE_FOR_ONE_DCA_EXECUTION,
				min_buy_amount: 7_920_000_000_000,
			}]);
		});
}

#[test]
fn slippage_limit_should_be_used_for_buy_dca_when_it_is_smaller_than_specified_trade_max_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);
			assert_balance!(ALICE, BTC, 0);

			let buy_amount = 10 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: DAI,
					amount_out: buy_amount,
					max_limit: Balance::MAX,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![BuyExecution {
				asset_in: HDX,
				asset_out: DAI,
				amount_out: buy_amount,
				max_sell_amount: 9680000000000,
			}]);
		});
}

#[test]
fn one_sell_dca_execution_should_be_rescheduled_when_price_diff_is_more_than_max_allowed() {
	let initial_alice_hdx_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, initial_alice_hdx_balance)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(501)));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![]);
			assert_eq!(
				total_amount - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);

			let schedule_id = 0;
			assert_scheduled_ids!(601, vec![schedule_id]);
		});
}

#[test]
fn one_buy_dca_execution_should_be_rescheduled_when_price_diff_is_more_than_max_allowed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;
			let max_limit = 2 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![]);
			assert_eq!(
				total_amount - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);

			let schedule_id = 0;
			assert_scheduled_ids!(601, vec![schedule_id]);
		});
}

#[test]
fn specified_slippage_should_be_used_in_circuit_breaker_price_check() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_buy = ONE;
			let max_limit = 2 * ONE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: amount_to_buy,
					max_limit,
					slippage: Some(Permill::from_percent(9)),
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_buy_trades!(vec![]);
			assert_eq!(
				total_amount - FEE_FOR_ONE_DCA_EXECUTION,
				Currencies::reserved_balance(HDX, &ALICE)
			);

			let schedule_id = 0;
			assert_scheduled_ids!(601, vec![schedule_id]);

			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(1, retries.unwrap());
		});
}

#[test]
fn dca_should_be_terminated_when_dca_cannot_be_planned_due_to_not_free_blocks() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;
			let one_block = 1;

			let schedule_id = 0;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(one_block)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			for _ in RangeInclusive::new(1, 120) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(502)));
			}

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![SellExecution {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: amount_to_sell - FEE_FOR_ONE_DCA_EXECUTION,
				min_buy_amount: 792000000000,
			}]);

			assert_that_dca_is_terminated(ALICE, schedule_id, Error::<Test>::NoFreeBlockFound.into());
		});
}

#[test]
fn dca_should_be_terminated_when_price_change_is_big_but_no_free_blocks_to_replan() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000000 * ONE)])
		.with_max_price_difference(Permill::from_percent(9))
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = 5 * ONE;
			let amount_to_sell = ONE;

			let schedule_id = 0;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_period(1)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: amount_to_sell,
					min_limit: Balance::MIN,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(501)));
			assert_eq!(total_amount, Currencies::reserved_balance(HDX, &ALICE));

			for _ in RangeInclusive::new(1, 120) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::Some(502)));
			}

			//Act
			set_to_blocknumber(501);

			//Assert
			assert_executed_sell_trades!(vec![]);
			assert_that_dca_is_terminated(ALICE, schedule_id, Error::<Test>::NoFreeBlockFound.into());
		});
}

#[test]
fn dca_sell_schedule_should_be_completed_after_one_trade_when_total_amount_is_equal_to_amount_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = ONE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: total_amount,
					min_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			assert_number_of_executed_sell_trades!(1);
			assert_that_dca_is_completed(ALICE, schedule_id);
		});
}

#[test]
fn dca_sell_schedule_should_be_terminated_when_schedule_allocation_is_more_than_reserved_funds() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 5000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, 500);

			let total_amount = ONE;
			let schedule = ScheduleBuilder::new()
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: total_amount,
					min_limit: 5 * ONE,
					slippage: None,
					route: empty_vec(),
				})
				.build();

			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			Currencies::unreserve_named(&NamedReserveId::get(), HDX, &ALICE, ONE / 2);

			//Act
			set_to_blocknumber(501);

			//Assert
			let schedule_id = 0;
			assert_number_of_executed_sell_trades!(0);
			assert_that_dca_is_terminated(ALICE, schedule_id, Error::<Test>::InvalidState.into());
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

fn assert_that_dca_is_completed(owner: AccountId, schedule_id: ScheduleId) {
	assert_that_schedule_has_been_removed_from_storages!(owner, schedule_id);

	expect_events(vec![DcaEvent::Completed {
		id: schedule_id,
		who: owner,
	}
	.into()]);
}

fn assert_that_dca_is_terminated(owner: AccountId, schedule_id: ScheduleId, error: DispatchError) {
	assert_that_schedule_has_been_removed_from_storages!(owner, schedule_id);

	expect_events(vec![DcaEvent::Terminated {
		id: schedule_id,
		who: owner,
		error,
	}
	.into()]);
}
