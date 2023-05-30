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
use crate::{Error, Event, Trade};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::router::PoolType;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn buy_should_work_when_route_has_single_trade() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let amount_to_buy = 10;
		let limit = 5;

		let trades = vec![BSX_AUSD_TRADE_IN_XYK];

		//Act
		assert_ok!(Router::buy(
			RuntimeOrigin::signed(ALICE),
			BSX,
			AUSD,
			amount_to_buy,
			limit,
			trades
		));

		//Assert
		assert_executed_buy_trades(vec![(PoolType::XYK, amount_to_buy, BSX, AUSD)]);
		expect_events(vec![Event::RouteExecuted {
			asset_in: BSX,
			asset_out: AUSD,
			amount_in: XYK_BUY_CALCULATION_RESULT,
			amount_out: amount_to_buy,
		}
		.into()]);
	});
}

#[test]
fn buy_should_work_when_route_has_single_trade_without_native_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, AUSD, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_to_buy = 10;
			let limit = 5;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: AUSD,
				asset_out: KSM,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE),
				AUSD,
				KSM,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_executed_buy_trades(vec![(PoolType::XYK, amount_to_buy, AUSD, KSM)]);
		});
}

#[test]
fn buy_should_fail_when_max_limit_for_trade_reached() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, RMRK, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let trade1 = Trade {
				pool: PoolType::XYK,
				asset_in: BSX,
				asset_out: AUSD,
			};
			let trade2 = Trade {
				pool: PoolType::XYK,
				asset_in: AUSD,
				asset_out: MOVR,
			};
			let trade3 = Trade {
				pool: PoolType::XYK,
				asset_in: MOVR,
				asset_out: KSM,
			};
			let trade4 = Trade {
				pool: PoolType::XYK,
				asset_in: KSM,
				asset_out: RMRK,
			};
			let trades = vec![trade1, trade2, trade3, trade4];

			//Act and Assert
			assert_noop!(
				Router::buy(RuntimeOrigin::signed(ALICE), BSX, RMRK, 10, 5, trades),
				Error::<Test>::MaxTradesExceeded
			);
		});
}

#[test]
fn buy_should_fail_when_route_has_single_trade_producing_calculation_error() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, AUSD, INVALID_CALCULATION_AMOUNT)])
		.build()
		.execute_with(|| {
			//Arrange
			let limit = 5;

			let trades = vec![BSX_AUSD_TRADE_IN_XYK];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(ALICE),
					BSX,
					AUSD,
					INVALID_CALCULATION_AMOUNT,
					limit,
					trades
				),
				DispatchError::Other("Some error happened")
			);
		});
}

#[test]
fn buy_should_when_route_has_multiple_trades_with_same_pool_type() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, KSM, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_to_buy = 10;
			let limit = 5;
			let trade1 = Trade {
				pool: PoolType::XYK,
				asset_in: BSX,
				asset_out: AUSD,
			};
			let trade2 = Trade {
				pool: PoolType::XYK,
				asset_in: AUSD,
				asset_out: MOVR,
			};
			let trade3 = Trade {
				pool: PoolType::XYK,
				asset_in: MOVR,
				asset_out: KSM,
			};
			let trades = vec![trade1, trade2, trade3];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE),
				BSX,
				KSM,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_executed_buy_trades(vec![
				(PoolType::XYK, XYK_BUY_CALCULATION_RESULT, BSX, AUSD),
				(PoolType::XYK, XYK_BUY_CALCULATION_RESULT, AUSD, MOVR),
				(PoolType::XYK, amount_to_buy, MOVR, KSM),
			]);

			expect_events(vec![Event::RouteExecuted {
				asset_in: BSX,
				asset_out: KSM,
				amount_in: XYK_BUY_CALCULATION_RESULT,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
}

#[test]
fn buy_should_work_when_route_has_multiple_trades_with_different_pool_type() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, KSM, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_to_buy = 10;
			let limit = 5;
			let trade1 = Trade {
				pool: PoolType::XYK,
				asset_in: BSX,
				asset_out: MOVR,
			};
			let trade2 = Trade {
				pool: PoolType::Stableswap(AUSD),
				asset_in: MOVR,
				asset_out: AUSD,
			};
			let trade3 = Trade {
				pool: PoolType::Omnipool,
				asset_in: AUSD,
				asset_out: KSM,
			};
			let trades = vec![trade1, trade2, trade3];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE),
				BSX,
				KSM,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_executed_buy_trades(vec![
				(PoolType::XYK, STABLESWAP_BUY_CALCULATION_RESULT, BSX, MOVR),
				(PoolType::Stableswap(AUSD), OMNIPOOL_BUY_CALCULATION_RESULT, MOVR, AUSD),
				(PoolType::Omnipool, amount_to_buy, AUSD, KSM),
			]);

			expect_events(vec![Event::RouteExecuted {
				asset_in: BSX,
				asset_out: KSM,
				amount_in: XYK_BUY_CALCULATION_RESULT,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
}

#[test]
fn buy_should_work_when_first_trade_is_not_supported_in_the_first_pool() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, KSM, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_to_buy = 10;
			let limit = 5;
			let trade1 = Trade {
				pool: PoolType::Stableswap(AUSD),
				asset_in: BSX,
				asset_out: AUSD,
			};
			let trade2 = Trade {
				pool: PoolType::XYK,
				asset_in: AUSD,
				asset_out: KSM,
			};
			let trades = vec![trade1, trade2];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE),
				BSX,
				KSM,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_executed_buy_trades(vec![
				(PoolType::Stableswap(AUSD), XYK_BUY_CALCULATION_RESULT, BSX, AUSD),
				(PoolType::XYK, amount_to_buy, AUSD, KSM),
			]);
		});
}

#[test]
fn buy_should_fail_when_called_with_non_signed_origin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, AUSD, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_to_buy = 10;
			let limit = 5;

			let trades = vec![BSX_AUSD_TRADE_IN_XYK];

			//Act and Assert
			assert_noop!(
				Router::buy(RuntimeOrigin::none(), BSX, AUSD, amount_to_buy, limit, trades),
				BadOrigin
			);
		});
}

#[test]
fn buy_should_fail_when_route_has_no_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, AUSD, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let trades = vec![];

			//Act and Assert
			assert_noop!(
				Router::buy(RuntimeOrigin::signed(ALICE), BSX, AUSD, 10, 5, trades),
				Error::<Test>::RouteHasNoTrades
			);
		});
}

#[test]
fn buy_should_fail_when_max_limit_to_spend_is_reached() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, AUSD, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let amount_to_buy = 10;
			let limit = XYK_BUY_CALCULATION_RESULT - 1;

			let trades = vec![BSX_AUSD_TRADE_IN_XYK];

			//Act and Assert
			assert_noop!(
				Router::buy(RuntimeOrigin::signed(ALICE), BSX, AUSD, amount_to_buy, limit, trades),
				Error::<Test>::TradingLimitReached
			);
		});
}
