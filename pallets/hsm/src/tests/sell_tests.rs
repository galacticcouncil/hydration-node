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

use crate::tests::mock::*;
use crate::{Error, HollarAmountReceived};
use frame_support::traits::Hooks;
use frame_support::{assert_err, assert_noop, assert_ok, BoundedVec};
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::stableswap::AssetAmount;
use num_traits::One;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_broadcast::types::{Asset, Filler, TradeOperation};
use pallet_stableswap::types::PegSource;
use sp_runtime::FixedU128;
use sp_runtime::{Perbill, Permill};

// Setup helper to create a test environment with DAI as collateral
fn setup_test_with_dai_collateral() -> sp_io::TestExternalities {
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
			FixedU128::one(),
			Permill::from_percent(1),
		)
		.build();
	ext.execute_with(|| {
		move_block();
	});

	ext
}

fn setup_test_with_dai_collateral_with_different_decimals(decimals: u8) -> sp_io::TestExternalities {
	let one = 10u128.pow(decimals as u32);
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HOLLAR, 1_000 * ONE),
			(ALICE, DAI, 1_000 * one),
			(BOB, HOLLAR, 1_000 * ONE),
			(BOB, DAI, 1_000 * one),
			(PROVIDER, HOLLAR, 1_000 * ONE),
			(PROVIDER, DAI, 1_000 * one),
			(HSM::account_id(), DAI, 1_000 * one),
		])
		.with_registered_assets(vec![(HDX, 12), (DAI, decimals), (HOLLAR, 18), (100, 18)])
		// Create a stablepool for HOLLAR and DAI
		.build();
	ext.execute_with(|| {
		assert_ok!(Stableswap::create_pool(
			RuntimeOrigin::root(),
			100,
			BoundedVec::try_from(vec![HOLLAR, DAI]).unwrap(),
			2,
			Permill::from_percent(1),
		));
		let liquidity = vec![AssetAmount::new(HOLLAR, 1000 * ONE), AssetAmount::new(DAI, 900 * one)];
		assert_ok!(Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(PROVIDER),
			100,
			BoundedVec::try_from(liquidity).unwrap(),
			0,
		));
		assert_ok!(HSM::add_collateral_asset(
			RuntimeOrigin::root(),
			DAI,
			100,
			Permill::from_percent(1),
			FixedU128::one(),
			Permill::from_percent(1),
			Perbill::from_percent(50),
			None,
		));
		move_block();
	});

	ext
}

#[test]
fn sell_collateral_to_get_hollar_works() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values based on implementation
		let collateral_amount = 10 * ONE;
		let expected_hollar_amount = 9900990099009900990;

		// Execute the sell
		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HOLLAR,
			collateral_amount,
			1,
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai - collateral_amount);
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar + expected_hollar_amount
		);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai + collateral_amount
		);

		// Check that EVM mint call was made
		let (contract, _data) = last_evm_call().unwrap();
		assert_eq!(contract, EvmAddress::from(GHO_ADDRESS));

		// Check that the event was emitted correctly
		System::assert_has_event(
			pallet_broadcast::Event::<Test>::Swapped3 {
				swapper: ALICE,
				filler: HSM::account_id(),
				filler_type: Filler::HSM,
				operation: TradeOperation::ExactIn,
				inputs: vec![Asset::new(DAI, collateral_amount)],
				outputs: vec![Asset::new(HOLLAR, expected_hollar_amount)],
				fees: vec![],
				operation_stack: vec![],
			}
			.into(),
		);

		// Clean up for next test
		clear_evm_calls();
	});
}

