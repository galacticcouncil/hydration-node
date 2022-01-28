// This file is part of Basilisk-node.

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
	Currency, Event as TestEvent, ExtBuilder, Origin, System, Test, ACA, ALICE, BOB, DOT, HDX, HDX_DOT_POOL_ID, XYK,
};
use frame_support::BoundedVec;
use frame_support::{assert_noop, assert_ok};
use hydra_dx_math::MathError;
use hydradx_traits::AMM as AmmPool;
use pallet_asset_registry::AssetType;
use sp_std::convert::TryInto;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn expect_events(e: Vec<TestEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

#[test]
fn create_pool_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000_000,
			Price::from(10)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(XYK::get_pool_assets(&pair_account), Some(vec![asset_a, asset_b]));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &ALICE), 900000000000000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 0);
		assert_eq!(Currency::free_balance(share_token, &ALICE), 100000000000000);
		assert_eq!(XYK::total_liquidity(&pair_account), 100000000000000);

		expect_events(vec![Event::PoolCreated(
			ALICE,
			asset_a,
			asset_b,
			100000000000000,
			share_token,
			pair_account,
		)
		.into()]);
	});
}

#[test]
fn create_same_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = ACA;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_b,
			asset_a,
			1000,
			Price::from(2)
		));
		assert_noop!(
			XYK::create_pool(Origin::signed(user), asset_b, asset_a, 999, Price::from(2)),
			Error::<Test>::InsufficientLiquidity
		);
		assert_noop!(
			XYK::create_pool(Origin::signed(user), asset_b, asset_a, 1000, Price::from(0)),
			Error::<Test>::ZeroInitialPrice
		);
		assert_noop!(
			XYK::create_pool(Origin::signed(user), asset_a, asset_a, 1000, Price::from(2)),
			Error::<Test>::CannotCreatePoolWithSameAssets
		);
		assert_noop!(
			XYK::create_pool(Origin::signed(user), asset_b, asset_a, 1000, Price::from(2)),
			Error::<Test>::TokenPoolAlreadyExists
		);

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		expect_events(vec![Event::PoolCreated(
			ALICE,
			asset_b,
			asset_a,
			2000,
			share_token,
			pair_account,
		)
		.into()]);
	});
}

#[test]
fn create_pool_overflowing_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = ACA;

		assert_noop!(
			XYK::create_pool(
				Origin::signed(user),
				asset_b,
				asset_a,
				u128::MAX as u128,
				Price::from(2)
			),
			Error::<Test>::CreatePoolAssetAmountInvalid
		);
	});
}

#[test]
fn create_pool_with_insufficient_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;

		assert_noop!(
			XYK::create_pool(
				Origin::signed(user),
				4000,
				asset_a,
				100_000_000_000_000,
				Price::from(10)
			),
			Error::<Test>::InsufficientAssetBalance
		);

		assert_noop!(
			XYK::create_pool(
				Origin::signed(user),
				asset_a,
				4000,
				100_000_000_000_000,
				Price::from(10)
			),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn add_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = DOT;
		let asset_b = HDX;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		assert_ok!(XYK::add_liquidity(
			Origin::signed(user),
			asset_a,
			asset_b,
			400_000,
			1_000_000_000_000
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1004000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999899600000);
		assert_eq!(Currency::free_balance(share_token, &user), 1004000000000);
		assert_eq!(XYK::total_liquidity(&pair_account), 1004000000000);

		expect_events(vec![
			Event::PoolCreated(ALICE, asset_a, asset_b, 1000000000000, share_token, pair_account).into(),
			Event::LiquidityAdded(ALICE, asset_a, asset_b, 400000, 4000000000).into(),
		]);
	});
}

