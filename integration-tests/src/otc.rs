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
#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use orml_traits::NamedMultiReservableCurrency;
use pallet_otc::NAMED_RESERVE_ID;
use xcm_emulator::TestExt;

#[test]
fn place_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			false,
		));

		// Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_some());
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE.into()),
			100 * UNITS
		);
	});
}

#[test]
fn partial_fill_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::partial_fill_order(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			0,
			15 * UNITS,
		));

		// Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_some());
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE.into()),
			25 * UNITS
		);
	});
}

#[test]
fn fill_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::fill_order(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			0,
		));

		// Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_none());
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE.into()),
			0
		);
	});
}

#[test]
fn cancel_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::cancel_order(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0
		));

		// Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_none());
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE.into()),
			0
		);
	});
}