#[test]
fn sell_hollar_to_get_collateral_works() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set initial collateral holdings for HSM
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967582916609;

		// Execute the sell
		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			hollar_amount,
			1, // Minimal slippage limit
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);

		// Check that EVM burn call was made
		let (contract, _data) = last_evm_call().unwrap();
		assert_eq!(contract, EvmAddress::from(GHO_ADDRESS));

		// Check that the event was emitted correctly
		System::assert_has_event(
			pallet_broadcast::Event::<Test>::Swapped3 {
				swapper: ALICE,
				filler: HSM::account_id(),
				filler_type: Filler::HSM,
				operation: TradeOperation::ExactIn,
				inputs: vec![Asset::new(HOLLAR, hollar_amount)],
				outputs: vec![Asset::new(DAI, expected_collateral_amount)],
				fees: vec![],
				operation_stack: vec![],
			}
			.into(),
		);

		// Clean up for next test
		clear_evm_calls();
	});
}

#[test]
fn sell_with_slippage_limit_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Execute the sell with a high slippage limit
		assert_err!(
			HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				10 * ONE,
				20 * ONE, // Unreasonably high slippage limit
			),
			Error::<Test>::SlippageLimitExceeded
		);
	});
}

#[test]
fn sell_with_invalid_asset_pair_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Try to sell HDX which is not a valid collateral asset
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(ALICE), HDX, HOLLAR, 10 * ONE, 5 * ONE,),
			Error::<Test>::InvalidAssetPair
		);

		// Try to sell HOLLAR for USDC which is not a valid collateral asset
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(ALICE), HOLLAR, USDC, 10 * ONE, 5 * ONE,),
			Error::<Test>::InvalidAssetPair
		);

		// Try to sell HOLLAR for HDX which is not a valid collateral asset
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(ALICE), HOLLAR, HDX, 10 * ONE, 5 * ONE,),
			Error::<Test>::InvalidAssetPair
		);
	});
}

#[test]
fn sell_hollar_with_insufficient_hsm_collateral_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set a low collateral holdings for HSM
		assert_ok!(Tokens::update_balance(
			DAI,
			&HSM::account_id(),
			-((Tokens::free_balance(DAI, &HSM::account_id()) - 2 * ONE) as i128)
		));

		// Try to sell more than the HSM holds
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 10 * ONE, 5 * ONE),
			Error::<Test>::InsufficientCollateralBalance
		);
	});
}

#[test]
fn sell_with_insufficient_balance_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Create a new account with insufficient balance
		let initial_charlie_dai = Tokens::free_balance(DAI, &CHARLIE);
		let initial_charlie_hollar = Tokens::free_balance(HOLLAR, &CHARLIE);

		// CHARLIE has no DAI
		assert_eq!(initial_charlie_dai, 0);

		// Try to sell DAI
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(CHARLIE), DAI, HOLLAR, 10 * ONE, 5 * ONE),
			orml_tokens::Error::<Test>::BalanceTooLow
		);

		// CHARLIE has no HOLLAR either
		assert_eq!(initial_charlie_hollar, 0);

		// Try to sell HOLLAR
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(CHARLIE), HOLLAR, DAI, 10 * ONE, 5 * ONE,),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
fn sell_hollar_with_max_buy_price_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// SEt max hollar price to lower than current price
		assert_ok!(HSM::update_collateral_asset(
			RuntimeOrigin::root(),
			DAI,
			None,
			Some(FixedU128::from_rational(90, 100)),
			None,
			None,
			Some(Some(10 * ONE)), // Set max holding to a low value
		));

		// Try to sell HOLLAR, should fail due to max buy price exceeded
		assert_err!(
			HSM::sell(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, 10 * ONE, 5 * ONE,),
			Error::<Test>::MaxBuyPriceExceeded
		);
	});
}

#[test]
fn sell_hollar_with_max_holding_exceeded_fails() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Update collateral to add a max_in_holding limit
		assert_ok!(HSM::update_collateral_asset(
			RuntimeOrigin::root(),
			DAI,
			None,
			None,
			None,
			None,
			Some(Some(10 * ONE)), // Set max holding to a low value
		));

		// The current holding is already at or near the limit
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 9 * ONE as i128));

		assert_err!(
			HSM::sell(RuntimeOrigin::signed(ALICE), DAI, HOLLAR, 5 * ONE, ONE,),
			Error::<Test>::MaxHoldingExceeded
		);
	});
}

