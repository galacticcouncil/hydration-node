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

use crate::tests::mock::*;
use crate::{Error, HollarAmountReceived};
use frame_support::traits::Hooks;
use frame_support::{assert_err, assert_ok, BoundedVec};
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::stableswap::AssetAmount;
use num_traits::One;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_stableswap::types::PegSource;
use sp_runtime::{FixedU128, Perbill, Permill};

// Setup helper to create a test environment with DAI as collateral
fn setup_test_with_dai_collateral() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HOLLAR, 1_000 * ONE),
			(ALICE, DAI, 1_000 * ONE),
			(BOB, DAI, 1_000 * ONE),
			(HSM::account_id(), DAI, 1_000 * ONE),
		])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (USDC, 6), (HOLLAR, 18), (100, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
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
			Permill::from_percent(1),
			FixedU128::from_rational(104, 100), // 100% coefficient as a ratio (1.0)
			Permill::from_percent(1),
		)
		.build();

	ext.execute_with(|| {
		move_block();
	});

	ext
}

#[test]
fn buy_hollar_with_collateral_works() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values based on implementation
		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 10100000000000000000;

		// Execute the buy
		assert_ok!(HSM::buy(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HOLLAR,
			hollar_amount,
			2 * expected_collateral_amount, // Higher than expected amount (slippage limit)
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai - expected_collateral_amount
		);
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar + hollar_amount
		);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai + expected_collateral_amount
		);

		// Check that EVM mint call was made
		let (contract, _data) = last_evm_call().unwrap();
		assert_eq!(contract, EvmAddress::from(GHO_ADDRESS));

		// Check that the event was emitted correctly
		System::assert_has_event(
			crate::Event::<Test>::BuyExecuted {
				who: ALICE,
				asset_in: DAI,
				asset_out: HOLLAR,
				amount_in: expected_collateral_amount,
				amount_out: hollar_amount,
			}
			.into(),
		);

		// Clean up for next test
		clear_evm_calls();
	});
}

#[test]
fn buy_collateral_with_hollar_works() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set initial collateral holdings for HSM
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let collateral_amount = 10 * ONE;
		let expected_hollar_amount = 10115651205298638227;

		// Execute the buy
		assert_ok!(HSM::buy(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			collateral_amount,
			2 * expected_hollar_amount, // Higher than expected amount (slippage limit)
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - expected_hollar_amount
		);
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai + collateral_amount);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), expected_hollar_amount);

		// Check that EVM call was made for burning Hollar
		let (contract, _data) = last_evm_call().unwrap();
		assert_eq!(contract, EvmAddress::from(GHO_ADDRESS));

		// Check that the event was emitted correctly
		System::assert_has_event(
			crate::Event::<Test>::BuyExecuted {
				who: ALICE,
				asset_in: HOLLAR,
				asset_out: DAI,
				amount_in: expected_hollar_amount,
				amount_out: collateral_amount,
			}
			.into(),
		);

		// Clean up for next test
		clear_evm_calls();
	});
}

#[test]
fn buy_with_slippage_limit_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Execute the buy with a low slippage limit
		assert_err!(
			HSM::buy(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				10 * ONE,
				5 * ONE, // Unreasonably low slippage limit (amount_in)
			),
			Error::<Test>::SlippageLimitExceeded
		);
	});
}

#[test]
fn buy_with_invalid_asset_pair_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Try to buy with HDX which is not a valid collateral asset
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), HDX, HOLLAR, 10 * ONE, 20 * ONE),
			Error::<Test>::InvalidAssetPair
		);

		// Try to buy HDX with HOLLAR which is not a valid collateral asset
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, HDX, 10 * ONE, 20 * ONE),
			Error::<Test>::InvalidAssetPair
		);

		// Try to buy USDC with HOLLAR which is not a valid collateral asset
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, USDC, 10 * ONE, 20 * ONE),
			Error::<Test>::InvalidAssetPair
		);
	});
}

