use crate::tests::mock::*;
use crate::types::{AssetLiquidity, PoolInfo};
use crate::{assert_balance, Error};
use hydradx_traits::AccountIdFor;

use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;

#[test]
fn sell_should_work_when_correct_input_provided() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1)
		.with_registered_asset("two".as_bytes().to_vec(), 2)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				Origin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 29_950_934_311_773u128;

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_balance!(BOB, asset_a, 170 * ONE);
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_a, 130 * ONE);
			assert_balance!(pool_account, asset_b, 100 * ONE - expected);
		});
}

#[test]
fn buy_should_work_when_correct_input_provided() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1)
		.with_registered_asset("two".as_bytes().to_vec(), 2)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::buy(
				Origin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				30 * ONE,
				35 * ONE,
			));

			let expected_to_sell = 30049242502720u128;

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_balance!(BOB, asset_a, 200 * ONE - expected_to_sell);
			assert_balance!(BOB, asset_b, 30 * ONE);
			assert_balance!(pool_account, asset_a, 100 * ONE + expected_to_sell);
			assert_balance!(pool_account, asset_b, 70 * ONE);
		});
}

#[test]
fn sell_with_fee_should_work_when_correct_input_provided() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1)
		.with_registered_asset("two".as_bytes().to_vec(), 2)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_percent(10),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				Origin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 29950934311773u128;

			let fee = Permill::from_percent(10).mul_floor(expected);

			let expected = expected - fee;

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_balance!(BOB, asset_a, 170 * ONE);
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_a, 130 * ONE);
			assert_balance!(pool_account, asset_b, 100 * ONE - expected);
		});
}

#[test]
fn sell_should_work_when_fee_is_small() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1)
		.with_registered_asset("two".as_bytes().to_vec(), 2)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_rational(3u32, 1000u32),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				Origin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 29950934311773u128;

			let fee = Permill::from_float(0.003).mul_floor(expected);

			let expected = expected - fee;

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_balance!(BOB, asset_a, 170 * ONE);
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_a, 130 * ONE);
			assert_balance!(pool_account, asset_b, 100 * ONE - expected);
		});
}

#[test]
fn buy_should_work_when_fee_is_set() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1)
		.with_registered_asset("two".as_bytes().to_vec(), 2)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_percent(10),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::buy(
				Origin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				30 * ONE,
				35 * ONE,
			));

			let expected_to_sell = 30049242502720u128;

			let fee = Permill::from_percent(10).mul_ceil(expected_to_sell);

			let expected_to_sell = expected_to_sell + fee;

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_balance!(BOB, asset_a, 200 * ONE - expected_to_sell);
			assert_balance!(BOB, asset_b, 30 * ONE);
			assert_balance!(pool_account, asset_a, 100 * ONE + expected_to_sell);
			assert_balance!(pool_account, asset_b, 70 * ONE);
		});
}

#[test]
fn sell_should_fail_when_insufficient_amount_is_provided() {
	let asset_a: AssetId = 1000;
	let asset_b: AssetId = 2000;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1000, 200 * ONE),
			(BOB, 3000, 200 * ONE),
			(ALICE, 1000, 200 * ONE),
			(ALICE, 2000, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1000)
		.with_registered_asset("two".as_bytes().to_vec(), 2000)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_noop!(
				Stableswap::sell(Origin::signed(BOB), pool_id, asset_a, asset_b, 30, 25 * ONE,),
				Error::<Test>::InsufficientTradingAmount
			);

			assert_noop!(
				Stableswap::sell(Origin::signed(BOB), pool_id, asset_a, asset_b, 30000 * ONE, 25 * ONE,),
				Error::<Test>::InsufficientBalance
			);

			assert_noop!(
				Stableswap::sell(Origin::signed(BOB), pool_id + 1, asset_a, asset_b, 30 * ONE, 25 * ONE,),
				Error::<Test>::PoolNotFound
			);
			assert_noop!(
				Stableswap::sell(Origin::signed(BOB), pool_id, 3000, asset_b, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);
			assert_noop!(
				Stableswap::sell(Origin::signed(BOB), pool_id, asset_a, 3000, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);

			assert_noop!(
				Stableswap::sell(Origin::signed(BOB), pool_id, asset_a, asset_b, 30 * ONE, 250 * ONE,),
				Error::<Test>::BuyLimitNotReached
			);
		});
}

#[test]
fn buy_should_fail_when_insufficient_amount_is_provided() {
	let asset_a: AssetId = 1000;
	let asset_b: AssetId = 2000;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(BOB, 3000, 200 * ONE),
			(ALICE, asset_a, 200 * ONE),
			(ALICE, asset_b, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				amplification: 100u16,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 100 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 100 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id, asset_a, asset_b, 30, 25 * ONE,),
				Error::<Test>::InsufficientTradingAmount
			);

			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id, asset_a, asset_b, 30000 * ONE, 25 * ONE,),
				Error::<Test>::InsufficientLiquidity
			);

			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id, asset_a, asset_b, 90 * ONE, 30000 * ONE,),
				Error::<Test>::InsufficientBalance
			);

			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id + 1, asset_a, asset_b, 30 * ONE, 25 * ONE,),
				Error::<Test>::PoolNotFound
			);

			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id, 3000, asset_b, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);
			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id, asset_a, 3000, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);

			assert_noop!(
				Stableswap::buy(Origin::signed(BOB), pool_id, asset_a, asset_b, 30 * ONE, 10 * ONE,),
				Error::<Test>::SellLimitExceeded
			);
		});
}
