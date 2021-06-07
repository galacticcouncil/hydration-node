// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

use super::*;
pub use crate::mock::{
	Currency, Event as TestEvent, Exchange, ExtBuilder, Origin, System, Test, ALICE, BOB, CHARLIE, DAVE, DOT, ETH,
	FERDIE, GEORGE, HDX, XYK as XYKPallet,
};
use frame_support::sp_runtime::traits::Hash;
use frame_support::sp_runtime::FixedPointNumber;
use frame_support::traits::OnFinalize;
use frame_support::{assert_noop, assert_ok};
use frame_system::InitKind;
use primitives::Price;
use sp_runtime::DispatchError;

use pallet_xyk as xyk;

const ENDOWED_AMOUNT: u128 = 1_000_000_000_000_000;

fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn last_event() -> TestEvent {
	system::Pallet::<Test>::events().pop().expect("Event expected").event
}

fn expect_event<E: Into<TestEvent>>(e: E) {
	assert_eq!(last_event(), e.into());
}

fn last_events(n: usize) -> Vec<TestEvent> {
	system::Pallet::<Test>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

fn expect_events(e: Vec<TestEvent>) {
	assert_eq!(last_events(e.len()), e);
}

fn generate_intention_id(account: &<Test as system::Config>::AccountId, c: u32) -> crate::IntentionId<Test> {
	let b = <system::Pallet<Test>>::current_block_number();
	(c, &account, b, DOT, ETH).using_encoded(<Test as system::Config>::Hashing::hash)
}

/// HELPER FOR INITIALIZING POOLS
fn initialize_pool(asset_a: u32, asset_b: u32, user: u64, amount: u128, price: Price) {
	assert_ok!(XYKPallet::create_pool(
		Origin::signed(user),
		asset_a,
		asset_b,
		amount,
		price
	));

	let shares = if asset_a <= asset_b {
		amount
	} else {
		price.checked_mul_int(amount).unwrap()
	};

	expect_event(xyk::Event::PoolCreated(user, asset_a, asset_b, shares));

	let pair_account = XYKPallet::get_pair_id(AssetPair {
		asset_in: asset_a,
		asset_out: asset_b,
	});
	let share_token = XYKPallet::share_token(pair_account);

	let amount_b = price.saturating_mul_int(amount);

	// Check users state
	assert_eq!(Currency::free_balance(asset_a, &user), ENDOWED_AMOUNT - amount);
	assert_eq!(Currency::free_balance(asset_b, &user), ENDOWED_AMOUNT - amount_b);

	// Check initial state of the pool
	assert_eq!(Currency::free_balance(asset_a, &pair_account), amount);
	assert_eq!(Currency::free_balance(asset_b, &pair_account), amount_b);

	// Check pool shares
	assert_eq!(Currency::free_balance(share_token, &user), shares);

	// Advance blockchain so that we kill old events
	System::initialize(&1, &[0u8; 32].into(), &Default::default(), InitKind::Full);
}

#[test]
fn sell_test_pool_finalization_states() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			20000000000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			1_000_000_000_000,
			4_000_000_000_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Balance should not change yet
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100_000_000_000_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				1000000000000,
				2000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				2000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_b,
				4000000000,
			)
			.into(),
			xyk::Event::SellExecuted(user_2, 3000, 2000, 1000000000000, 1976296910892, 2000, 3960514851).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1003960514851,
				1976296910892,
			)
			.into(),
		]);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1003974296910892);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997996000000000);

		// Check final pool balances
		// TODO: CHECK IF RIGHT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 101000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 198029703089108);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_standard() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			300_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			1_000_000_000_000,
			4_000_000_000_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1003974296910892);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997996000000000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 101000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 198029703089108);

		// TODO: check if final transferred balances add up to initial balance
		// No tokens should be created or lost
		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				1000000000000,
				2000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				2000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_b,
				4000000000,
			)
			.into(),
			xyk::Event::SellExecuted(user_2, 3000, 2000, 1000000000000, 1976296910892, 2000, 3960514851).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1003960514851,
				1976296910892,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_test_inverse_standard() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			4_000_000_000_000,
			1_000_000_000_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances  -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1001996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_001_986_118_811_882);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 996_000_000_000_000);

		// Check final pool balances  -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 99_013_881_188_118);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 202004000000000);

		// TODO: check if final transferred balances add up to initial balance
		// No tokens should be created or lost

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				4_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::SellExecuted(3, 2000, 3000, 2000000000000, 988118811882, 3000, 1980198019).into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				2001980198019,
				988118811882,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				1000000000000,
				2000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				4000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				2000000000,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_test_exact_match() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			1_500_000_000_000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			2_000_000_000_000,
			200_000_000_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_001_996_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_000_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 998_000_000_000_000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100002000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200004000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				2_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				1000000000000,
				2000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				4000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				2000000000,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_test_single_eth_sells() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			2_000_000_000_000,
			200_000_000_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_001_899_942_737_485);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1003913725490197);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 103_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 194_186_331_772_318);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_a,
				asset_b,
				2_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_3,
				asset_a,
				asset_b,
				2000000000000,
				3913725490197,
				asset_b,
				7843137254,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				2007843137254,
				3913725490197,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_2,
				asset_a,
				asset_b,
				1000000000000,
				1899942737485,
				asset_b,
				3807500475,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1003807500475,
				1899942737485,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_test_single_dot_sells() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			2_000_000_000_000,
			200_000_000_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000486767770571);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000988118811882);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 998_000_000_000_000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 98525113417547);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 203_000_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				1_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				2_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_3,
				asset_b,
				asset_a,
				2000000000000,
				988118811882,
				asset_a,
				1980198019,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				2001980198019,
				988118811882,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_2,
				asset_b,
				asset_a,
				1000000000000,
				486767770571,
				asset_a,
				975486514,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1000975486514,
				486767770571,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_trade_limits_respected_for_matched_intention() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));

		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000_000_000_000,
			100_000_000_000_000_000, // Limit set to absurd amount which can't go through
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				1_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolveErrorEvent(
				user_3,
				AssetPair {
					asset_in: asset_b,
					asset_out: asset_a,
				},
				IntentionType::SELL,
				user_3_sell_intention_id,
				DispatchError::Module {
					index: 1,
					error: 2,
					message: None,
				},
			)
			.into(),
			xyk::Event::SellExecuted(
				user_2,
				asset_a,
				asset_b,
				1000000000000,
				1976237623763,
				asset_b,
				3960396039,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1003960396039,
				1976237623763,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_test_single_multiple_sells() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let user_5 = FERDIE;
		let user_6 = GEORGE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);
		assert_ok!(Exchange::sell(
			Origin::signed(user_5),
			asset_b,
			asset_a,
			1_000_000_000_000,
			100_000_000_000,
			false,
		));
		let user_5_sell_intention_id = generate_intention_id(&user_5, 3);
		assert_ok!(Exchange::sell(
			Origin::signed(user_6),
			asset_b,
			asset_a,
			2_000_000_000_000,
			200_000_000_000,
			false,
		));
		let user_6_sell_intention_id = generate_intention_id(&user_6, 4);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 5);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1001996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000499000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 999000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1001991034974081);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100001522538341);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200012965025919);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				1_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::SELL,
				user_4_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_5,
				asset_b,
				asset_a,
				1_000_000_000_000,
				IntentionType::SELL,
				user_5_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_6,
				asset_b,
				asset_a,
				2_000_000_000_000,
				IntentionType::SELL,
				user_6_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_6,
				user_2_sell_intention_id,
				user_6_sell_intention_id,
				1000000000000,
				2000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				4000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_6,
				user_6_sell_intention_id,
				pair_account,
				asset_a,
				2000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_4,
				user_3,
				user_4_sell_intention_id,
				user_3_sell_intention_id,
				500000000000,
				1000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				2000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				1000000000,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_4,
				asset_a,
				asset_b,
				500000000000,
				993034974081,
				asset_b,
				1990050048,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				501990050048,
				993034974081,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_5,
				asset_b,
				asset_a,
				1000000000000,
				501477461659,
				asset_a,
				1004964853,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_5,
				IntentionType::SELL,
				user_5_sell_intention_id,
				1001004964853,
				501477461659,
			)
			.into(),
		]);
	});
}