#[test]
fn buy_collateral_with_insufficient_hsm_collateral_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set a low collateral holdings for HSM
		assert_ok!(Tokens::update_balance(
			DAI,
			&HSM::account_id(),
			-((Tokens::free_balance(DAI, &HSM::account_id()) - 2 * ONE) as i128)
		));

		let hsm_acc_balance = Tokens::free_balance(DAI, &HSM::account_id());

		assert_ok!(Tokens::update_balance(
			DAI,
			&HSM::account_id(),
			-((hsm_acc_balance - 2 * ONE) as i128)
		));

		// Try to buy more than the HSM holds
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 3 * ONE, 10 * ONE),
			Error::<Test>::InsufficientCollateralBalance
		);
	});
}

#[test]
fn buy_hollar_with_insufficient_balance_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set a low balance for ALICE
		assert_ok!(Tokens::update_balance(
			DAI,
			&ALICE,
			-((Tokens::free_balance(DAI, &ALICE) - ONE) as i128)
		));

		// Try to buy more than ALICE has
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), DAI, HOLLAR, 10 * ONE, 20 * ONE),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
fn buy_collateral_with_insufficient_hollar_balance_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Ensure HSM has collateral
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 2 * ONE as i128));

		// Try to buy more than ALICE has HOLLAR for
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(BOB), HOLLAR, DAI, 1 * ONE, 20 * ONE),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
fn buy_collateral_with_max_buy_price_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set an extremely low max_buy_price_coefficient to force failure
		assert_ok!(HSM::update_collateral_asset(
			RuntimeOrigin::root(),
			DAI,
			None,
			Some(FixedU128::from_rational(1, 100)), // 1/100 = 1% ratio to force max buy price failure
			None,
			None,
			None,
		));

		// Ensure HSM has collateral
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Try to buy collateral, should exceed max buy price
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 10 * ONE, 20 * ONE),
			Error::<Test>::MaxBuyPriceExceeded
		);
	});
}

#[test]
fn buy_collateral_with_max_buyback_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set a very low b parameter to force failure
		assert_ok!(HSM::update_collateral_asset(
			RuntimeOrigin::root(),
			DAI,
			None,
			None,
			None,
			Some(Perbill::from_percent(1)), // Set very low limit
			None,
		));

		// Set some existing HollarAmountReceived to make hitting the limit easier
		HollarAmountReceived::<Test>::insert(DAI, 10 * ONE);

		// Ensure HSM has collateral
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Try to buy collateral, should exceed max buyback
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 10 * ONE, 100 * ONE),
			Error::<Test>::MaxBuyBackExceeded
		);
	});
}

#[test]
fn buy_collateral_with_max_holding_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set max holding to a very low value
		assert_ok!(HSM::update_collateral_asset(
			RuntimeOrigin::root(),
			DAI,
			None,
			None,
			None,
			None,
			Some(Some(5 * ONE)),
		));

		// Execute the buy (should fail due to max holding)
		assert_err!(
			HSM::buy(RuntimeOrigin::signed(ALICE), DAI, HOLLAR, 10 * ONE, 20 * ONE),
			Error::<Test>::MaxHoldingExceeded
		);
	});
}

#[test]
fn on_finalize_clears_hollar_amount_received() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set some HollarAmountReceived value
		HollarAmountReceived::<Test>::insert(DAI, 10 * ONE);

		// Run on_finalize
		HSM::on_finalize(1);

		// Check that HollarAmountReceived was cleared
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), 0);
	});
}

