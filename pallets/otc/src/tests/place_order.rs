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
fn create_order_should_work() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(
				OTC::place_order(Origin::signed(ALICE), DAI, HDX, ONE, 10 * ONE, true)
			);

			// Assert
			let order = OTC::orders(0).unwrap();
			assert_eq!(order.owner, ALICE);
			assert_eq!(order.asset_buy, DAI);
			assert_eq!(order.asset_sell, HDX);
			assert_eq!(order.amount_sell, 10 * ONE);
			assert_eq!(order.amount_buy, ONE);
			assert_eq!(order.partially_fillable, true);

			// TODO: fix events
			// expect_events(vec![
			// 	Event::OrderPlaced { order_id: 0 }.into(),
			// ]);

			assert_eq!(
				Currencies::reserved_balance(HDX.into(), &ALICE.into()),
				10 * ONE,
			);

			let next_order_id = OTC::next_order_id();
			assert_eq!(next_order_id, 1);
		});
}

#[test]
fn create_order_should_throw_error_when_amount_is_higher_than_balance() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				OTC::place_order(Origin::signed(ALICE), DAI, HDX, ONE, 100_000 * ONE, true),
				Error::<Test>::InsufficientBalance
			);
		}
	);
}

#[test]
fn create_order_should_throw_error_when_asset_sell_is_not_registered() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				OTC::place_order(Origin::signed(ALICE), DAI, DOGE, ONE, 10 * ONE, true),
				Error::<Test>::AssetNotRegistered
			);
		}
	);
}

#[test]
fn create_order_should_throw_error_when_asset_buy_is_not_registered() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				OTC::place_order(Origin::signed(ALICE), DOGE, HDX, ONE, 10 * ONE, true),
				Error::<Test>::AssetNotRegistered
			);
		}
	);
}