#[test]
fn add_liquidity_as_another_user_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_b,
			asset_a,
			100_000_000,
			Price::from(10_000)
		));
		assert_ok!(XYK::add_liquidity(
			Origin::signed(user),
			asset_b,
			asset_a,
			400_000,
			1_000_000_000_000
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1004000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_b, &user), 999999899600000);
		assert_eq!(Currency::free_balance(share_token, &user), 1004000000000);
		assert_eq!(XYK::total_liquidity(&pair_account), 1004000000000);

		assert_ok!(XYK::add_liquidity(
			Origin::signed(BOB),
			asset_b,
			asset_a,
			1_000_000,
			1_000_000_000_000
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1014000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 101400000);
		assert_eq!(Currency::free_balance(asset_b, &user), 999999899600000);
		assert_eq!(Currency::free_balance(asset_b, &BOB), 999999999000000);
		assert_eq!(Currency::free_balance(share_token, &user), 1004000000000);
		assert_eq!(Currency::free_balance(share_token, &BOB), 10000000000);
		assert_eq!(XYK::total_liquidity(&pair_account), 1014000000000);

		expect_events(vec![
			Event::PoolCreated(ALICE, asset_b, asset_a, 1000000000000, share_token, pair_account).into(),
			Event::LiquidityAdded(ALICE, asset_b, asset_a, 400000, 4000000000).into(),
			orml_tokens::Event::Endowed {
				currency_id: 0,
				who: 2,
				amount: 10000000000,
			}
			.into(),
			Event::LiquidityAdded(BOB, asset_b, asset_a, 1000000, 10000000000).into(),
		]);
	});
}

#[test]
fn remove_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(share_token, &user), 100000000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999900000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000000);

		assert_ok!(XYK::remove_liquidity(Origin::signed(user), asset_a, asset_b, 355_000));

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 996450000000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999900355000);

		assert_eq!(Currency::free_balance(share_token, &user), 99645000);
		assert_eq!(XYK::total_liquidity(&pair_account), 99645000);

		expect_events(vec![
			Event::PoolCreated(ALICE, asset_a, asset_b, 100000000, share_token, pair_account).into(),
			Event::LiquidityRemoved(ALICE, asset_a, asset_b, 355_000).into(),
		]);
	});
}

#[test]
fn remove_liquidity_without_shares_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(1)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);
		let shares = Currency::free_balance(share_token, &user);

		assert_ok!(Currency::transfer(Origin::signed(ALICE), BOB, share_token, shares));

		assert_noop!(
			XYK::remove_liquidity(Origin::signed(user), asset_a, asset_b, 355_000),
			Error::<Test>::InsufficientAssetBalance
		);

		expect_events(vec![
			Event::PoolCreated(ALICE, asset_a, asset_b, 100000000, share_token, pair_account).into(),
			orml_tokens::Event::Endowed {
				currency_id: share_token,
				who: BOB,
				amount: shares,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: share_token,
				from: ALICE,
				to: BOB,
				amount: shares,
			}
			.into(),
		]);
	});
}

// events in the following test do not occur during standard chain operation
#[test]
fn remove_liquidity_from_reduced_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(1)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		// remove some amount from the pool
		assert_ok!(Currency::transfer(
			Origin::signed(pair_account),
			BOB,
			asset_a,
			90_000_000
		));

		assert_noop!(
			XYK::remove_liquidity(Origin::signed(user), asset_a, asset_b, 200_000_000),
			Error::<Test>::InsufficientLiquidity
		);

		// return it back to the pool
		assert_ok!(Currency::transfer(
			Origin::signed(BOB),
			pair_account,
			asset_a,
			90_000_000
		));
		// do it again with asset_b
		assert_ok!(Currency::transfer(
			Origin::signed(pair_account),
			BOB,
			asset_b,
			90_000_000
		));

		assert_noop!(
			XYK::remove_liquidity(Origin::signed(user), asset_a, asset_b, 200_000_000),
			Error::<Test>::InsufficientLiquidity
		);

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		expect_events(vec![
			Event::PoolCreated(ALICE, asset_a, asset_b, 100000000, share_token, pair_account).into(),
			orml_tokens::Event::Transfer {
				currency_id: asset_a,
				from: pair_account,
				to: BOB,
				amount: 90_000_000,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: asset_a,
				from: BOB,
				to: pair_account,
				amount: 90_000_000,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: asset_b,
				from: pair_account,
				to: BOB,
				amount: 90_000_000,
			}
			.into(),
		]);
	});
}

#[test]
fn add_liquidity_more_than_owner_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			HDX,
			ACA,
			200_000_000,
			Price::from(3000000)
		));

		assert_eq!(Currency::free_balance(ACA, &ALICE), 400000000000000);

		assert_noop!(
			XYK::add_liquidity(Origin::signed(ALICE), HDX, ACA, 200_000_000_000_000_000, 600_000_000),
			Error::<Test>::InsufficientAssetBalance
		);

		assert_noop!(
			XYK::add_liquidity(Origin::signed(ALICE), HDX, ACA, 600_000_000, 200_000_000_000_000_000),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn add_insufficient_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			HDX,
			ACA,
			1000,
			Price::from_float(1.5)
		));

		assert_noop!(
			XYK::add_liquidity(Origin::signed(ALICE), HDX, ACA, 0, 0),
			Error::<Test>::InsufficientTradingAmount
		);

		assert_noop!(
			XYK::add_liquidity(Origin::signed(ALICE), HDX, ACA, 1000, 0),
			Error::<Test>::ZeroLiquidity
		);

		assert_noop!(
			XYK::add_liquidity(Origin::signed(BOB), ACA, HDX, 1000, 2000),
			Error::<Test>::InsufficientLiquidity
		);
	});
}