#[test]
fn sell_test_group_sells() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			200_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			200_000_000_000,
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			200_000_000_000,
			false,
		));
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1002495000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 995000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1019282164364955);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 106008000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 188717835635045);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				5_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				3_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				10_000_000_000_000,
				IntentionType::SELL,
				user_4_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_4,
				user_2,
				user_4_sell_intention_id,
				user_2_sell_intention_id,
				2500000000000,
				5000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				10000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_a,
				5000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_4,
				user_3,
				user_4_sell_intention_id,
				user_3_sell_intention_id,
				1500000000000,
				3000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				6000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				3000000000,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_4,
				asset_a,
				asset_b,
				6000000000000,
				11298164364955,
				asset_b,
				22641611953,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				6022641611953,
				11298164364955,
			)
			.into(),
		]);
	});
}

#[test]
fn trades_without_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Exchange::sell(Origin::signed(ALICE), HDX, ETH, 1000, 200, false),
			Error::<Test>::TokenPoolNotFound
		);

		assert_noop!(
			Exchange::buy(Origin::signed(ALICE), HDX, ETH, 1000, 200, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn trade_min_limit() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Exchange::sell(Origin::signed(ALICE), HDX, ETH, 10, 200, false),
			Error::<Test>::MinimumTradeLimitNotReached
		);

		assert_noop!(
			Exchange::buy(Origin::signed(ALICE), HDX, ETH, 10, 200, false),
			Error::<Test>::MinimumTradeLimitNotReached
		);
	});
}

