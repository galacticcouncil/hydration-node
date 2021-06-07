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
	Currency, Event as TestEvent, Exchange, ExtBuilder, Origin, System, Test, ALICE, BOB, CHARLIE, DAVE, DOT,
	ENDOWED_AMOUNT, ETH, FERDIE, GEORGE, HDX, XYK as XYKPallet,
};
use frame_support::sp_runtime::traits::Hash;
use frame_support::sp_runtime::FixedPointNumber;
use frame_support::traits::OnFinalize;
use frame_support::{assert_noop, assert_ok};
use frame_system::InitKind;
use primitives::Price;
use sp_runtime::DispatchError;

use pallet_xyk as xyk;

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
		assert_eq!(Currency::free_balance(asset_a, &user_2), ENDOWED_AMOUNT);
		assert_eq!(Currency::free_balance(asset_b, &user_2), ENDOWED_AMOUNT);

		assert_eq!(Currency::free_balance(asset_a, &user_3), ENDOWED_AMOUNT);
		assert_eq!(Currency::free_balance(asset_b, &user_3), ENDOWED_AMOUNT);

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
			xyk::Event::SellExecuted(user_2, 3000, 2000, 1000000000000, 1976336046259).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1000000000000,
				1976336046259,
			)
			.into(),
		]);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_003974336046259);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_997996000000000);

		// Check final pool balances
		// TODO: CHECK IF RIGHT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 101000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 198029663953741);

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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_003974336046259);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_997996000000000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 101000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 198029663953741);

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
			xyk::Event::SellExecuted(user_2, 3000, 2000, 1000000000000, 1976336046259).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1000000000000,
				1976336046259,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_001996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001_986_138_378_978);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_996_000_000_000_000);

		// Check final pool balances  -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 99_013_861_621_022);
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
			xyk::Event::SellExecuted(3, 2000, 3000, 2000000000000, 988138378978).into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				2000000000000,
				988138378978,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_001_996_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_998_000_000_000_000);

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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_001_899_978_143_094);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 99_998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_003913878975647);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 103_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 194_186_142_881_259);

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
			xyk::Event::SellExecuted(user_3, asset_a, asset_b, 2000000000000, 3913878975647).into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				2000000000000,
				3913878975647,
			)
			.into(),
			xyk::Event::SellExecuted(user_2, asset_a, asset_b, 1000000000000, 1899978143094).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1000000000000,
				1899978143094,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000486772470162);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_999_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000988138378978);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_998_000_000_000_000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 98525089150860);
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
			xyk::Event::SellExecuted(user_3, asset_b, asset_a, 2000000000000, 988138378978).into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				2000000000000,
				988138378978,
			)
			.into(),
			xyk::Event::SellExecuted(user_2, asset_b, asset_a, 1000000000000, 486772470162).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1000000000000,
				486772470162,
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
			xyk::Event::SellExecuted(user_2, asset_a, asset_b, 1000000000000, 1976276757956).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				1000000000000,
				1976276757956,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_999000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_001996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000499000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_999000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 99_999000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 100_001991044854829);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100001517499067);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200012955145171);

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
			xyk::Event::SellExecuted(user_4, asset_a, asset_b, 500000000000, 993044854829).into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				5_000_000_000_00,
				993044854829,
			)
			.into(),
			xyk::Event::SellExecuted(user_5, asset_b, asset_a, 1000000000000, 501482500933).into(),
			Event::IntentionResolvedAMMTrade(
				user_5,
				IntentionType::SELL,
				user_5_sell_intention_id,
				1000000000000,
				501482500933,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_002495000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_995000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 99_990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 100_019283443450697);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 106008000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 188716556549303);

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
			xyk::Event::SellExecuted(user_4, asset_a, asset_b, 6000000000000, 11299443450697).into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				6000000000000,
				11299443450697,
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
			Exchange::sell(Origin::signed(ALICE), HDX, ETH, 10 * ENDOWED_AMOUNT, 1, false),
			Error::<Test>::InsufficientAssetBalance
		);

		// With BUY
		assert_noop!(
			Exchange::buy(Origin::signed(ALICE), ETH, HDX, 10 * ENDOWED_AMOUNT, 1, false),
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_996969336047446);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 99_990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 100_018633353446528);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111533663952554);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179366646553472);

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
			xyk::Event::SellExecuted(user_4, asset_a, asset_b, 8500000000000, 15639353446528).into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				8500000000000,
				15639353446528,
			)
			.into(),
			xyk::Event::BuyExecuted(user_2, asset_b, asset_a, 5000000000000, 3030663952554).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				5000000000000,
				3030663952554,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_996969336047446);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 99_990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 100_018633353446528);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111533663952554);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179366646553472);

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
			xyk::Event::SellExecuted(user_4, asset_a, asset_b, 8500000000000, 15639353446528).into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				8500000000000,
				15639353446528,
			)
			.into(),
			xyk::Event::BuyExecuted(user_2, asset_b, asset_a, 5000000000000, 3030663952554).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				5000000000000,
				3030663952554,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_896972951159572);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_001497000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_897000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 99_990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 100_018652130468064);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111530048840428);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179347869531936);

		assert_eq!(Currency::free_balance(HDX, &user_4), 99_999988100000000);
		assert_eq!(Currency::free_balance(HDX, &user_2), 99_799995765096058);
		assert_eq!(Currency::free_balance(HDX, &user_3), 99_800000000000000);

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
			xyk::Event::SellExecuted(user_4, asset_a, asset_b, 8500000000000, 15658130468064).into(),
			Event::IntentionResolvedAMMTrade(
				user_4,
				IntentionType::SELL,
				user_4_sell_intention_id,
				8500000000000,
				15658130468064,
			)
			.into(),
			xyk::Event::BuyExecuted(user_2, asset_b, asset_a, 5000000000000, 3027048840428).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::BUY,
				user_2_sell_intention_id,
				5000000000000,
				3027048840428,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_001000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_997996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 99_998998000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_002000000000000);

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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_997495000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 99_998696090255837);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_003000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 100_010000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 99_978741351351351);

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
			xyk::Event::BuyExecuted(user_4, asset_a, asset_b, 7500000000000, 16248648648649).into(),
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
			xyk::Event::BuyExecuted(user_3, asset_b, asset_a, 3000000000000, 1303909744163).into(),
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 100_000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 100_000000000000000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200000000000000);

		assert_eq!(Currency::free_balance(HDX, &user_4), ENDOWED_AMOUNT);
		assert_eq!(Currency::free_balance(HDX, &user_2), ENDOWED_AMOUNT);
		assert_eq!(Currency::free_balance(HDX, &user_3), ENDOWED_AMOUNT);

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

		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_999999999998000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000000000003992);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000000000000499);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_999999999999000);

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
			xyk::Event::SellExecuted(2, 3000, 2000, 1500, 2994).into(),
			Event::IntentionResolvedAMMTrade(user_2, IntentionType::SELL, user_2_sell_intention_id, 1500, 2994).into(),
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

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000000000002000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_999999999995991);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 99_999999999999499);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_000000000001000);

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
			xyk::Event::BuyExecuted(2, 3000, 2000, 1500, 3007).into(),
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

		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_999999999998000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000000000003994);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000000000001000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 99_999999999997996);

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
			xyk::Event::SellExecuted(2, 3000, 2000, 1000, 1996).into(),
			Event::IntentionResolvedAMMTrade(user_2, IntentionType::SELL, user_2_sell_intention_id, 1000, 1996).into(),
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

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000000000002000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_999999999995991);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 99_999999999999000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_000000000001998);

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
			xyk::Event::BuyExecuted(user_2, 3000, 2000, 1000, 2005).into(),
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 99_998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_003913878975647);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 102000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 196086121024353);

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
			xyk::Event::SellExecuted(2, 3000, 2000, 2000000000000, 3913878975647).into(),
			Event::IntentionResolvedAMMTrade(
				user_2,
				IntentionType::SELL,
				user_2_sell_intention_id,
				2000000000000,
				3913878975647,
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
		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_002000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_995910204081632);

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
			xyk::Event::BuyExecuted(2, 3000, 2000, 2000000000000, 4089795918368).into(),
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

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 100_000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_000000000000000);

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

