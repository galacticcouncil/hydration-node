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

use crate::assert_scheduled_ids;
use crate::tests::create_bounded_vec_with_schedule_ids;
use crate::tests::mock::*;
use crate::tests::{create_bounded_vec, ScheduleBuilder};
use crate::{Error, Event, Order};
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;
use std::ops::RangeInclusive;
use test_case::test_case;

#[test]
fn schedule_should_reserve_all_total_amount_as_named_reserve() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange

			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 10 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();
			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			assert_eq!(
				total_amount,
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE)
			);
		});
}

#[test]
fn schedule_should_store_total_amounts_in_storage() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange

			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 10 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();
			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			let schedule_id = 0;
			assert_eq!(DCA::remaining_amounts(schedule_id).unwrap(), total_amount);
		});
}

#[test]
fn schedule_should_compound_named_reserve_for_multiple_schedules() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange

			let total_amount = 10000 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 100 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			let total_amount_2 = 20000 * ONE;
			let schedule_2 = ScheduleBuilder::new()
				.with_total_amount(total_amount_2)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 1000 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule_2, Option::None));

			//Assert
			assert_eq!(
				total_amount + total_amount_2,
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE)
			);
			let schedule_id = 0;
			assert_eq!(DCA::remaining_amounts(schedule_id).unwrap(), total_amount);

			let schedule_id_2 = 1;
			assert_eq!(DCA::remaining_amounts(schedule_id_2).unwrap(), total_amount_2);
		});
}

#[test]
fn schedule_should_store_schedule_for_next_block_when_no_blocknumber_specified() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			let schedule_id = 0;
			let stored_schedule = DCA::schedules(schedule_id).unwrap();
			assert_eq!(stored_schedule, ScheduleBuilder::new().build());

			//Check if schedule ids are stored
			let schedule_ids = DCA::schedule_ids_per_block(501);
			assert!(!DCA::schedule_ids_per_block(501).is_empty());
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![schedule_id]);
			assert_eq!(schedule_ids, expected_scheduled_ids_for_next_block);

			//Check if schedule ownership is created
			assert!(DCA::owner_of(ALICE, schedule_id).is_some());
		});
}

#[test]
fn schedule_should_work_when_multiple_schedules_stored() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			//Act
			set_block_number(500);

			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			let schedule_id = 0;
			let schedule_id_2 = 1;
			assert!(DCA::schedules(schedule_id).is_some());
			assert!(DCA::schedules(schedule_id_2).is_some());

			let scheduled_ids_for_next_block = DCA::schedule_ids_per_block(501);

			let expected_scheduled_ids_for_next_block =
				create_bounded_vec_with_schedule_ids(vec![schedule_id, schedule_id_2]);
			assert_eq!(scheduled_ids_for_next_block, expected_scheduled_ids_for_next_block);
		});
}

#[test]
fn schedule_should_work_when_block_is_specified_by_user() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::Some(600)
			));

			//Assert
			let schedule_id = 0;
			let stored_schedule = DCA::schedules(schedule_id).unwrap();
			assert_eq!(stored_schedule, schedule);

			//Check if schedule ids are stored
			let schedule_ids = DCA::schedule_ids_per_block(600);
			assert!(!DCA::schedule_ids_per_block(600).is_empty());
			let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids(vec![schedule_id]);
			assert_eq!(schedule_ids, expected_scheduled_ids_for_next_block);

			//Check if schedule ownership is created
			assert!(DCA::owner_of(ALICE, schedule_id).is_some());
		});
}

#[test]
fn schedule_should_emit_necessary_events() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();

			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));

			//Assert
			let schedule_id = 0;
			expect_events(vec![
				Event::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 501,
				}
				.into(),
				Event::Scheduled {
					id: schedule_id,
					who: ALICE,
					period: schedule.period,
					total_amount: schedule.total_amount,
					order: schedule.order,
				}
				.into(),
			]);
		});
}