#[test]
fn buy_purchase_zero_fee_works() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			pool_id,
			vec![HOLLAR, DAI],
			22,
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
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_percent(0),
		)
		.build()
		.execute_with(|| {
			move_block();
			// Initial state
			let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
			let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(initial_alice_dai, 1000 * ONE);
			assert_eq!(initial_alice_hollar, 0);
			assert_eq!(initial_hsm_dai, 0);

			let hollar_amount = 10 * ONE;
			let expected_collateral_amount = hollar_amount; // 1:1 peg with no fee - same amount

			// Execute the sell
			assert_ok!(HSM::buy(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				hollar_amount,
				u128::MAX,
			));

			// Check that ALICE's balances are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &ALICE),
				initial_alice_dai - expected_collateral_amount
			);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				initial_alice_hollar + hollar_amount
			);

			// Check that HSM holdings are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &HSM::account_id()),
				initial_hsm_dai + expected_collateral_amount
			);
		});
}

#[test]
fn buy_one_hollar_works_when_purchase_nonzero_fee() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			pool_id,
			vec![HOLLAR, DAI],
			22,
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
			pool_id,
			Permill::from_percent(1),
			FixedU128::one(),
			Permill::from_percent(0),
		)
		.build()
		.execute_with(|| {
			move_block();
			// Initial state
			let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
			let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(initial_alice_dai, 1000 * ONE);
			assert_eq!(initial_alice_hollar, 0);
			assert_eq!(initial_hsm_dai, 0);

			let expected_collateral_amount = 1010000000000000000;
			let hollar_amount = 1 * ONE; // 1:1 peg with 1% fee

			// Execute the sell
			assert_ok!(HSM::buy(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				hollar_amount,
				u128::MAX,
			));

			// Check that ALICE's balances are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &ALICE),
				initial_alice_dai - expected_collateral_amount
			);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				initial_alice_hollar + hollar_amount
			);

			// Check that HSM holdings are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &HSM::account_id()),
				initial_hsm_dai + expected_collateral_amount
			);
		});
}

#[test]
fn buy_purchase_nonzero_fee_works() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			pool_id,
			vec![HOLLAR, DAI],
			22,
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
			pool_id,
			Permill::from_percent(1),
			FixedU128::one(),
			Permill::from_percent(0),
		)
		.build()
		.execute_with(|| {
			move_block();
			// Initial state
			let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
			let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(initial_alice_dai, 1000 * ONE);
			assert_eq!(initial_alice_hollar, 0);
			assert_eq!(initial_hsm_dai, 0);

			let expected_collateral_amount = 10000000000000;
			let hollar_amount = 9900990099009; // 1:1 peg with 1% fee

			// Execute the sell
			assert_ok!(HSM::buy(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				hollar_amount,
				u128::MAX,
			));

			let spent = initial_alice_dai - Tokens::free_balance(DAI, &ALICE);
			dbg!(spent);

			// Check that ALICE's balances are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &ALICE),
				initial_alice_dai - expected_collateral_amount
			);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				initial_alice_hollar + hollar_amount
			);

			// Check that HSM holdings are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &HSM::account_id()),
				initial_hsm_dai + expected_collateral_amount
			);
		});
}

#[test]
fn buy_collateral_works_when_buy_fee_is_zero() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			pool_id,
			vec![HOLLAR, DAI],
			22,
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
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_percent(0),
		)
		.build()
		.execute_with(|| {
			move_block();
			assert_ok!(HSM::buy(RuntimeOrigin::signed(ALICE), DAI, HOLLAR, 10 * ONE, u128::MAX,));

			let alice_dai = Tokens::free_balance(DAI, &ALICE);
			let alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

			let expected_hollar_amount = 1004564035979960838;

			assert_ok!(HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 1 * ONE, u128::MAX,));

			// Check that ALICE's balances are updated correctly
			assert_eq!(Tokens::free_balance(DAI, &ALICE), alice_dai + ONE,);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				alice_hollar - expected_hollar_amount
			);

			// Check that HSM holdings are updated correctly
			assert_eq!(Tokens::free_balance(DAI, &HSM::account_id()), hsm_dai - ONE,);
		});
}