#[test]
fn add_liquidity_exceeding_max_limit_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			HDX,
			ACA,
			100_000_000_000_000,
			Price::from(1)
		));

		assert_noop!(
			XYK::add_liquidity(Origin::signed(ALICE), HDX, ACA, 10_000_000, 1_000_000),
			Error::<Test>::AssetAmountExceededLimit
		);
	});
}
#[test]
fn remove_liquidity_should_respect_min_pool_limit() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			HDX,
			ACA,
			1000,
			Price::from_float(1.5)
		));

		assert_ok!(XYK::add_liquidity(Origin::signed(BOB), ACA, HDX, 2000, 2000));

		assert_noop!(
			XYK::remove_liquidity(Origin::signed(BOB), ACA, HDX, 500),
			Error::<Test>::InsufficientLiquidity
		);
	});
}

#[test]
fn remove_zero_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::remove_liquidity(Origin::signed(ALICE), HDX, ACA, 0),
			Error::<Test>::ZeroLiquidity
		);
	});
}

#[test]
fn sell_test() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_ok!(XYK::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			456_444_678,
			1000000000000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999799543555322);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 401363483591788);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200456444678);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 598636516408212);

		expect_events(vec![
			Event::PoolCreated(ALICE, asset_a, asset_b, 600000000000000, share_token, pair_account).into(),
			Event::SellExecuted(
				ALICE,
				asset_a,
				asset_b,
				456444678,
				1363483591788,
				asset_b,
				2732432046,
				pair_account,
			)
			.into(),
		]);
	});
}

#[test]
fn work_flow_happy_path_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = HDX;
		let asset_b = ACA;

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		// Check initial balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 0);

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			350_000_000_000,
			Price::from(40)
		));

		// User 1 really tries!
		assert_noop!(
			XYK::add_liquidity(Origin::signed(user_1), asset_a, asset_b, 800_000_000_000_000_000, 100),
			Error::<Test>::InsufficientAssetBalance
		);

		// Total liquidity
		assert_eq!(XYK::total_liquidity(&pair_account), 350_000_000_000);

		let share_token = XYK::share_token(pair_account);

		// Check balance after add liquidity for user 1 and user 2

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_000_000_000_000_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 0);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 350_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 14_000_000_000_000);

		// User 2 adds liquidity
		let current_b_balance = Currency::free_balance(asset_b, &user_2);
		assert_ok!(XYK::add_liquidity(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			300_000_000_000,
			current_b_balance
		));

		assert_eq!(XYK::total_liquidity(&pair_account), 650_000_000_000);

		// Check balance after add liquidity for user 1 and user 2
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_700_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 988_000_000_000_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 300_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 26_000_000_000_000);

		// User 2 SELLs
		assert_ok!(XYK::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			216_666_666_666,
			100_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_483_333_333_334);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 994_486_999_999_986);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 300_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 866_666_666_666);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 19_513_000_000_014);

		// User 1 SELLs
		assert_ok!(XYK::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			288_888_888_888,
			100_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_361_111_111_112);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 990_868_493_499_997);

		let user_2_original_balance_1 = Currency::free_balance(asset_a, &user_2);
		let user_2_original_balance_2 = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_2_original_balance_1, 999_483_333_333_334);
		assert_eq!(user_2_original_balance_2, 994_486_999_999_986);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 300_000_000_000);

		// User 2 removes liquidity

		assert_ok!(XYK::remove_liquidity(Origin::signed(user_2), asset_a, asset_b, 10_000));

		let user_2_remove_1_balance_1 = Currency::free_balance(asset_a, &user_2);
		let user_2_remove_1_balance_2 = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_2_remove_1_balance_1, 999_483_333_351_111);
		assert_eq!(user_2_remove_1_balance_2, 994_487_000_225_286);
		assert_eq!(Currency::free_balance(share_token, &user_2), 299_999_990_000);

		assert_ok!(XYK::remove_liquidity(Origin::signed(user_2), asset_b, asset_a, 10_000));

		let user_2_remove_2_balance_1 = Currency::free_balance(asset_a, &user_2);
		let user_2_remove_2_balance_2 = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_2_remove_2_balance_1, 999_483_333_368_888);
		assert_eq!(user_2_remove_2_balance_2, 994_487_000_450_586);
		assert_eq!(Currency::free_balance(share_token, &user_2), 299_999_980_000);

		// The two removes should be equal (this could slip by 1 because of rounding error)

		assert_eq!(
			user_2_remove_1_balance_1 - user_2_original_balance_1,
			user_2_remove_2_balance_1 - user_2_remove_1_balance_1
		);

		assert_eq!(
			user_2_remove_1_balance_2 - user_2_original_balance_2,
			user_2_remove_2_balance_2 - user_2_remove_1_balance_2
		);

		assert_eq!(XYK::total_liquidity(&pair_account), 649_999_980_000);

		assert_ok!(XYK::remove_liquidity(Origin::signed(user_2), asset_a, asset_b, 18_000));
		assert_eq!(Currency::free_balance(share_token, &user_2), 299_999_962_000);

		assert_eq!(XYK::total_liquidity(&pair_account), 649_999_962_000);

		expect_events(vec![
			Event::PoolCreated(user_1, asset_a, asset_b, 350_000_000_000, share_token, pair_account).into(),
			orml_tokens::Event::Endowed {
				currency_id: 0,
				who: 2,
				amount: 300000000000,
			}
			.into(),
			Event::LiquidityAdded(user_2, asset_a, asset_b, 300_000_000_000, 12_000_000_000_000).into(),
			Event::SellExecuted(
				user_2,
				asset_a,
				asset_b,
				216_666_666_666,
				6_486_999_999_986,
				asset_b,
				12_999_999_998,
				pair_account,
			)
			.into(),
			Event::SellExecuted(
				ALICE,
				asset_a,
				asset_b,
				288_888_888_888,
				4_868_493_499_997,
				asset_b,
				9_756_499_998,
				pair_account,
			)
			.into(),
			Event::LiquidityRemoved(user_2, asset_a, asset_b, 10_000).into(),
			Event::LiquidityRemoved(user_2, asset_b, asset_a, 10_000).into(),
			Event::LiquidityRemoved(user_2, asset_a, asset_b, 18_000).into(),
		]);
	});
}

