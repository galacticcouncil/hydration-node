// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate as otc;
use crate::tests::mock::*;
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;

#[test]
fn cancel_order_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		// Act
		assert_ok!(OTC::cancel_order(RuntimeOrigin::signed(ALICE), 0));

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_none());

		assert_eq!(Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE), 0);

		expect_events(vec![Event::Cancelled { order_id: 0 }.into()]);
	});
}

#[test]
fn cancel_order_should_throw_error_when_order_does_not_exist() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::cancel_order(RuntimeOrigin::signed(ALICE), 0),
			Error::<Test>::OrderNotFound
		);
	});
}

#[test]
fn cancel_order_should_throw_error_when_called_by_non_owner() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		// Act
		assert_noop!(
			OTC::cancel_order(RuntimeOrigin::signed(BOB), 0),
			Error::<Test>::Forbidden
		);

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_some());

		assert_eq!(
			Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE),
			100 * ONE
		);
	});
}