#[test]
fn sell_more_than_owner_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYKPallet::create_pool(
			Origin::signed(ALICE),
			HDX,
			ETH,
			200_000,
			Price::from(2)
		));

		// With SELL
		assert_noop!(
			Exchange::sell(Origin::signed(ALICE), HDX, ETH, 1000_000_000_000_000u128, 1, false),
			Error::<Test>::InsufficientAssetBalance
		);

		// With BUY
		assert_noop!(
			Exchange::buy(Origin::signed(ALICE), ETH, HDX, 3000_000_000_000_000u128, 1, false),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn sell_test_mixed_buy_sells() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			20_000_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			1400_000_000_000,
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			2000_000_000_000,
			false,
		));
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 996969377448952);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1018630903108671);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111533622551048);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179369096891329);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				5_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				3_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				10_000_000_000_000,
				IntentionType::SELL,
				user_4_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_4,
				user_3,
				user_4_sell_intention_id,
				user_3_sell_intention_id,
				1500000000000,
				3000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				6000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				3000000000,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_4,
				asset_a,
				asset_b,
				8500000000000,
				15636903108671,
				asset_b,
				31336479175,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				8531336479175,
				15636903108671,
			)
			.into(),
			xyk::Event::BuyExecuted(
				user_2,
				asset_b,
				asset_a,
				5000000000000,
				3024573404240,
				asset_a,
				6049146808,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				5000000000000,
				3030622551048,
			)
			.into(),
		]);
	});
}

#[test]
fn discount_tests_no_discount() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			20_000_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			1400_000_000_000,
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			2000_000_000_000,
			false,
		));
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 996969377448952);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1018630903108671);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111533622551048);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179369096891329);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				5_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				3_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				10_000_000_000_000,
				IntentionType::SELL,
				user_4_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_4,
				user_3,
				user_4_sell_intention_id,
				user_3_sell_intention_id,
				1500000000000,
				3000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				6000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				3000000000,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_4,
				asset_a,
				asset_b,
				8500000000000,
				15636903108671,
				asset_b,
				31336479175,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				8531336479175,
				15636903108671,
			)
			.into(),
			xyk::Event::BuyExecuted(
				user_2,
				asset_b,
				asset_a,
				5000000000000,
				3024573404240,
				asset_a,
				6049146808,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				5000000000000,
				3030622551048,
			)
			.into(),
		]);
	});
}

#[test]
fn discount_tests_with_discount() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);
		initialize_pool(asset_a, HDX, user_2, pool_amount, initial_price);
		initialize_pool(asset_b, HDX, user_3, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			20_000_000_000_000,
			true,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			1400_000_000_000,
			true,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			2000_000_000_000,
			true,
		));
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 896972965651836);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 897000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1018651271820135);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111530034348164);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179348728179865);

		assert_eq!(Currency::free_balance(HDX, &user_4), 999978064464578);
		assert_eq!(Currency::free_balance(HDX, &user_2), 799995765116332);
		assert_eq!(Currency::free_balance(HDX, &user_3), 800000000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				5_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				3_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				10_000_000_000_000,
				IntentionType::SELL,
				user_4_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_4,
				user_3,
				user_4_sell_intention_id,
				user_3_sell_intention_id,
				1500000000000,
				3000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				6000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				3000000000,
			)
			.into(),
			xyk::Event::SellExecuted(
				user_4,
				asset_a,
				asset_b,
				8500000000000,
				15657271820135,
				asset_b,
				10967767711,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				8510967767711,
				15657271820135,
			)
			.into(),
			xyk::Event::BuyExecuted(
				user_2,
				asset_b,
				asset_a,
				5000000000000,
				3024916906330,
				asset_a,
				2117441834,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				5000000000000,
				3027034348164,
			)
			.into(),
		]);
	});
}