#[test]
fn sell_with_correct_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1_000_000_000_000_000u128),
		(BOB, HDX, 1_000_000_000_000_000u128),
		(ALICE, ACA, 1_000_000_000_000_000u128),
		(BOB, ACA, 1_000_000_000_000_000u128),
		(ALICE, DOT, 1_000_000_000_000_000u128),
		(BOB, DOT, 1_000_000_000_000_000u128),
	];

	let mut ext: sp_io::TestExternalities = ExtBuilder::default().with_accounts(accounts).build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = ACA;
		let asset_b = HDX;

		// Verify initial balances
		assert_eq!(Currency::free_balance(asset_a, &user_1), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1_000_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_b, &user_1), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_000_000_000_000_000);

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			10_000_000,
			Price::from(200)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999990000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999998000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 10000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2000000000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 2000000000);

		assert_ok!(XYK::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000,
			1_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 10100000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1980237622);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999989900000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999998019762378);
		expect_events(vec![
			Event::PoolCreated(user_1, asset_a, asset_b, 2000000000, share_token, pair_account).into(),
			Event::SellExecuted(
				user_1,
				asset_a,
				asset_b,
				100_000,
				19_762_378,
				asset_b,
				39_602,
				pair_account,
			)
			.into(),
		]);
	});
}
#[test]
fn discount_sell_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1_000_000u128),
		(BOB, HDX, 1_000_000u128),
		(ALICE, ACA, 1_000_000u128),
		(BOB, ACA, 1_000_000u128),
		(ALICE, DOT, 1_000_000u128),
		(BOB, DOT, 1_000u128),
	];

	let mut ext: sp_io::TestExternalities = ExtBuilder::default().with_accounts(accounts).build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			HDX,
			5_000,
			Price::from(2)
		));
		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			30_000,
			Price::from(2)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let native_pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: HDX,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 30_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 60_000);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 5_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 10_000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 965_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 940_000);
		assert_eq!(Currency::free_balance(HDX, &user_1), 990_000);

		assert_ok!(XYK::sell(Origin::signed(user_1), asset_a, asset_b, 10_000, 1_500, true,));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 40_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 45_007);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 5_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 10_000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 955_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 954_993);
		assert_eq!(Currency::free_balance(HDX, &user_1), 989_986);

		let name: Vec<u8> = vec![208, 7, 0, 0, 72, 68, 84, 184, 11, 0, 0];
		let bounded_name: BoundedVec<u8, <Test as pallet_asset_registry::Config>::StringLimit> =
			name.try_into().unwrap();

		let share_token = XYK::share_token(pair_account);
		let share_token_native = XYK::share_token(native_pair_account);

		expect_events(vec![
			Event::PoolCreated(user_1, asset_a, HDX, 10_000, share_token_native, native_pair_account).into(),
			pallet_asset_registry::Event::Registered(1, bounded_name, AssetType::PoolShare(asset_a, asset_b)).into(),
			frame_system::Event::NewAccount(pair_account).into(),
			orml_tokens::Event::Endowed {
				currency_id: asset_a,
				who: pair_account,
				amount: 30000,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: asset_b,
				who: pair_account,
				amount: 60000,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: 1,
				who: 1,
				amount: 60000,
			}
			.into(),
			Event::PoolCreated(user_1, asset_a, asset_b, 60_000, share_token, pair_account).into(),
			Event::SellExecuted(user_1, asset_a, asset_b, 10_000, 14_993, asset_b, 7, pair_account).into(),
		]);
	});
}