#[test]
fn on_finalize_clears_hollar_amount_received() {
	setup_test_with_dai_collateral().execute_with(|| {
		// Set some initial value
		HollarAmountReceived::<Test>::insert(DAI, 100 * ONE);

		// Verify it's set
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), 100 * ONE);

		// Call on_finalize
		HSM::on_finalize(1);

		// Verify it's cleared
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), 0);
	});
}

#[test]
fn sell_purchase_zero_fee_works() {
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
					amount: 990 * ONE,
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

			let collateral_amount = 10 * ONE;
			let expected_hollar_amount = collateral_amount; // 1:1 peg with no fee - same amount

			// Execute the sell
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				1, // Minimal slippage limit
			));

			// Check that ALICE's balances are updated correctly
			assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai - collateral_amount);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				initial_alice_hollar + expected_hollar_amount
			);

			// Check that HSM holdings are updated correctly
			assert_eq!(
				Tokens::free_balance(DAI, &HSM::account_id()),
				initial_hsm_dai + collateral_amount
			);
		});
}

#[test]
fn sell_purchase_nonzero_fee_works() {
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
					amount: 990 * ONE,
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

			let collateral_amount = 10 * ONE;
			let expected_hollar_amount = 9900990099009900990;

			// Execute the sell
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				1, // Minimal slippage limit
			));

			// Check that ALICE's balances are updated correctly
			assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai - collateral_amount);
			assert_eq!(
				Tokens::free_balance(HOLLAR, &ALICE),
				initial_alice_hollar + expected_hollar_amount
			);

			// Check that HSM holdings are updated correctly
			let hsm_dai_final = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_dai_final, initial_hsm_dai + collateral_amount);
		});
}

#[test]
fn sell_hollar_zero_fee_works() {
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
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_percent(0),
			Perbill::from_percent(75),
		)
		.build()
		.execute_with(|| {
			move_block();
			// Set initial HSM collateral holdings
			let hsm_dai_initial = 100 * ONE;
			assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), hsm_dai_initial as i128));

			// First buy some hollar by selling collateral
			let collateral_amount = 10 * ONE;
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				1, // Minimal slippage limit
			));

			let alice_dai = Tokens::free_balance(DAI, &ALICE);
			let alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

			// ACT - sell hollar back
			let hollar_to_sell = ONE;
			let expected_collateral = 995456489239760326;

			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				HOLLAR,
				DAI,
				hollar_to_sell,
				1, // Minimal slippage limit
			));
			assert_eq!(Tokens::free_balance(DAI, &ALICE), alice_dai + expected_collateral);
			assert_eq!(Tokens::free_balance(HOLLAR, &ALICE), alice_hollar - hollar_to_sell);

			// Check that HSM holdings are updated correctly
			let hsm_dai_final = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_dai_final, hsm_dai - expected_collateral);
		});
}

#[test]
fn sell_hollar_nonzero_fee_works() {
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
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_float(0.001),
			Perbill::from_percent(75),
		)
		.build()
		.execute_with(|| {
			move_block();
			// Set initial HSM collateral holdings
			let hsm_dai_initial = 100 * ONE;
			assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), hsm_dai_initial as i128));

			// First buy some hollar by selling collateral
			let collateral_amount = 10 * ONE;
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				1, // Minimal slippage limit
			));

			let alice_dai = Tokens::free_balance(DAI, &ALICE);
			let alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
			let hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

			// ACT - sell hollar back
			let hollar_to_sell = ONE;
			let expected_collateral = 996452942181942269;

			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				HOLLAR,
				DAI,
				hollar_to_sell,
				1, // Minimal slippage limit
			));
			assert_eq!(Tokens::free_balance(DAI, &ALICE), alice_dai + expected_collateral);
			assert_eq!(Tokens::free_balance(HOLLAR, &ALICE), alice_hollar - hollar_to_sell);

			// Check that HSM holdings are updated correctly
			let hsm_dai_final = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_dai_final, hsm_dai - expected_collateral);
		});
}