#[test]
fn buy_test_exact_match() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			4_000_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			2_000_000_000_000,
			4_000_000_000_000,
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1001000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 997996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 998998000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1002000000000000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100002000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200004000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				1_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				2_000_000_000_000,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_3,
				user_2,
				user_3_sell_intention_id,
				user_2_sell_intention_id,
				1000000000000,
				2000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				2000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				4000000000,
			)
			.into(),
		]);
	});
}

#[test]
fn buy_test_group_buys() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			20_000_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			20_000_000_000_000,
			false,
		));
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		assert_ok!(Exchange::buy(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			22_000_000_000_000,
			false,
		));
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 997495000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 998696090255837);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1003000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 1010000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 978741351351351);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 93808909744163);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 213258648648649);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				5_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				3_000_000_000_000,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				10_000_000_000_000,
				IntentionType::BUY,
				user_4_sell_intention_id,
			)
			.into(),
			xyk::Event::BuyExecuted(
				user_4,
				asset_a,
				asset_b,
				7500000000000,
				16216216216217,
				asset_b,
				32432432432,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::BUY,
				user_4_sell_intention_id,
				7500000000000,
				16248648648649,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_4,
				user_2_sell_intention_id,
				user_4_sell_intention_id,
				2500000000000,
				5000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_a,
				5000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_4,
				user_4_sell_intention_id,
				pair_account,
				asset_b,
				10000000000,
			)
			.into(),
			xyk::Event::BuyExecuted(
				user_3,
				asset_b,
				asset_a,
				3000000000000,
				1301307129904,
				asset_a,
				2602614259,
			)
			.into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::BUY,
				user_3_sell_intention_id,
				3000000000000,
				1303909744163,
			)
			.into(),
		]);
	});
}

#[test]
fn discount_tests_with_error() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			20_000_000_000_000,
			true,
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			20_000_000_000_000,
			true,
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			20_000_000_000_000,
			true,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);
		let user_4_sell_intention_id = generate_intention_id(&user_4, 2);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1000000000000000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000000000);

		assert_eq!(Currency::free_balance(HDX, &user_4), 1000000000000000);
		assert_eq!(Currency::free_balance(HDX, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(HDX, &user_3), 1000000000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_b,
				asset_a,
				5_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				3_000_000_000_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_4,
				asset_a,
				asset_b,
				10_000_000_000_000,
				IntentionType::SELL,
				user_4_sell_intention_id,
			)
			.into(),
			Event::IntentionResolveErrorEvent(
				user_4,
				AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				},
				IntentionType::SELL,
				user_4_sell_intention_id,
				DispatchError::Module {
					index: 2,
					error: 19,
					message: None,
				},
			)
			.into(),
			Event::IntentionResolveErrorEvent(
				user_2,
				AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				},
				IntentionType::BUY,
				user_2_sell_intention_id,
				DispatchError::Module {
					index: 2,
					error: 19,
					message: None,
				},
			)
			.into(),
			Event::IntentionResolveErrorEvent(
				user_3,
				AssetPair {
					asset_in: asset_b,
					asset_out: asset_a,
				},
				IntentionType::SELL,
				user_3_sell_intention_id,
				DispatchError::Module {
					index: 2,
					error: 19,
					message: None,
				},
			)
			.into(),
		]);
	});
}

#[test]
fn simple_sell_sell() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000,
			400,
			false,
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000,
			400,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999998000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000003992);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000499);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999999000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100001501);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 199997008);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				1_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				500,
				1000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(user_2, user_2_sell_intention_id, pair_account, asset_b, 2).into(),
			Event::IntentionResolvedDirectTradeFees(user_3, user_3_sell_intention_id, pair_account, asset_a, 1).into(),
			xyk::Event::SellExecuted(2, 3000, 2000, 1500, 2994, 2000, 6).into(),
			Event::IntentionResolvedAMMTrade(user_2, IntentionType::SELL, user_2_sell_intention_id, 1506, 2994).into(),
		]);
	});
}