#[test]
fn sell_without_sufficient_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			1_000_000_000,
			Price::from(1)
		));

		assert_ok!(Currency::transfer(Origin::signed(user), BOB, ACA, 999_998_999_999_999));

		assert_noop!(
			XYK::sell(Origin::signed(user), ACA, DOT, 1_000, 100, false),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn sell_without_sufficient_discount_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			1_000_000_000_000,
			Price::from(1)
		));

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			HDX,
			1_000_000_000_000,
			Price::from(1)
		));

		assert_ok!(Currency::transfer(Origin::signed(user), BOB, HDX, 998_999_999_999_999));

		assert_noop!(
			XYK::sell(Origin::signed(user), ACA, DOT, 1_000_000_000, 100, true),
			Error::<Test>::InsufficientNativeCurrencyBalance
		);
	});
}

#[test]
fn buy_without_sufficient_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			1_000_000_000,
			Price::from(1)
		));

		assert_ok!(Currency::transfer(Origin::signed(user), BOB, ACA, 999_998_999_999_999));

		assert_noop!(
			XYK::buy(Origin::signed(user), DOT, ACA, 1_000, 10_000, false),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn buy_without_sufficient_discount_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			1_000_000_000_000,
			Price::from(1)
		));

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_b,
			HDX,
			1_000_000_000_000,
			Price::from(1)
		));

		assert_ok!(Currency::transfer(Origin::signed(user), BOB, HDX, 998_999_999_999_999));

		assert_noop!(
			XYK::buy(Origin::signed(user), DOT, ACA, 1_000_000_000, 10_000_000_000, true),
			Error::<Test>::InsufficientNativeCurrencyBalance
		);
	});
}

#[test]
fn single_buy_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			Price::from(3200)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_999_800_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_360_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640_000_000_000);

		assert_ok!(XYK::buy(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			66_666_666,
			1_000_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_999_866_666_666);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_039_360_004_809);
		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 133_333_334);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 960_639_995_191);

		expect_events(vec![
			Event::PoolCreated(user_1, asset_a, asset_b, 640_000_000_000, share_token, pair_account).into(),
			Event::BuyExecuted(
				user_1,
				asset_a,
				asset_b,
				66_666_666,
				319_999_995_201,
				asset_b,
				639_999_990,
				pair_account,
			)
			.into(),
		]);
	});
}

