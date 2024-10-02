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
use hydradx_traits::router::AssetPair;
use hydradx_traits::router::PoolType;
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn sell_should_work_when_route_has_single_trade() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let limit = 5;

		let trades = vec![HDX_AUSD_TRADE_IN_XYK];

		let alice_balance = Currencies::free_balance(HDX, &ALICE);

		//Act
		assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, AUSD, limit, trades));

		//Assert
		assert_executed_sell_trades(vec![(PoolType::XYK, alice_balance, HDX, AUSD)]);
		expect_events(vec![Event::Executed {
			asset_in: HDX,
			asset_out: AUSD,
			amount_in: alice_balance,
			amount_out: XYK_SELL_CALCULATION_RESULT,
			event_id: 0,
		}
		.into()]);
	});
}

#[test]
fn sell_should_work_with_omnipool_when_no_specified_or_onchain_route_exist() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let limit = 1;

		let alice_balance = Currencies::free_balance(HDX, &ALICE);

		//Act
		assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, AUSD, limit, vec![]));

		//Assert
		assert_executed_sell_trades(vec![(PoolType::Omnipool, alice_balance, HDX, AUSD)]);
		expect_events(vec![Event::Executed {
			asset_in: HDX,
			asset_out: AUSD,
			amount_in: alice_balance,
			amount_out: OMNIPOOL_SELL_CALCULATION_RESULT,
			event_id: 0,
		}
		.into()]);
	});
}

#[test]
fn sell_should_work_when_route_has_single_trade_without_native_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, KSM, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let limit = 5;

			let alice_nonnative_balance = Currencies::free_balance(KSM, &ALICE);

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: KSM,
				asset_out: AUSD,
			}];

			//Act
			assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), KSM, AUSD, limit, trades));

			//Assert
			assert_executed_sell_trades(vec![(PoolType::XYK, alice_nonnative_balance, KSM, AUSD)]);
		});
}

#[test]
fn sell_should_work_when_route_has_multiple_trades_with_same_pooltype() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let alice_native_balance = Currencies::free_balance(HDX, &ALICE);

		let limit = 5;
		let trade1 = Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
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
		assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, KSM, limit, trades));

		//Assert
		assert_executed_sell_trades(vec![
			(PoolType::XYK, alice_native_balance, HDX, AUSD),
			(PoolType::XYK, XYK_SELL_CALCULATION_RESULT, AUSD, MOVR),
			(PoolType::XYK, XYK_SELL_CALCULATION_RESULT, MOVR, KSM),
		]);
		expect_events(vec![Event::Executed {
			asset_in: HDX,
			asset_out: KSM,
			amount_in: alice_native_balance,
			amount_out: XYK_SELL_CALCULATION_RESULT,
			event_id: 0,
		}
		.into()]);
	});
}

#[test]
fn sell_should_work_when_route_has_multiple_trades_with_different_pool_type() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let alice_native_balance = Currencies::free_balance(HDX, &ALICE);

		let limit = 1;
		let trade1 = Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
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
		assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, KSM, limit, trades));

		//Assert
		assert_executed_sell_trades(vec![
			(PoolType::XYK, alice_native_balance, HDX, MOVR),
			(PoolType::Stableswap(AUSD), XYK_SELL_CALCULATION_RESULT, MOVR, AUSD),
			(PoolType::Omnipool, STABLESWAP_SELL_CALCULATION_RESULT, AUSD, KSM),
		]);

		expect_events(vec![Event::Executed {
			asset_in: HDX,
			asset_out: KSM,
			amount_in: alice_native_balance,
			amount_out: OMNIPOOL_SELL_CALCULATION_RESULT,
			event_id: 0,
		}
		.into()]);
	});
}

#[test]
fn sell_should_work_with_onchain_route_when_no_routes_specified() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let alice_native_balance = Currencies::free_balance(HDX, &ALICE);
		let limit = 1;
		let trade1 = Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: MOVR,
		};
		let trade2 = Trade {
			pool: PoolType::Stableswap(AUSD),
			asset_in: MOVR,
			asset_out: AUSD,
		};
		let trade3 = Trade {
			pool: PoolType::XYK,
			asset_in: AUSD,
			asset_out: KSM,
		};
		let trades = vec![trade1, trade2, trade3];
		assert_ok!(Router::set_route(
			RuntimeOrigin::signed(ALICE),
			AssetPair::new(HDX, KSM),
			trades,
		));

		//Act
		assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, KSM, limit, vec![]));

		//Assert
		assert_last_executed_sell_trades(
			3,
			vec![
				(PoolType::XYK, alice_native_balance, HDX, MOVR),
				(PoolType::Stableswap(AUSD), XYK_SELL_CALCULATION_RESULT, MOVR, AUSD),
				(PoolType::XYK, STABLESWAP_SELL_CALCULATION_RESULT, AUSD, KSM),
			],
		);

		expect_events(vec![Event::Executed {
			asset_in: HDX,
			asset_out: KSM,
			amount_in: alice_native_balance,
			amount_out: XYK_SELL_CALCULATION_RESULT,
			event_id: 0,
		}
		.into()]);
	});
}

