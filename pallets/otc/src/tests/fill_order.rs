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
use orml_traits::{MultiReservableCurrency, MultiCurrency};
use test_case::test_case;

#[test]
fn fill_order_should_work_when_fill_is_partial() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
      // Arrange
			assert_ok!(
        OTC::place_order(Origin::signed(ALICE), DAI, HDX, 20 * ONE, 100 * ONE, true)
			);
      
      let alice_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
      let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

      let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
      let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

      // Act
      let amount_fill = 5 * ONE;
      assert_ok!(
				OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill)
			);

			// Assert
      let expected_receive_amount = 24_999_999_999_999_u128;
      let expected_new_amount_buy = 15_000_000_000_000_u128;
      let expected_new_amount_sell = 75_000_000_000_001_u128;

      let alice_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
      let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

      let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
      let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

      assert_eq!(alice_hdx_balance_after, alice_hdx_balance_before - expected_receive_amount);
      assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + expected_receive_amount);

      assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_fill);
      assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_fill);

      let order = OTC::orders(0).unwrap();
      assert_eq!(order.amount_buy, expected_new_amount_buy);
      assert_eq!(order.amount_sell, expected_new_amount_sell);
		});
}