#[test]
fn single_buy_with_discount_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			Price::from(3200)
		));

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			HDX,
			50_000_000_000,
			Price::from(2)
		));

		let native_pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: HDX,
		});

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);
		let share_token_native = XYK::share_token(native_pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_949_800_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_360_000_000_000);

		assert_eq!(Currency::free_balance(HDX, &user_1), 999_900_000_000_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640_000_000_000);

		assert_ok!(XYK::buy(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			66_666_666,
			1_000_000_000_000,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_949_866_666_666);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_039_776_004_806); // compare to values in previous test to see difference!

		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 133_333_334);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 960_223_995_194);
		assert_eq!(Currency::free_balance(HDX, &user_1), 999_899_552_000_014);

		let name: Vec<u8> = vec![232, 3, 0, 0, 72, 68, 84, 184, 11, 0, 0];
		let bounded_name: BoundedVec<u8, <Test as pallet_asset_registry::Config>::StringLimit> =
			name.try_into().unwrap();

		expect_events(vec![
			Event::PoolCreated(user_1, asset_a, asset_b, 640_000_000_000, share_token, pair_account).into(),
			pallet_asset_registry::Event::Registered(1, bounded_name, AssetType::PoolShare(asset_a, HDX)).into(),
			frame_system::Event::NewAccount(native_pair_account).into(),
			orml_tokens::Event::Endowed {
				currency_id: asset_a,
				who: 1003000,
				amount: 50000000000,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: 1000,
				who: 1003000,
				amount: 100000000000,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: 1,
				who: 1,
				amount: 100000000000,
			}
			.into(),
			Event::PoolCreated(
				user_1,
				asset_a,
				HDX,
				100_000_000_000,
				share_token_native,
				native_pair_account,
			)
			.into(),
			Event::BuyExecuted(
				user_1,
				asset_a,
				asset_b,
				66_666_666,
				319_999_995_201,
				asset_b,
				223_999_993,
				pair_account,
			)
			.into(),
		]);
	});
}

#[test]
fn create_pool_with_insufficient_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::create_pool(Origin::signed(ALICE), ACA, HDX, 500, Price::from(3200)),
			Error::<Test>::InsufficientLiquidity
		);

		assert_noop!(
			XYK::create_pool(Origin::signed(ALICE), ACA, HDX, 5000, Price::from_float(0.1f64)),
			Error::<Test>::InsufficientLiquidity
		);

		assert_noop!(
			XYK::create_pool(Origin::signed(ALICE), ACA, HDX, 1000, Price::from(0)),
			Error::<Test>::ZeroInitialPrice
		);
	});
}

#[test]
fn add_liquidity_to_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::add_liquidity(Origin::signed(ALICE), HDX, ACA, 200_000_000_000_000_000, 600_000_000),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn remove_zero_liquidity_from_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::remove_liquidity(Origin::signed(ALICE), HDX, ACA, 100),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn sell_with_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::sell(Origin::signed(ALICE), HDX, DOT, 456_444_678, 1_000_000, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_sell_with_no_native_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			ACA,
			DOT,
			1000,
			Price::from(3200)
		));

		assert_noop!(
			XYK::sell(Origin::signed(ALICE), ACA, DOT, 456_444_678, 1_000_000, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn buy_with_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::buy(Origin::signed(ALICE), HDX, DOT, 456_444_678, 1_000_000_000, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_buy_with_no_native_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			ACA,
			DOT,
			10000,
			Price::from(3200)
		));

		assert_noop!(
			XYK::buy(Origin::signed(ALICE), ACA, DOT, 1000, 1_000_000_000, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn create_pool_small_fixed_point_amount_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;

		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000_000,
			Price::from_float(0.00001)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000);
		assert_eq!(Currency::free_balance(asset_a, &ALICE), 900000000000000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 999999000000000);
		assert_eq!(Currency::free_balance(share_token, &ALICE), 100000000000000);
		assert_eq!(XYK::total_liquidity(&pair_account), 100000000000000);

		expect_events(vec![Event::PoolCreated(
			ALICE,
			asset_a,
			asset_b,
			100000000000000,
			share_token,
			pair_account,
		)
		.into()]);
	});
}

#[test]
fn create_pool_fixed_point_amount_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000,
			Price::from_float(4560.234543)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 456023454299999);
		assert_eq!(Currency::free_balance(asset_a, &ALICE), 999900000000000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 543976545700001);
		assert_eq!(Currency::free_balance(share_token, &ALICE), 100000000000);
		assert_eq!(XYK::total_liquidity(&pair_account), 100000000000);

		expect_events(vec![Event::PoolCreated(
			ALICE,
			asset_a,
			asset_b,
			100000000000,
			share_token,
			pair_account,
		)
		.into()]);
	});
}

#[test]
fn money_in_sell_money_out_should_leave_the_same_balance() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		let user_1_balance_a_before = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_before = Currency::free_balance(asset_b, &user_1);

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_ok!(XYK::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			456_444_678,
			1000000000000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999799543555322);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 401363483591788);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200456444678);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 598636516408212);

		assert_ok!(XYK::remove_liquidity(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			600000000000000
		));

		let user_1_balance_a_after = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_after = Currency::free_balance(asset_b, &user_1);

		assert_eq!(user_1_balance_a_before, user_1_balance_a_after);
		assert_eq!(user_1_balance_b_before, user_1_balance_b_after);
	});
}