#[test]
fn simple_buy_buy() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000,
			5000,
			false,
		));
		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000,
			5000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000002000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999999999995991);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 999999999999499);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000000000001000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 99998501);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200003009);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				1_000,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::BuyExecuted(2, 3000, 2000, 1500, 3001, 2000, 6).into(),
			Event::IntentionResolvedAMMTrade(user_2, IntentionType::BUY, user_2_sell_intention_id, 1500, 3007).into(),
			Event::IntentionResolvedDirectTrade(
				user_3,
				user_2,
				user_3_sell_intention_id,
				user_2_sell_intention_id,
				500,
				1000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(user_3, user_3_sell_intention_id, pair_account, asset_a, 1).into(),
			Event::IntentionResolvedDirectTradeFees(user_2, user_2_sell_intention_id, pair_account, asset_b, 2).into(),
		]);
	});
}

#[test]
fn simple_sell_buy() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000,
			400,
			false,
		));
		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			1_000,
			2_000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999998000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000003994);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000001000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999997996);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100001000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 199998010);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_a,
				asset_b,
				1_000,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				1000,
				2000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(user_2, user_2_sell_intention_id, pair_account, asset_b, 2).into(),
			Event::IntentionResolvedDirectTradeFees(user_3, user_3_sell_intention_id, pair_account, asset_b, 4).into(),
			xyk::Event::SellExecuted(2, 3000, 2000, 1000, 1996, 2000, 4).into(),
			Event::IntentionResolvedAMMTrade(user_2, IntentionType::SELL, user_2_sell_intention_id, 1004, 1996).into(),
		]);
	});
}

#[test]
fn simple_buy_sell() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000,
			5000,
			false,
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			1_000,
			1500,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);
		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000002000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999999999995991);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 999999999999000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000000000001998);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 99999000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200002011);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_a,
				asset_b,
				1_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::BuyExecuted(user_2, 3000, 2000, 1000, 2001, 2000, 4).into(),
			Event::IntentionResolvedAMMTrade(user_2, IntentionType::BUY, user_2_sell_intention_id, 1000, 2005).into(),
			Event::IntentionResolvedDirectTrade(
				user_3,
				user_2,
				user_3_sell_intention_id,
				user_2_sell_intention_id,
				1000,
				2000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(user_3, user_3_sell_intention_id, pair_account, asset_b, 2).into(),
			Event::IntentionResolvedDirectTradeFees(user_2, user_2_sell_intention_id, pair_account, asset_b, 4).into(),
		]);
	});
}

#[test]
fn single_sell_intention_test() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			400_000_000_000,
			false,
		));
		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 1);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1003913725490197);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 102000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 196086274509803);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000_000_000_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			xyk::Event::SellExecuted(2, 3000, 2000, 2000000000000, 3913725490197, 2000, 7843137254).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				2007843137254,
				3913725490197,
			)
			.into(),
		]);
	});
}

#[test]
fn single_buy_intention_test() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			15000_000_000_000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 1);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1002000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 995910204081632);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 98000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 204089795918368);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000_000_000_000,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			xyk::Event::BuyExecuted(2, 3000, 2000, 2000000000000, 4081632653062, 2000, 8163265306).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				2000000000000,
				4089795918368,
			)
			.into(),
		]);
	});
}

#[test]
fn simple_sell_sell_with_error_should_not_pass() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000,
			5_000,
			false,
		));

		let user_2_sell_intention_id = generate_intention_id(&user_2, 0);

		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000,
			5_000,
			false,
		));

		let user_3_sell_intention_id = generate_intention_id(&user_3, 1);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				2_000,
				IntentionType::SELL,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				1_000,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolveErrorEvent(
				user_2,
				AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				},
				IntentionType::SELL,
				user_2_sell_intention_id,
				DispatchError::Module {
					index: 2,
					error: 8,
					message: None,
				},
			)
			.into(),
			Event::IntentionResolveErrorEvent(
				user_3,
				AssetPair {
					asset_in: asset_b,
					asset_out: asset_a,
				},
				IntentionType::SELL,
				user_3_sell_intention_id,
				DispatchError::Module {
					index: 2,
					error: 8,
					message: None,
				},
			)
			.into(),
		]);
	});
}