#[test]
fn sell_should_work_with_onchain_route_when_onchain_route_present_in_reverse_order() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, KSM, 2000)])
		.build()
		.execute_with(|| {
			//Arrange
			let alice_nonnative_balance = Currencies::free_balance(KSM, &ALICE);

			let limit = 1;
			let trade1 = Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: MOVR,
			};
			let trade2 = Trade {
				pool: PoolType::Stableswap(AUSD),
				asset_in: MOVR,
				asset_out: AUSD,
			};
			let trade3 = Trade {
				pool: PoolType::XYK,
				asset_in: AUSD,
				asset_out: KSM,
			};
			let trades = vec![trade1, trade2, trade3];
			assert_ok!(Router::set_route(
				RuntimeOrigin::signed(ALICE),
				AssetPair::new(HDX, KSM),
				trades,
			));

			//Act
			//it fails, the amount out is not there after all three trades.
			assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), KSM, HDX, limit, vec![]));

			//Assert
			assert_last_executed_sell_trades(
				3,
				vec![
					(PoolType::XYK, alice_nonnative_balance, KSM, AUSD),
					(PoolType::Stableswap(AUSD), XYK_SELL_CALCULATION_RESULT, AUSD, MOVR),
					(PoolType::XYK, STABLESWAP_SELL_CALCULATION_RESULT, MOVR, HDX),
				],
			);

			expect_events(vec![Event::Executed {
				asset_in: KSM,
				asset_out: HDX,
				amount_in: alice_nonnative_balance,
				amount_out: XYK_SELL_CALCULATION_RESULT,
				event_id: 0,
			}
			.into()]);
		});
}

#[test]
fn sell_should_work_when_first_trade_is_not_supported_in_the_first_pool() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let alice_native_balance = Currencies::free_balance(HDX, &ALICE);
		let limit = 5;
		let trade1 = Trade {
			pool: PoolType::Stableswap(AUSD),
			asset_in: HDX,
			asset_out: AUSD,
		};
		let trade2 = Trade {
			pool: PoolType::XYK,
			asset_in: AUSD,
			asset_out: KSM,
		};
		let trades = vec![trade1, trade2];

		//Act
		assert_ok!(Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, KSM, limit, trades));

		//Assert
		assert_executed_sell_trades(vec![
			(PoolType::Stableswap(AUSD), alice_native_balance, HDX, AUSD),
			(PoolType::XYK, STABLESWAP_SELL_CALCULATION_RESULT, AUSD, KSM),
		]);
	});
}

#[test]
fn sell_should_fail_when_max_limit_for_trade_reached() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let trade1 = Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
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
			let trade5 = Trade {
				pool: PoolType::XYK,
				asset_in: RMRK,
				asset_out: SDN,
			};
			let trade6 = Trade {
				pool: PoolType::XYK,
				asset_in: SDN,
				asset_out: STABLE_SHARE_ASSET,
			};
			let trades = vec![trade1, trade2, trade3, trade4, trade5, trade6];

			//Act and Assert
			assert_noop!(
				Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, SDN, 5, trades),
				Error::<Test>::MaxTradesExceeded
			);
		});
}

#[test]
fn sell_should_fail_when_called_with_non_signed_origin() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let limit = 5;
		let trades = vec![HDX_AUSD_TRADE_IN_XYK];

		//Act and Assert
		assert_noop!(
			Router::sell_all(RuntimeOrigin::none(), HDX, AUSD, limit, trades),
			BadOrigin
		);
	});
}

#[test]
fn sell_should_fail_when_min_limit_to_receive_is_not_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let limit = XYK_SELL_CALCULATION_RESULT + 1;

		let trades = vec![HDX_AUSD_TRADE_IN_XYK];

		//Act and Assert
		assert_noop!(
			Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, AUSD, limit, trades),
			Error::<Test>::TradingLimitReached
		);
	});
}

#[test]
fn sell_should_fail_when_assets_dont_correspond_to_route() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, AUSD, 1000)])
		.build()
		.execute_with(|| {
			//Arrange
			let limit = 5;

			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: AUSD,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: MOVR,
				},
			];

			//Act and assert
			assert_noop!(
				Router::sell_all(RuntimeOrigin::signed(ALICE), MOVR, AUSD, limit, trades),
				Error::<Test>::InvalidRoute
			);
		});
}

#[test]
fn sell_should_fail_when_intermediare_assets_are_inconsistent() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let limit = 5;
		let trade1 = Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		};
		let trade2 = Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: MOVR,
		};
		let trade3 = Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: KSM,
		};
		let trades = vec![trade1, trade2, trade3];

		//Act
		assert_noop!(
			Router::sell_all(RuntimeOrigin::signed(ALICE), HDX, KSM, limit, trades),
			Error::<Test>::InvalidRoute
		);
	});
}
