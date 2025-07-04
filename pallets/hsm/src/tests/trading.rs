// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use frame_support::{assert_ok, traits::Hooks};
use hydradx_traits::stableswap::AssetAmount;
use num_traits::One;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use pallet_stableswap::types::PegSource;
use sp_runtime::{FixedU128, Permill};

use crate::tests::mock::*;

fn setup_test_for_comparison() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HOLLAR, 1_000 * ONE),
			(ALICE, DAI, 1_000 * ONE),
			(BOB, HOLLAR, 1_000 * ONE),
			(BOB, DAI, 1_000 * ONE),
			(HSM::account_id(), DAI, 1_000 * ONE),
		])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (USDC, 6), (HOLLAR, 18), (100, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(0),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1_000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 900 * ONE,
				},
			],
		)
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(0), // purchase_fee
			FixedU128::one(),         // max_buy_price_coefficient
			Permill::from_percent(0), // buy_back_fee
		)
		.build();

	ext.execute_with(|| {
		move_block();
	});

	ext
}

#[test]
fn selling_collateral_for_hollar_equals_buying_hollar_with_collateral() {
	setup_test_for_comparison().execute_with(|| {
		// Define a fixed amount of collateral to use in both operations
		let collateral_amount = 10 * ONE;

		// Scenario 1: Sell collateral to get Hollar
		let hollar_from_sell = {
			// Record initial balances
			let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);

			// Execute sell (collateral -> Hollar)
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				0, // Minimum amount out (we don't care about slippage here)
			));

			// Calculate how much Hollar ALICE received
			let final_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			final_alice_hollar - initial_alice_hollar
		};

		// Reset state for next test
		System::reset_events();
		HSM::on_finalize(1);
		clear_evm_calls();

		// Scenario 2: Buy Hollar with collateral
		let collateral_for_buy = {
			// Record initial balances
			let initial_bob_dai = Tokens::free_balance(DAI, &BOB);

			// Execute buy (buying the exact amount of Hollar we got from selling)
			assert_ok!(HSM::buy(
				RuntimeOrigin::signed(BOB),
				DAI,
				HOLLAR,
				hollar_from_sell,
				2 * collateral_amount, // High slippage limit
			));

			// Calculate how much collateral BOB paid
			let final_bob_dai = Tokens::free_balance(DAI, &BOB);
			initial_bob_dai - final_bob_dai
		};

		// Compare: collateral_amount should equal collateral_for_buy
		assert_eq!(
			collateral_amount, collateral_for_buy,
			"Selling {} DAI for Hollar should equal buying {} Hollar with DAI",
			collateral_amount, hollar_from_sell
		);
	});
}

#[test]
fn selling_hollar_for_collateral_equals_buying_collateral_with_hollar() {
	setup_test_for_comparison().execute_with(|| {
		// Ensure HSM has enough collateral
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Define a fixed amount of Hollar to use in both operations
		let hollar_amount = 10 * ONE;

		// Scenario 1: Sell Hollar to get collateral
		let collateral_from_sell = {
			// Record initial balances
			let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);

			// Execute sell (Hollar -> collateral)
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				HOLLAR,
				DAI,
				hollar_amount,
				0, // Minimum amount out (we don't care about slippage here)
			));

			// Calculate how much collateral ALICE received
			let final_alice_dai = Tokens::free_balance(DAI, &ALICE);
			final_alice_dai - initial_alice_dai
		};

		// Reset state for next test
		System::reset_events();
		HSM::on_finalize(1);
		clear_evm_calls();

		// Scenario 2: Buy collateral with Hollar
		let hollar_for_buy = {
			// Record initial balances
			let initial_bob_hollar = Tokens::free_balance(HOLLAR, &BOB);

			// Execute buy (buying the exact amount of collateral we got from selling)
			assert_ok!(HSM::buy(
				RuntimeOrigin::signed(BOB),
				HOLLAR,
				DAI,
				collateral_from_sell,
				2 * hollar_amount, // High slippage limit
			));

			// Calculate how much Hollar BOB paid
			let final_bob_hollar = Tokens::free_balance(HOLLAR, &BOB);
			initial_bob_hollar - final_bob_hollar
		};

		// Compare: hollar_amount should equal hollar_for_buy
		assert_eq!(
			hollar_amount, hollar_for_buy,
			"Selling {} Hollar for DAI should equal buying {} DAI with Hollar",
			hollar_amount, collateral_from_sell
		);
	});
}
