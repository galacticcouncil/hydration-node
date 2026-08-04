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

// When the OTC `asset_in` is a StableSwap share, the settlement can only deliver what the maker's
// payment actually buys from the pool, so an order asking for more shares than it pays for cannot
// be settled: it reverts, minting nothing.
#[test]
fn settle_otc_order_should_not_over_mint_shares_when_maker_underpays_for_stableswap_share() {
	use frame_support::storage::with_transaction;
	use frame_support::{assert_noop, BoundedVec};
	use hydradx_traits::router::{AssetPair, PoolType, RouteProvider, Trade};
	use sp_runtime::{DispatchResult, TransactionOutcome};

	TestNet::reset();
	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// StableSwap pool set up the same way as the router/DCA integration tests. `pool_id` is the
			// pool's share token; `asset_b` is one of the pooled stablecoins.
			let (pool_id, _asset_a, asset_b) = crate::dca::init_stableswap().unwrap();

			// The route the settlement buys shares through: depositing `asset_b` into the pool. It is an
			// add-liquidity because the output asset is the share token `pool_id`.
			let pair = AssetPair {
				asset_in: asset_b,
				asset_out: pool_id,
			};
			assert_ok!(hydradx_runtime::Router::force_insert_route(
				hydradx_runtime::RuntimeOrigin::root(),
				pair,
				BoundedVec::truncate_from(vec![Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: asset_b,
					asset_out: pool_id,
				}]),
			));
			let route = hydradx_runtime::Router::get_route(pair);

			// Fund the attacker with the stablecoin they will pay with.
			let attacker = AccountId::from(CHARLIE);
			let payment = 1_000_000_000_000_000_000u128; // 1 unit of asset_b (18 decimals)
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				attacker.clone(),
				asset_b,
				(payment * 10) as i128,
			));

			// The maker places an OTC order buying more shares (`want_lp`) than `payment` fairly mints.
			// `asset_in` is the share token the maker receives.
			let want_lp = payment + payment / 10; // asks for 10% more shares than the payment buys
			assert_ok!(hydradx_runtime::OTC::place_order(
				hydradx_runtime::RuntimeOrigin::signed(attacker.clone()),
				pool_id,
				asset_b,
				want_lp,
				payment,
				true,
			));

			// Self-settling this order must revert and change nothing: the pool only mints what
			// `payment` buys, which is less than `want_lp`.
			assert_noop!(
				hydradx_runtime::OtcSettlements::settle_otc_order(
					hydradx_runtime::RuntimeOrigin::signed(attacker),
					0,
					want_lp,
					route,
				),
				pallet_otc_settlements::Error::<hydradx_runtime::Runtime>::TradeAmountTooHigh
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
