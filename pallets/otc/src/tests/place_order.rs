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
use orml_traits::NamedMultiReservableCurrency;
use pretty_assertions::assert_eq;

#[test]
fn create_order_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_ok!(OTC::place_order(
			Origin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		// Assert
		let order = OTC::orders(0).unwrap();
		assert_eq!(order.owner, ALICE);
		assert_eq!(order.asset_buy, DAI);
		assert_eq!(order.asset_sell, HDX);
		assert_eq!(order.amount_sell, 100 * ONE);
		assert_eq!(order.amount_buy, 20 * ONE);
		assert_eq!(order.partially_fillable, true);

		expect_events(vec![Event::OrderPlaced {
			order_id: 0,
			asset_buy: DAI,
			asset_sell: HDX,
			amount_buy: order.amount_buy,
			amount_sell: order.amount_sell,
			partially_fillable: true,
		}
		.into()]);

		let reserve_id = named_reserve_identifier(0);
		assert_eq!(Currencies::reserved_balance_named(&reserve_id, HDX, &ALICE), 100 * ONE);

		let next_order_id = OTC::next_order_id();
		assert_eq!(next_order_id, 1);
	});
}

#[test]
fn create_order_should_throw_error_when_amount_is_higher_than_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::place_order(Origin::signed(ALICE), DAI, HDX, 20 * ONE, 100_000 * ONE, true),
			Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn create_order_should_throw_error_when_asset_sell_is_not_registered() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::place_order(Origin::signed(ALICE), DAI, DOGE, 20 * ONE, 100 * ONE, true),
			Error::<Test>::AssetNotRegistered
		);
	});
}

#[test]
fn create_order_should_throw_error_when_asset_buy_is_not_registered() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::place_order(Origin::signed(ALICE), DOGE, HDX, 20 * ONE, 100 * ONE, true),
			Error::<Test>::AssetNotRegistered
		);
	});
}

#[test]
fn create_order_should_throw_error_when_amount_buy_is_too_low() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::place_order(Origin::signed(ALICE), DAI, HDX, 5 * ONE, 100 * ONE, true),
			Error::<Test>::OrderSizeTooSmall
		);
	});
}

#[test]
fn create_order_should_throw_error_when_amount_sell_is_too_low() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::place_order(Origin::signed(ALICE), DAI, HDX, 20 * ONE, 5 * ONE, true),
			Error::<Test>::OrderSizeTooSmall
		);
	});
}