#[test]
fn money_in_money_out_should_leave_the_same_balance_for_both_accounts() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = HDX;
		let asset_b = DOT;

		let user_1_balance_a_before = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_before = Currency::free_balance(asset_b, &user_1);
		let user_2_balance_a_before = Currency::free_balance(asset_a, &user_2);
		let user_2_balance_b_before = Currency::free_balance(asset_b, &user_2);

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		let asset_pair = AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		};

		let pair_account = XYK::get_pair_id(asset_pair);
		let share_token = XYK::share_token(pair_account);

		assert!(XYK::exists(asset_pair));

		assert_ok!(XYK::add_liquidity(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			100_000_000,
			1_000_000_000_000
		));

		assert_eq!(Currency::free_balance(share_token, &user_1), 100_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 100_000_000);

		assert_ok!(XYK::remove_liquidity(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000_000
		));

		assert_ok!(XYK::remove_liquidity(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			100_000_000
		));

		assert_eq!(XYK::total_liquidity(&pair_account), 0);

		let user_1_balance_a_after = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_after = Currency::free_balance(asset_b, &user_1);
		let user_2_balance_a_after = Currency::free_balance(asset_a, &user_2);
		let user_2_balance_b_after = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_1_balance_a_before, user_1_balance_a_after);
		assert_eq!(user_1_balance_b_before, user_1_balance_b_after);
		assert_eq!(user_2_balance_a_before, user_2_balance_a_after);
		assert_eq!(user_2_balance_b_before, user_2_balance_b_after);

		assert!(!XYK::exists(asset_pair));
	});
}

#[test]
fn destroy_pool_on_remove_liquidity_and_recreate_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		let asset_pair = AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		};

		let pair_account = XYK::get_pair_id(asset_pair);
		let share_token = XYK::share_token(pair_account);

		assert!(XYK::exists(asset_pair));

		assert_ok!(XYK::remove_liquidity(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000
		));

		assert_eq!(XYK::total_liquidity(&pair_account), 0);

		assert!(!XYK::exists(asset_pair));

		// It should be possible to recreate the pool again

		assert_ok!(XYK::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		expect_events(vec![
			Event::PoolCreated(user, asset_a, asset_b, 100_000_000, share_token, pair_account).into(),
			frame_system::Event::KilledAccount(pair_account).into(),
			Event::LiquidityRemoved(user, asset_a, asset_b, 100_000_000).into(),
			Event::PoolDestroyed(user, asset_a, asset_b, share_token, pair_account).into(),
			frame_system::Event::NewAccount(pair_account).into(),
			orml_tokens::Event::Endowed {
				currency_id: asset_a,
				who: pair_account,
				amount: 100000000,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: asset_b,
				who: pair_account,
				amount: 1000000000000,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: 0,
				who: 1,
				amount: 100000000,
			}
			.into(),
			Event::PoolCreated(user, asset_a, asset_b, 100_000_000, share_token, pair_account).into(),
		]);
	});
}

#[test]
fn create_pool_with_same_assets_should_not_be_allowed() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;

		assert_noop!(
			XYK::create_pool(Origin::signed(user), asset_a, asset_a, 100_000_000, Price::from(10_000)),
			Error::<Test>::CannotCreatePoolWithSameAssets
		);
	})
}

#[test]
fn sell_test_not_reaching_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_noop!(
			XYK::sell(
				Origin::signed(user_1),
				asset_a,
				asset_b,
				456_444_678,
				1_000_000_000_000_000,
				false,
			),
			Error::<Test>::AssetAmountNotReachedLimit
		);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);
	});
}

#[test]
fn buy_test_exceeding_max_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_noop!(
			XYK::buy(
				Origin::signed(user_1),
				asset_a,
				asset_b,
				456_444_678,
				1_000_000_000,
				false,
			),
			Error::<Test>::AssetAmountExceededLimit
		);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);
	});
}

#[test]
fn single_buy_more_than_ratio_out_should_not_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			Price::from(3200)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_999_800_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_360_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640_000_000_000);

		assert_noop!(
			XYK::buy(
				Origin::signed(user_1),
				asset_a,
				asset_b,
				66_666_667,
				1_000_000_000_000,
				false,
			),
			Error::<Test>::MaxOutRatioExceeded
		);
	});
}