#[test]
fn buy_collateral_works_when_buy_fee_is_nonzero() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			pool_id,
			vec![HOLLAR, DAI],
			22,
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
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_float(0.001),
		)
		.build()
		.execute_with(|| {
			move_block();
			assert_ok!(HSM::buy(RuntimeOrigin::signed(ALICE), DAI, HOLLAR, 10 * ONE, u128::MAX,));

			let alice_dai = Tokens::free_balance(DAI, &ALICE);
			let alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

			let expected_hollar_amount = 1003559471943980877; // should be less by the fee than amount with 0 fee

			assert_ok!(HSM::buy(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 1 * ONE, u128::MAX,));

			// Check that ALICE's balances are updated correctly
			assert_eq!(Tokens::free_balance(DAI, &ALICE), alice_dai + ONE,);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				alice_hollar - expected_hollar_amount
			);

			// Check that HSM holdings are updated correctly
			assert_eq!(Tokens::free_balance(DAI, &HSM::account_id()), hsm_dai - ONE,);
		});
}

#[test]
fn buy_collateral_should_yield_same_results_when_stabepool_state_changes_due_to_sell() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let collateral_amount = 10 * ONE;
		let expected_hollar_amount = 10115651205298638227;

		// Do a stablepool trade
		assert_ok!(Stableswap::sell(
			RuntimeOrigin::signed(BOB),
			100,
			DAI,
			HOLLAR,
			10 * ONE,
			0
		));

		// Execute the buy
		assert_ok!(HSM::buy(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			collateral_amount,
			2 * expected_hollar_amount, // Higher than expected amount (slippage limit)
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - expected_hollar_amount
		);
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai + collateral_amount);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), expected_hollar_amount);
	});
}

#[test]
fn buy_collateral_should_yield_same_results_when_stabepool_state_changes_due_to_buy() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		// Set initial collateral holdings for HSM
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let collateral_amount = 10 * ONE;
		let expected_hollar_amount = 10115651205298638227;

		// Do a stablepool trade
		assert_ok!(Stableswap::buy(
			RuntimeOrigin::signed(BOB),
			100,
			HOLLAR,
			DAI,
			10 * ONE,
			u128::MAX,
		));

		// Execute the buy
		assert_ok!(HSM::buy(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			collateral_amount,
			2 * expected_hollar_amount, // Higher than expected amount (slippage limit)
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - expected_hollar_amount
		);
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai + collateral_amount);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), expected_hollar_amount);
	});
}

#[test]
fn buy_collateral_should_yield_same_results_when_stabepool_state_changes_due_to_add_liquidity() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let collateral_amount = 10 * ONE;
		let expected_hollar_amount = 10115651205298638227;
		// Do a stablepool add liquidity
		assert_ok!(Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(BOB),
			100,
			BoundedVec::truncate_from(vec![AssetAmount::new(DAI, 10 * ONE)]),
			9,
		));

		// Execute the buy
		assert_ok!(HSM::buy(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			collateral_amount,
			2 * expected_hollar_amount, // Higher than expected amount (slippage limit)
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - expected_hollar_amount
		);
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai + collateral_amount);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), expected_hollar_amount);
	});
}
#[test]
fn buy_collateral_should_yield_same_results_when_stabepool_state_changes_due_to_remove_liquidity() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let collateral_amount = 10 * ONE;
		let expected_hollar_amount = 10115651205298638227;

		// Do a stablepool remove liquidity
		assert_ok!(Stableswap::remove_liquidity(
			RuntimeOrigin::signed(PROVIDER),
			100,
			ONE,
			BoundedVec::truncate_from(vec![AssetAmount::new(DAI, 0), AssetAmount::new(HOLLAR, 0)]),
		));

		// Execute the buy
		assert_ok!(HSM::buy(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			collateral_amount,
			2 * expected_hollar_amount, // Higher than expected amount (slippage limit)
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - expected_hollar_amount
		);
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai + collateral_amount);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), expected_hollar_amount);
	});
}
