//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  /
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/
//     .?P!.?GY7:   .. .    ^?PP^:JP~
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

use crate::tests::mock::*;

use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;

#[test]
fn partial_fill_order_should_work_when_order_is_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		let alice_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount_fill = 5 * ONE;
		assert_ok!(OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill));

		// Assert
		let expected_receive_amount = 25_000_000_000_000_u128;
		let expected_new_amount_buy = 15_000_000_000_000_u128;
		let expected_new_amount_sell = 75_000_000_000_000_u128;

		let alice_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		assert_eq!(
			alice_hdx_balance_after,
			alice_hdx_balance_before - expected_receive_amount
		);
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + expected_receive_amount);

		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_fill);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_fill);

		let order = OTC::orders(0).unwrap();
		assert_eq!(order.amount_buy, expected_new_amount_buy);
		assert_eq!(order.amount_sell, expected_new_amount_sell);

		expect_events(vec![Event::OrderPartiallyFilled {
			order_id: 0,
			who: BOB,
			amount_fill: 5 * ONE,
			amount_receive: expected_receive_amount,
		}
		.into()]);
	});
}

#[test]
fn complete_fill_order_should_work_when_order_is_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		let alice_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount_fill = 20 * ONE;
		assert_ok!(OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill));

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_none());

		let alice_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		assert_eq!(alice_hdx_balance_after, alice_hdx_balance_before - 100 * ONE);
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + 100 * ONE);

		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_fill);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_fill);

		expect_events(vec![Event::OrderFilled {
			order_id: 0,
			who: BOB,
			amount_fill: 20 * ONE,
		}
		.into()]);
	});
}

#[test]
fn complete_fill_order_should_work_when_order_is_not_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			false
		));

		let alice_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount_fill = 20 * ONE;
		assert_ok!(OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill));

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_none());

		let alice_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		assert_eq!(alice_hdx_balance_after, alice_hdx_balance_before - 100 * ONE);
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + 100 * ONE);

		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_fill);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_fill);

		expect_events(vec![Event::OrderFilled {
			order_id: 0,
			who: BOB,
			amount_fill: 20 * ONE,
		}
		.into()]);
	});
}

#[test]
fn partial_fill_order_should_throw_error_when_remaining_amounts_are_too_low() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		// Act
		let amount_fill = 15_000_000_000_001;
		assert_noop!(
			OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill),
			Error::<Test>::RemainingOrderSizeTooSmall
		);
	});
}

#[test]
fn partial_fill_order_should_throw_error_when_order_is_not_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			false
		));

		// Act
		let amount_fill = 5 * ONE;
		assert_noop!(
			OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill),
			Error::<Test>::OrderNotPartiallyFillable
		);
	});
}

#[test]
fn fill_order_should_throw_error_when_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			200 * ONE,
			100 * ONE,
			true
		));

		// Act
		let amount_fill = 110 * ONE;
		assert_noop!(
			OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn fill_order_should_throw_error_when_amount_fill_is_larger_than_order() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		// Act
		let amount_fill = 30 * ONE;
		assert_noop!(
			OTC::fill_order(Origin::signed(BOB), 0, DAI, amount_fill),
			Error::<Test>::CannotFillMoreThanOrdered
		);
	});
}