#[test]
fn single_sell_more_than_ratio_in_should_not_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_800_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400_000_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600_000_000_000_000);

		assert_noop!(
			XYK::sell(
				Origin::signed(user_1),
				asset_a,
				asset_b,
				66_666_666_667,
				10_000_000,
				false,
			),
			Error::<Test>::MaxInRatioExceeded
		);
	});
}

#[test]
fn test_calculate_out_given_in() {
	ExtBuilder::default().build().execute_with(|| {
		let in_reserve: Balance = 10000000000000;
		let out_reserve: Balance = 100000;
		let in_amount: Balance = 100000000000;
		let result = hydra_dx_math::xyk::calculate_out_given_in(in_reserve, out_reserve, in_amount);
		assert_eq!(result, Ok(990));
	});
}

#[test]
fn test_calculate_out_given_in_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		let in_reserve: Balance = 0;
		let out_reserve: Balance = 1000;
		let in_amount: Balance = 0;
		let result = hydra_dx_math::xyk::calculate_out_given_in(in_reserve, out_reserve, in_amount);
		assert_eq!(result, Ok(0));
	});
}

#[test]
fn test_calculate_in_given_out_insufficient_pool_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let in_reserve: Balance = 10000000000000;
		let out_reserve: Balance = 100000;
		let out_amount: Balance = 100000000000;
		let result = hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, out_amount);
		assert_eq!(result, Err(MathError::InsufficientOutReserve));
	});
}

#[test]
fn test_calculate_in_given_out() {
	ExtBuilder::default().build().execute_with(|| {
		let in_reserve: Balance = 10000000000000;
		let out_reserve: Balance = 10000000;
		let out_amount: Balance = 1000000;
		let result = hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, out_amount);
		assert_eq!(result, Ok(1111111111112));
	});
}

#[test]
fn sell_with_low_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::sell(Origin::signed(ALICE), HDX, DOT, 1, 1_000_000, false),
			Error::<Test>::InsufficientTradingAmount
		);
	});
}

#[test]
fn buy_with_low_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::buy(Origin::signed(ALICE), HDX, DOT, 1, 1_000_000, false),
			Error::<Test>::InsufficientTradingAmount
		);
	});
}

#[test]
fn buy_with_excesive_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			HDX,
			DOT,
			10_000,
			Price::from(1)
		));

		assert_noop!(
			XYK::buy(Origin::signed(ALICE), HDX, DOT, 20_000, 1_000_000, false),
			Error::<Test>::InsufficientPoolAssetBalance
		);
	});
}

#[test]
fn fee_calculation() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(XYK::calculate_discounted_fee(10000), Ok(7));
		assert_eq!(XYK::calculate_discounted_fee(100000), Ok(70));
		assert_eq!(XYK::calculate_discounted_fee(100000), Ok(70));

		assert_eq!(XYK::calculate_fee(100000), Ok(200));
		assert_eq!(XYK::calculate_fee(10000), Ok(20));
	});
	ExtBuilder::default()
		.with_exchange_fee((10, 1000))
		.build()
		.execute_with(|| {
			assert_eq!(XYK::calculate_fee(100000), Ok(1000));
			assert_eq!(XYK::calculate_fee(10000), Ok(100));
		});

	ExtBuilder::default()
		.with_exchange_fee((10, 0))
		.build()
		.execute_with(|| {
			assert_eq!(XYK::calculate_fee(100000), Ok(0));
		});

	ExtBuilder::default()
		.with_exchange_fee((10, 1))
		.build()
		.execute_with(|| {
			assert_noop!(XYK::calculate_fee(u128::MAX), Error::<Test>::FeeAmountInvalid);
		});
}

#[test]
fn can_create_pool_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = 10u32;
		let asset_b = 10u32;
		assert_noop!(
			XYK::create_pool(
				Origin::signed(ALICE),
				asset_a,
				asset_b,
				100_000_000_000_000,
				Price::from(10)
			),
			Error::<Test>::CannotCreatePool
		);
	});
}

#[test]
fn get_fee_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			Origin::signed(ALICE),
			HDX,
			DOT,
			1_000_000_000,
			Price::from(2)
		),);

		// existing pool
		let fee = XYK::get_fee(&HDX_DOT_POOL_ID);
		assert_eq!(fee, (2, 1_000));
		// non existing pool
		let fee = XYK::get_fee(&1_234);
		assert_eq!(fee, (2, 1_000));
	});
}