#[test]
fn matching_limits_buy_buy_should_work() {
	new_test_ext().execute_with(|| {
		let one: Balance = 1_000_000_000_000;
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = HDX;
		let asset_b = DOT;
		let pool_amount = 1000 * one;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1_000 * one);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2_000 * one);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000 * one);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000 * one);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			100_000_000_000_000,
			223333333333334,
			false,
		));

		let b = <system::Pallet<Test>>::current_block_number();
		let user_2_sell_intention_id = (0, &user_2, b, HDX, DOT).using_encoded(<Test as system::Config>::Hashing::hash);

		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			220 * one,
			124213483146068,
			false,
		));

		let user_3_sell_intention_id = (1, &user_3, b, HDX, DOT).using_encoded(<Test as system::Config>::Hashing::hash);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_100 * one);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_799_600_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1010321212121213);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1980400000000000);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				100 * one,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				220 * one,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::BuyExecuted(3, asset_b, asset_a, 20_000_000_000_000, 10121212121213).into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::BUY,
				user_3_sell_intention_id,
				20_000_000_000_000,
				10121212121213,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				200000000000000,
				100000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				400000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				200000000000,
			)
			.into(),
		]);
	});
}

#[test]
fn matching_limits_sell_buy_should_work() {
	new_test_ext().execute_with(|| {
		let one: Balance = 1_000_000_000_000;
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = HDX;
		let asset_b = DOT;
		let pool_amount = 1000 * one;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1_000 * one);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2_000 * one);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000 * one);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000 * one);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			30_000_000_000_000,
			62164948453608,
			false,
		));

		let b = <system::Pallet<Test>>::current_block_number();
		let user_2_sell_intention_id = (0, &user_2, b, HDX, DOT).using_encoded(<Test as system::Config>::Hashing::hash);

		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			50 * one,
			94761904761906,
			false,
		));

		let user_3_sell_intention_id = (1, &user_3, b, HDX, DOT).using_encoded(<Test as system::Config>::Hashing::hash);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_030 * one);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_939_880_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1020000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1961041210243538);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				30 * one,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_a,
				asset_b,
				50 * one,
				IntentionType::SELL,
				user_3_sell_intention_id,
			)
			.into(),
			xyk::Event::SellExecuted(3, asset_a, asset_b, 20_000_000_000_000, 39138789756462).into(),
			Event::IntentionResolvedAMMTrade(
				user_3,
				IntentionType::SELL,
				user_3_sell_intention_id,
				20_000_000_000_000,
				39138789756462,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				60000000000000,
				30000000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				120000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_b,
				60000000000,
			)
			.into(),
		]);
	});
}