#[test]
fn schedule_should_emit_necessary_events_when_multiple_schedules_are_created() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			let schedule2 = ScheduleBuilder::new().build();

			//Act and assert
			set_block_number(500);

			let schedule_id = 0;
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule.clone(),
				Option::None
			));
			expect_events(vec![
				Event::ExecutionPlanned {
					id: schedule_id,
					who: ALICE,
					block: 501,
				}
				.into(),
				Event::Scheduled {
					id: schedule_id,
					who: ALICE,
					period: schedule.period,
					total_amount: schedule.total_amount,
					order: schedule.order,
				}
				.into(),
			]);

			let schedule_id2 = 1;

			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE),
				schedule2.clone(),
				Option::Some(1000)
			));
			expect_events(vec![
				Event::ExecutionPlanned {
					id: schedule_id2,
					who: ALICE,
					block: 1000,
				}
				.into(),
				Event::Scheduled {
					id: schedule_id2,
					who: ALICE,
					period: schedule2.period,
					total_amount: schedule2.total_amount,
					order: schedule2.order,
				}
				.into(),
			]);
		});
}

#[test]
fn schedule_should_throw_error_when_user_has_not_enough_balance() {
	let total_amount_to_be_taken = 100 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, total_amount_to_be_taken - 1)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().with_total_amount(100 * ONE).build();

			//Act
			set_block_number(500);
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				pallet_balances::Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn sell_schedule_should_throw_error_when_total_budget_is_smaller_than_amount_to_sell_plus_fee() {
	let budget = 5 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new()
				.with_total_amount(budget)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: budget + BUY_DCA_FEE_IN_NATIVE,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act
			set_block_number(500);

			//Assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::BudgetTooLow
			);
		});
}

#[test]
fn buy_schedule_should_throw_error_when_total_budget_is_smaller_than_amount_in_plus_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let budget = CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY + BUY_DCA_FEE_IN_NATIVE - 1;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(budget)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: 10 * ONE,
					max_amount_in: budget + BUY_DCA_FEE_IN_NATIVE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act
			set_block_number(500);

			//Assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::BudgetTooLow
			);
		});
}

#[test]
fn buy_schedule_should_work_when_total_budget_is_equal_to_calculated_amount_in_plus_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let budget = CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY + BUY_DCA_FEE_IN_NATIVE;

			let schedule = ScheduleBuilder::new()
				.with_total_amount(budget)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: 10 * ONE,
					max_amount_in: budget + BUY_DCA_FEE_IN_NATIVE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act
			set_block_number(500);

			//Assert
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),);
		});
}

#[test]
fn schedule_should_fail_when_not_called_by_user() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let schedule = ScheduleBuilder::new().build();

		//Act and assert
		assert_noop!(DCA::schedule(RuntimeOrigin::none(), schedule, Option::None), BadOrigin);
	});
}

#[test_case(1)]
#[test_case(499)]
#[test_case(500)]
fn schedule_should_fail_when_specified_next_block_is_not_greater_than_current_block(block: BlockNumberFor<Test>) {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let schedule = ScheduleBuilder::new().build();
			set_block_number(500);

			//Act and assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(block)),
				Error::<Test>::BlockNumberIsNotInFuture
			);
		});
}

#[test]
fn schedule_should_schedule_for_consequent_block_when_next_block_is_full() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			for _ in RangeInclusive::new(1, 20) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			}

			//Act
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 20;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			let actual_schedule_ids = DCA::schedule_ids_per_block(501);
			assert_eq!(20, actual_schedule_ids.len());

			assert_scheduled_ids!(502, vec![schedule_id]);
		});
}

#[test]
fn schedule_should_schedule_for_after_consequent_block_when_both_next_block_and_consquent_block_is_full() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			for _ in RangeInclusive::new(1, 40) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			}

			//Act
			let schedule = ScheduleBuilder::new().build();
			let schedule_id = 40;
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			let block = 501;
			let actual_schedule_ids = DCA::schedule_ids_per_block(block);
			assert_eq!(20, actual_schedule_ids.len());

			let actual_schedule_ids = DCA::schedule_ids_per_block(block + GENERATED_SEARCH_RADIUSES[0]);
			assert_eq!(20, actual_schedule_ids.len());

			assert_scheduled_ids!(
				block + GENERATED_SEARCH_RADIUSES[0] + GENERATED_SEARCH_RADIUSES[1],
				vec![schedule_id]
			);
		});
}

