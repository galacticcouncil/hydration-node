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
use orml_traits::MultiReservableCurrency;
use pretty_assertions::assert_eq;

#[test]
fn cancel_order_should_work() {
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
		assert_ok!(OTC::cancel_order(Origin::signed(ALICE), 0));

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_none());

		assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 0_u128,);

		expect_events(vec![Event::OrderCancelled { order_id: 0 }.into()]);
	});
}

#[test]
fn cancel_order_should_throw_error_when_order_does_not_exist() {
	ExtBuilder::default().build().execute_with(|| {
		// Act
		assert_noop!(
			OTC::cancel_order(Origin::signed(ALICE), 0),
			Error::<Test>::OrderNotFound
		);
	});
}

#[test]
fn cancel_order_should_throw_error_when_called_by_non_owner() {
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
		assert_noop!(OTC::cancel_order(Origin::signed(BOB), 0), Error::<Test>::NoPermission);

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_some());

		assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 100 * ONE,);
	});
}