#[test]
fn exact_match_limit_should_work() {
	new_test_ext().execute_with(|| {
		let one: Balance = 1_000_000_000_000;
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = HDX;
		let asset_b = DOT;
		let pool_amount = 1000 * one;
		let initial_price = Price::from(2);

		let pair_account = XYKPallet::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1_000 * one);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2_000 * one);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_000 * one);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 100_000 * one);

		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			50_000_000_000_000,
			106_315_789_473_684,
			false,
		));

		let b = <system::Pallet<Test>>::current_block_number();
		let user_2_sell_intention_id = (0, &user_2, b, HDX, DOT).using_encoded(<Test as system::Config>::Hashing::hash);

		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			100_000_000_000_000,
			53_157_894_736_843,
			false,
		));

		let user_3_sell_intention_id = (1, &user_3, b, HDX, DOT).using_encoded(<Test as system::Config>::Hashing::hash);

		<Exchange as OnFinalize<u64>>::on_finalize(9);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 100_050_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 99_899_800_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 99_949_900_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 100_100_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1_000_100_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2_000_200_000_000_000);

		expect_events(vec![
			Event::IntentionRegistered(
				user_2,
				asset_a,
				asset_b,
				50 * one,
				IntentionType::BUY,
				user_2_sell_intention_id,
			)
			.into(),
			Event::IntentionRegistered(
				user_3,
				asset_b,
				asset_a,
				100 * one,
				IntentionType::BUY,
				user_3_sell_intention_id,
			)
			.into(),
			Event::IntentionResolvedDirectTrade(
				user_2,
				user_3,
				user_2_sell_intention_id,
				user_3_sell_intention_id,
				100 * one,
				50 * one,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_2,
				user_2_sell_intention_id,
				pair_account,
				asset_b,
				200000000000,
			)
			.into(),
			Event::IntentionResolvedDirectTradeFees(
				user_3,
				user_3_sell_intention_id,
				pair_account,
				asset_a,
				100000000000,
			)
			.into(),
		]);
	});
}