#[test]
fn schedule_should_fail_when_there_is_no_free_consquent_blocks() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			for _ in RangeInclusive::new(1, 220) {
				let schedule = ScheduleBuilder::new().build();
				assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
			}

			//Check if all the blocks within radiuses are fully filled
			let next_block = 501;
			let mut next_block_with_radius = next_block;
			for radius in GENERATED_SEARCH_RADIUSES {
				next_block_with_radius += radius;
				let actual_schedule_ids = DCA::schedule_ids_per_block(next_block_with_radius);
				assert_eq!(20, actual_schedule_ids.len());
			}

			//Act and assert
			let schedule = ScheduleBuilder::new().build();
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::NoFreeBlockFound
			);
		});
}

#[test]
fn schedule_should_fail_when_total_amount_is_smaller_than_min_budget_and_sold_currency_is_native() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange

			let schedule = ScheduleBuilder::new()
				.with_total_amount(*ORIGINAL_MIN_BUDGET_IN_NATIVE - 1)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 100 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act and Assert
			set_block_number(500);

			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::TotalAmountIsSmallerThanMinBudget
			);
		});
}

#[test]
fn schedule_should_fail_when_total_amount_in_non_native_currency_is_smaller_than_min_budget_in_native() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE), (ALICE, DAI, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange

			let schedule = ScheduleBuilder::new()
				.with_total_amount(*ORIGINAL_MIN_BUDGET_IN_NATIVE / 3)
				.with_order(Order::Buy {
					asset_in: DAI,
					asset_out: HDX,
					amount_out: ONE,
					max_amount_in: 100 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act and Assert
			set_block_number(500);

			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::TotalAmountIsSmallerThanMinBudget
			);
		});
}

#[test]
fn schedule_should_work_when_sell_amount_is_equal_to_20_times_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: SELL_DCA_FEE_IN_NATIVE * 20,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();
			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),);
		});
}
#[test]
fn schedule_should_fail_when_trade_amount_is_less_than_20x_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: SELL_DCA_FEE_IN_NATIVE * 20 - 1,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();
			//Act
			set_block_number(500);

			//Assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::MinTradeAmountNotReached
			);
		});
}

#[test]
fn schedule_should_fail_when_trade_amount_is_less_than_min_trading_limit() {
	let min_trading_limit = ONE / 10;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.with_min_trading_limit(min_trading_limit)
		.build()
		.execute_with(|| {
			//Arrange
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in: min_trading_limit - 1,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();
			//Act
			set_block_number(500);

			//Assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::MinTradeAmountNotReached
			);
		});
}

#[test]
fn sell_schedule_should_work_when_total_amount_is_equal_to_amount_in_plus_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_in = ONE;
			let total_amount = amount_in + SELL_DCA_FEE_IN_NATIVE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Sell {
					asset_in: HDX,
					asset_out: BTC,
					amount_in,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			//Act and Assert
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
		});
}

#[test]
fn schedule_should_init_retries_to_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange

			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 10 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();
			//Act
			set_block_number(500);
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

			//Assert
			let schedule_id = 0;
			let retries = DCA::retries_on_error(schedule_id);
			assert_eq!(0, retries);
		});
}

#[test]
fn schedule_should_fail_when_wrong_user_is_specified_in_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_owner(BOB)
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 10 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			set_block_number(500);

			//Act and assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn schedule_should_be_created_when_no_routes_specified() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 10 * ONE,
					route: create_bounded_vec(vec![]),
				})
				.build();

			//Act and assert
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));
		});
}

#[test]
fn thousands_of_dcas_should_be_schedules_on_a_specific_block_because_of_salt_added_to_block_search_randomness() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100000000000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let total_amount = 100 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(total_amount)
				.with_order(Order::Buy {
					asset_in: HDX,
					asset_out: BTC,
					amount_out: ONE,
					max_amount_in: 10 * ONE,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					}]),
				})
				.build();

			use_prod_randomness();

			//Act and assert
			set_block_number(500);
			for _ in RangeInclusive::new(1, 10000) {
				assert_ok!(DCA::schedule(
					RuntimeOrigin::signed(ALICE),
					schedule.clone(),
					Option::None
				));
			}
		});
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