#[test]
fn sell_hollar_fail_when_buyback_limit_exceeded() {
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
					amount: 990 * ONE,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_percent(0),
			Perbill::from_percent(50),
		)
		.build()
		.execute_with(|| {
			move_block();
			// First buy some hollar by selling collateral
			let collateral_amount = 10 * ONE;
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				1, // Minimal slippage limit
			));

			// ACT - sell hollar back
			let hollar_to_sell = 5 * ONE;
			assert_noop!(
				HSM::sell(
					RuntimeOrigin::signed(ALICE),
					HOLLAR,
					DAI,
					hollar_to_sell,
					1, // Minimal slippage limit
				),
				Error::<Test>::MaxBuyBackExceeded
			);
		});
}

#[test]
fn sell_hollar_nonzero_fee_should_fail_when_max_price_exceeded() {
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
					amount: 990 * ONE,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_float(0.001),
			Perbill::from_percent(75),
		)
		.build()
		.execute_with(|| {
			move_block();
			// First buy some hollar by selling collateral
			let collateral_amount = 10 * ONE;
			assert_ok!(HSM::sell(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HOLLAR,
				collateral_amount,
				1, // Minimal slippage limit
			));

			let hollar_to_sell = ONE;
			assert_noop!(
				HSM::sell(
					RuntimeOrigin::signed(ALICE),
					HOLLAR,
					DAI,
					hollar_to_sell,
					1, // Minimal slippage limit
				),
				Error::<Test>::MaxBuyPriceExceeded
			);
		});
}

// Trades, add, remove liquidity
#[test]
fn sell_hollar_to_get_collateral_should_yield_same_results_when_stablepool_state_changes_in_block_due_to_sell() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Do a stablepool trade
		assert_ok!(Stableswap::sell(
			RuntimeOrigin::signed(BOB),
			100,
			DAI,
			HOLLAR,
			10 * ONE,
			0,
		));

		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967582916609;

		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			hollar_amount,
			1, // Minimal slippage limit
		));

		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);
	});
}

#[test]
fn sell_hollar_to_get_collateral_should_yield_same_results_when_stablepool_state_changes_in_block_due_to_buy() {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Do a stablepool trade
		assert_ok!(Stableswap::buy(
			RuntimeOrigin::signed(BOB),
			100,
			DAI,
			HOLLAR,
			10 * ONE,
			u128::MAX,
		));

		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967582916609;

		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			hollar_amount,
			1, // Minimal slippage limit
		));

		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);
	});
}

#[test]
fn sell_hollar_to_get_collateral_should_yield_same_results_when_stablepool_state_changes_in_block_due_to_add_liquidity()
{
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Do a stablepool add liquidity
		assert_ok!(Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(BOB),
			100,
			BoundedVec::truncate_from(vec![AssetAmount::new(DAI, 10 * ONE)]),
			9,
		));

		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967582916609;

		assert_ok!(HSM::sell(RuntimeOrigin::signed(ALICE), HOLLAR, DAI, hollar_amount, 1,));

		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);
	});
}

#[test]
fn sell_hollar_to_get_collateral_should_yield_same_results_when_stablepool_state_changes_in_block_due_to_remove_liquidity(
) {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Do a stablepool remove liquidity
		assert_ok!(Stableswap::remove_liquidity(
			RuntimeOrigin::signed(PROVIDER),
			100,
			ONE,
			BoundedVec::truncate_from(vec![AssetAmount::new(DAI, 0), AssetAmount::new(HOLLAR, 0)]),
		));

		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967582916609;

		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			hollar_amount,
			1, // Minimal slippage limit
		));

		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);
	});
}

#[test]
fn sell_hollar_to_get_collateral_should_yield_same_results_when_stablepool_state_changes_in_block_due_to_multiple_operations(
) {
	setup_test_with_dai_collateral().execute_with(|| {
		move_block();
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Do a stablepool remove liquidity
		assert_ok!(Stableswap::remove_liquidity(
			RuntimeOrigin::signed(PROVIDER),
			100,
			ONE,
			BoundedVec::truncate_from(vec![AssetAmount::new(DAI, 0), AssetAmount::new(HOLLAR, 0)]),
		));

		// Do a stablepool trade
		assert_ok!(Stableswap::sell(
			RuntimeOrigin::signed(BOB),
			100,
			DAI,
			HOLLAR,
			10 * ONE,
			0,
		));

		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967582916609;

		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			hollar_amount,
			1, // Minimal slippage limit
		));

		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);
	});
}

