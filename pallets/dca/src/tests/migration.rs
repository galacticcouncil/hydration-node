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

use crate::tests::on_initialize::{proceed_to_blocknumber, set_to_blocknumber};
use crate::tests::*;
use crate::{
	assert_balance, assert_number_of_executed_sell_trades, assert_that_schedule_has_been_removed_from_storages,
	CancelReason, Error, Event as DcaEvent, Order,
};
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError;
use std::borrow::Borrow;

const START_BLOCK: BlockNumber = 500;
const EXECUTION_BLOCK: BlockNumber = 502;

fn enable_migration() {
	assert_ok!(DCA::set_migration_enabled(RuntimeOrigin::root(), true));
}

fn sell_order(amount_in: Balance, min_amount_out: Balance) -> Order<AssetId> {
	Order::Sell {
		asset_in: HDX,
		asset_out: BTC,
		amount_in,
		min_amount_out,
		route: create_bounded_vec(vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: HDX,
			asset_out: BTC,
		}]),
	}
}

fn buy_order(amount_out: Balance, max_amount_in: Balance) -> Order<AssetId> {
	Order::Buy {
		asset_in: HDX,
		asset_out: BTC,
		amount_out,
		max_amount_in,
		route: create_bounded_vec(vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: HDX,
			asset_out: BTC,
		}]),
	}
}

fn migrated_intent() -> (AccountId, ice_support::DcaParams) {
	MIGRATED_INTENTS.with(|v| {
		let intents = v.borrow();
		assert_eq!(intents.len(), 1, "exactly one intent should have been created");
		intents[0].clone()
	})
}

#[test]
fn migration_should_convert_schedule_into_intent_when_execution_slot_arrives() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_period(ONE_HUNDRED_BLOCKS)
				.with_slippage(Some(Permill::from_percent(3)))
				.with_order(sell_order(10 * ONE, 5 * ONE))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let remaining = DCA::remaining_amounts(schedule_id).unwrap();

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			let (owner, params) = migrated_intent();
			assert_eq!(owner, ALICE);
			assert_eq!(params.asset_in, HDX);
			assert_eq!(params.asset_out, BTC);
			assert_eq!(params.amount_in, 10 * ONE);
			assert_eq!(params.amount_out, 5 * ONE);
			assert_eq!(params.slippage, Permill::from_percent(3));
			assert_eq!(params.budget, Some(remaining));
			assert_eq!(params.period, ONE_HUNDRED_BLOCKS as u32);

			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);

			expect_events(vec![DcaEvent::Migrated {
				id: schedule_id,
				who: ALICE,
				intent_id: 1,
			}
			.into()]);
		});
}

#[test]
fn migration_should_not_execute_trade_when_schedule_converts() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			insert_schedule_into_storage(ALICE, schedule, None);
			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			assert_number_of_executed_sell_trades!(0);
		});
}

#[test]
fn migration_should_move_whole_reserve_to_intent_when_budget_is_fixed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let remaining = DCA::remaining_amounts(schedule_id).unwrap();
			let free_before = Currencies::free_balance(HDX, &ALICE);

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			assert_eq!(
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE),
				0
			);
			assert_eq!(
				Currencies::reserved_balance_named(&INTENT_NAMED_RESERVE_ID, HDX, &ALICE),
				remaining
			);
			assert_balance!(ALICE, HDX, free_before);
		});
}

#[test]
fn migration_should_release_excess_reserve_when_schedule_is_rolling() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let amount_in = 10 * ONE;
			let schedule = ScheduleBuilder::new()
				.with_total_amount(0)
				.with_order(sell_order(amount_in, 0))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let dca_reserve = DCA::remaining_amounts(schedule_id).unwrap();
			let free_before = Currencies::free_balance(HDX, &ALICE);

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			let (_, params) = migrated_intent();
			assert_eq!(params.budget, None, "rolling schedule must stay rolling");

			let intent_reserve = amount_in.saturating_mul(2);
			assert_eq!(
				Currencies::reserved_balance_named(&INTENT_NAMED_RESERVE_ID, HDX, &ALICE),
				intent_reserve
			);
			// The old pallet reserved the transaction fee on top of the trade amount; the intent
			// pallet does not, so the difference goes back to the owner.
			assert_balance!(ALICE, HDX, free_before + dca_reserve - intent_reserve);
		});
}

#[test]
fn migration_should_cancel_and_refund_when_order_is_buy() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_slippage(Some(Permill::from_percent(20)))
				.with_order(buy_order(ONE, 50 * ONE))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let remaining = DCA::remaining_amounts(schedule_id).unwrap();
			let free_before = Currencies::free_balance(HDX, &ALICE);

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			assert_number_of_executed_sell_trades!(0);
			assert_balance!(ALICE, HDX, free_before + remaining);
			assert_eq!(
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE),
				0
			);
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);

			expect_events(vec![DcaEvent::MigrationCancelled {
				id: schedule_id,
				who: ALICE,
				asset: HDX,
				refunded: remaining,
				reason: CancelReason::BuyOrder,
			}
			.into()]);
		});
}

#[test]
fn migration_should_cancel_when_remaining_budget_is_below_one_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			RemainingAmounts::<Test>::insert(schedule_id, ONE);

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			expect_events(vec![DcaEvent::MigrationCancelled {
				id: schedule_id,
				who: ALICE,
				asset: HDX,
				refunded: ONE,
				reason: CancelReason::BudgetBelowTrade,
			}
			.into()]);
		});
}

