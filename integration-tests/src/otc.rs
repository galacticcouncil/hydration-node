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
use pallet_otc::{types::OrderId, RESERVE_ID_PREFIX};
use xcm_emulator::TestExt;
#[test]
fn place_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			false,
		));

		// Assert
		let order = hydradx_runtime::OTC::orders(1);
		assert!(order.is_some());

		let reserve_id = named_reserve_identifier(1);
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&reserve_id, HDX, &ALICE.into()),
			100 * UNITS
		);
	});
}

#[test]
fn fill_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::fill_order(
			hydradx_runtime::Origin::signed(BOB.into()),
			1,
			DAI,
			15 * UNITS,
		));

		//Assert
		let order = hydradx_runtime::OTC::orders(1);
		assert!(order.is_some());

		let reserve_id = named_reserve_identifier(1);
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&reserve_id, HDX, &ALICE.into()),
			25 * UNITS
		);
	});
}

#[test]
fn cancel_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::cancel_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			1
		));

		//Assert
		let order = hydradx_runtime::OTC::orders(1);
		assert!(order.is_none());

		let reserve_id = named_reserve_identifier(1);
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&reserve_id, HDX, &ALICE.into()),
			0
		);
	});
}

fn named_reserve_identifier(order_id: OrderId) -> [u8; 8] {
	let mut result = [0; 8];
	result[0..3].copy_from_slice(RESERVE_ID_PREFIX);
	result[3..7].copy_from_slice(&order_id.to_be_bytes());

	result
}