#[test]
fn sell_collateral_to_get_hollar_works_when_collateral_has_different_decimals() {
	let one = 10u128.pow(12);
	setup_test_with_dai_collateral_with_different_decimals(12).execute_with(|| {
		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values based on implementation
		let collateral_amount = 10 * one;
		let expected_hollar_amount = 9900990099009900990;

		// Execute the sell
		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HOLLAR,
			collateral_amount,
			1,
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(Tokens::free_balance(DAI, &ALICE), initial_alice_dai - collateral_amount);
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar + expected_hollar_amount
		);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai + collateral_amount
		);

		// Check that EVM mint call was made
		let (contract, _data) = last_evm_call().unwrap();
		assert_eq!(contract, EvmAddress::from(GHO_ADDRESS));

		// Check that the event was emitted correctly
		System::assert_has_event(
			pallet_broadcast::Event::<Test>::Swapped3 {
				swapper: ALICE,
				filler: HSM::account_id(),
				filler_type: Filler::HSM,
				operation: TradeOperation::ExactIn,
				inputs: vec![Asset::new(DAI, collateral_amount)],
				outputs: vec![Asset::new(HOLLAR, expected_hollar_amount)],
				fees: vec![],
				operation_stack: vec![],
			}
			.into(),
		);

		// Clean up for next test
		clear_evm_calls();
	});
}

#[test]
fn sell_hollar_to_get_collateral_works_when_collateral_has_different_decimals() {
	let one = 10u128.pow(12);
	setup_test_with_dai_collateral_with_different_decimals(12).execute_with(|| {
		// Set initial collateral holdings for HSM
		assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * one as i128));

		// Initial state
		let initial_alice_dai = Tokens::free_balance(DAI, &ALICE);
		let initial_alice_hollar = Tokens::free_balance(HOLLAR, &ALICE);
		let initial_hsm_dai = Tokens::free_balance(DAI, &HSM::account_id());

		// Calculate expected values
		let hollar_amount = 10 * ONE;
		let expected_collateral_amount = 9883577967585;

		// Execute the sell
		assert_ok!(HSM::sell(
			RuntimeOrigin::signed(ALICE),
			HOLLAR,
			DAI,
			hollar_amount,
			1, // Minimal slippage limit
		));

		// Check that ALICE's balances are updated correctly
		assert_eq!(
			Tokens::free_balance(HOLLAR, &ALICE),
			initial_alice_hollar - hollar_amount
		);
		assert_eq!(
			Tokens::free_balance(DAI, &ALICE),
			initial_alice_dai + expected_collateral_amount
		);

		// Check that HSM holdings are updated correctly
		assert_eq!(
			Tokens::free_balance(DAI, &HSM::account_id()),
			initial_hsm_dai - expected_collateral_amount
		);

		// Check that HollarAmountReceived was updated correctly
		assert_eq!(HollarAmountReceived::<Test>::get(DAI), hollar_amount);

		// Check that EVM burn call was made
		let (contract, _data) = last_evm_call().unwrap();
		assert_eq!(contract, EvmAddress::from(GHO_ADDRESS));

		// Check that the event was emitted correctly
		System::assert_has_event(
			pallet_broadcast::Event::<Test>::Swapped3 {
				swapper: ALICE,
				filler: HSM::account_id(),
				filler_type: Filler::HSM,
				operation: TradeOperation::ExactIn,
				inputs: vec![Asset::new(HOLLAR, hollar_amount)],
				outputs: vec![Asset::new(DAI, expected_collateral_amount)],
				fees: vec![],
				operation_stack: vec![],
			}
			.into(),
		);

		// Clean up for next test
		clear_evm_calls();
	});
}