#[test]
fn migration_should_cancel_and_refund_when_intent_creation_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let remaining = DCA::remaining_amounts(schedule_id).unwrap();
			let free_before = Currencies::free_balance(HDX, &ALICE);

			enable_migration();
			MIGRATOR_SHOULD_FAIL.with(|v| *v.borrow_mut() = true);

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			assert_balance!(ALICE, HDX, free_before + remaining);
			assert_eq!(
				Currencies::reserved_balance_named(&NamedReserveId::get(), HDX, &ALICE),
				0
			);
			assert_eq!(
				Currencies::reserved_balance_named(&INTENT_NAMED_RESERVE_ID, HDX, &ALICE),
				0
			);
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);

			expect_events(vec![DcaEvent::MigrationCancelled {
				id: schedule_id,
				who: ALICE,
				asset: HDX,
				refunded: remaining,
				reason: CancelReason::IntentCreationFailed(sp_runtime::TokenError::FundsUnavailable.into()),
			}
			.into()]);
		});
}

#[test]
fn schedule_should_execute_normally_when_migration_is_disabled() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(ONE, Balance::MIN))
				.build();
			insert_schedule_into_storage(ALICE, schedule, None);

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			assert_number_of_executed_sell_trades!(1);
			MIGRATED_INTENTS.with(|v| assert!(v.borrow().is_empty()));
		});
}

#[test]
fn schedule_should_fail_when_migration_is_enabled() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);
			enable_migration();

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();

			//Act and assert
			assert_noop!(
				DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, None),
				Error::<Test>::MigrationInProgress
			);
		});
}

/// The deploy-time default. Nothing writes `MigrationEnabled` — the pallet has no
/// `GenesisConfig` and no migration sets it — so the flag is absent on a fresh
/// runtime and `ValueQuery` must resolve it to `false`. If this ever flipped, a
/// runtime upgrade would start converting schedules the moment it went live.
#[test]
fn migration_enabled_should_be_false_when_never_set() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(!crate::MigrationEnabled::<Test>::exists());
		assert!(!DCA::migration_enabled());
	});
}

#[test]
fn set_migration_enabled_should_unblock_scheduling_when_set_back_to_false() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);
			enable_migration();

			//Act
			assert_ok!(DCA::set_migration_enabled(RuntimeOrigin::root(), false));

			//Assert
			assert!(!DCA::migration_enabled());
			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, None));
		});
}

#[test]
fn set_migration_enabled_should_fail_when_origin_is_not_terminate_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			DCA::set_migration_enabled(RuntimeOrigin::signed(ALICE), true),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn force_cancel_should_remove_schedule_and_refund_when_called_by_terminate_origin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let remaining = DCA::remaining_amounts(schedule_id).unwrap();
			let free_before = Currencies::free_balance(HDX, &ALICE);

			//Act
			assert_ok!(DCA::force_cancel_schedules(RuntimeOrigin::root(), vec![schedule_id]));

			//Assert
			assert_balance!(ALICE, HDX, free_before + remaining);
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			expect_events(vec![DcaEvent::MigrationCancelled {
				id: schedule_id,
				who: ALICE,
				asset: HDX,
				refunded: remaining,
				reason: CancelReason::ForceCancelled,
			}
			.into()]);
		});
}

#[test]
fn force_cancel_should_skip_unknown_ids_when_schedule_does_not_exist() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DCA::force_cancel_schedules(RuntimeOrigin::root(), vec![42]));
	});
}

#[test]
fn migration_should_convert_schedule_when_min_amount_out_is_dust() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(10 * ONE, 1))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			let (_, params) = migrated_intent();
			assert_eq!(params.amount_out, INTENT_ED);

			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			expect_events(vec![DcaEvent::Migrated {
				id: schedule_id,
				who: ALICE,
				intent_id: 1,
			}
			.into()]);
		});
}

#[test]
fn migration_should_cancel_when_amount_in_is_below_existential_deposit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_order(sell_order(INTENT_ED - 1, 5 * ONE))
				.build();
			let schedule_id = insert_schedule_into_storage(ALICE, schedule, None);
			let remaining = DCA::remaining_amounts(schedule_id).unwrap();

			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			MIGRATED_INTENTS.with(|v| assert!(v.borrow().is_empty()));
			assert_that_schedule_has_been_removed_from_storages!(ALICE, schedule_id);
			expect_events(vec![DcaEvent::MigrationCancelled {
				id: schedule_id,
				who: ALICE,
				asset: HDX,
				refunded: remaining,
				reason: CancelReason::IntentCreationFailed(INTENT_BELOW_ED),
			}
			.into()]);
		});
}

#[test]
fn migration_should_use_default_slippage_when_schedule_has_none() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			proceed_to_blocknumber(1, START_BLOCK);

			let schedule = ScheduleBuilder::new()
				.with_total_amount(100 * ONE)
				.with_slippage(None)
				.with_order(sell_order(10 * ONE, 0))
				.build();
			insert_schedule_into_storage(ALICE, schedule, None);
			enable_migration();

			//Act
			set_to_blocknumber(EXECUTION_BLOCK);

			//Assert
			let (_, params) = migrated_intent();
			assert_eq!(params.slippage, OmnipoolMaxAllowedPriceDifference::get());
		});
}
