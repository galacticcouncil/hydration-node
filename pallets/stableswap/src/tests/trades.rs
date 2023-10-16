use crate::tests::mock::*;
use crate::types::{AssetAmount, PoolInfo};
use crate::{assert_balance, to_precision, Error};
use std::num::NonZeroU16;

use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;

#[test]
fn sell_should_work_when_correct_input_provided() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 29_902_625_420_922u128;

			let pool_account = pool_account(pool_id);

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
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				30 * ONE,
				35 * ONE,
			));

			let expected_to_sell = 30098072706882u128;

			let pool_account = pool_account(pool_id);

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
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(10),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 26912362878830u128;
			let pool_account = pool_account(pool_id);
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
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_rational(3u32, 1000u32),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 29812917544660u128;
			let pool_account = pool_account(pool_id);
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
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(10),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				30 * ONE,
				35 * ONE,
			));

			let expected_to_sell = 33_107_879_977_571;
			let pool_account = pool_account(pool_id);
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
		.with_registered_asset("one".as_bytes().to_vec(), 1000, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2000, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_noop!(
				Stableswap::sell(RuntimeOrigin::signed(BOB), pool_id, asset_a, asset_b, 30, 25 * ONE,),
				Error::<Test>::InsufficientTradingAmount
			);

			assert_noop!(
				Stableswap::sell(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					30000 * ONE,
					25 * ONE,
				),
				Error::<Test>::InsufficientBalance
			);

			assert_noop!(
				Stableswap::sell(
					RuntimeOrigin::signed(BOB),
					pool_id + 1,
					asset_a,
					asset_b,
					30 * ONE,
					25 * ONE,
				),
				Error::<Test>::PoolNotFound
			);
			assert_noop!(
				Stableswap::sell(RuntimeOrigin::signed(BOB), pool_id, 3000, asset_b, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);
			assert_noop!(
				Stableswap::sell(RuntimeOrigin::signed(BOB), pool_id, asset_a, 3000, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);

			assert_noop!(
				Stableswap::sell(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					30 * ONE,
					250 * ONE,
				),
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_noop!(
				Stableswap::buy(RuntimeOrigin::signed(BOB), pool_id, asset_a, asset_b, 30, 25 * ONE,),
				Error::<Test>::InsufficientTradingAmount
			);

			assert_noop!(
				Stableswap::buy(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					30000 * ONE,
					25 * ONE,
				),
				Error::<Test>::InsufficientLiquidity
			);

			assert_noop!(
				Stableswap::buy(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					90 * ONE,
					30000 * ONE,
				),
				Error::<Test>::InsufficientBalance
			);

			assert_noop!(
				Stableswap::buy(
					RuntimeOrigin::signed(BOB),
					pool_id + 1,
					asset_a,
					asset_b,
					30 * ONE,
					25 * ONE,
				),
				Error::<Test>::PoolNotFound
			);

			assert_noop!(
				Stableswap::buy(RuntimeOrigin::signed(BOB), pool_id, 3000, asset_b, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);
			assert_noop!(
				Stableswap::buy(RuntimeOrigin::signed(BOB), pool_id, asset_a, 3000, 30 * ONE, 25 * ONE,),
				Error::<Test>::AssetNotInPool
			);

			assert_noop!(
				Stableswap::buy(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					30 * ONE,
					10 * ONE,
				),
				Error::<Test>::SellLimitExceeded
			);
		});
}

#[test]
fn sell_should_work_when_pool_have_asset_with_various_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_registered_asset("three".as_bytes().to_vec(), 3, 18)
		.with_endowed_accounts(vec![
			(BOB, asset_c, ONE * 1_000_000),
			(ALICE, asset_a, 2000 * ONE),
			(ALICE, asset_b, 4000 * ONE),
			(ALICE, asset_c, 1000 * ONE * 1_000_000),
		])
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000_000_000_000),
					AssetAmount::new(asset_b, 3_000_000_000_000_000),
					AssetAmount::new(asset_c, 1000 * ONE * 1_000_000),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				asset_b,
				ONE * 1_000_000,
				0,
			));

			let expected = 1_001_709_976_613;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_c, 0);
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_c, 1_001_000_000_000_000_000_000);
			assert_balance!(pool_account, asset_b, 3_000 * ONE - expected);
		});
}

#[test]
fn buy_should_work_when_pool_have_asset_with_various_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_registered_asset("three".as_bytes().to_vec(), 3, 18)
		.with_endowed_accounts(vec![
			(BOB, asset_c, ONE * 1_000_000),
			(ALICE, asset_a, 2000 * ONE),
			(ALICE, asset_b, 4000 * ONE),
			(ALICE, asset_c, 10000 * ONE * 1_000_000),
		])
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000_000_000_000),
					AssetAmount::new(asset_b, 3_000_000_000_000_000),
					AssetAmount::new(asset_c, 1000 * ONE * 1_000_000),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let buy_amount = 1_001_709_976_614;

			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_c,
				buy_amount,
				2 * ONE * 1_000_000,
			));

			let paid = 999999999999187343;
			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_c, 1_000_000_000_000_000_000 - paid);
			assert_balance!(BOB, asset_b, buy_amount);
			assert_balance!(pool_account, asset_c, 1000 * ONE * 1_000_000 + paid);
			assert_balance!(pool_account, asset_b, 3_000_000_000_000_000 - buy_amount);
		});
}

#[test]
fn sell_should_work_when_assets_have_different_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let dec_a: u8 = 18;
	let dec_b: u8 = 6;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, to_precision!(200, dec_a)),
			(ALICE, asset_a, to_precision!(200, dec_a)),
			(ALICE, asset_b, to_precision!(200, dec_b)),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, dec_a)
		.with_registered_asset("two".as_bytes().to_vec(), 2, dec_b)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, to_precision!(100, dec_a)),
					AssetAmount::new(asset_b, to_precision!(100, dec_b)),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				to_precision!(30, dec_a),
				to_precision!(27, dec_b),
			));

			let expected = 29_902_624u128;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_a, to_precision!(170, dec_a));
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_a, to_precision!(130, dec_a));
			assert_balance!(pool_account, asset_b, to_precision!(100, dec_b) - expected);
		});
}

#[test]
fn buy_should_work_when_assets_have_different_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let dec_a: u8 = 18;
	let dec_b: u8 = 6;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, to_precision!(200, dec_a)),
			(ALICE, asset_a, to_precision!(200, dec_a)),
			(ALICE, asset_b, to_precision!(200, dec_b)),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, dec_a)
		.with_registered_asset("two".as_bytes().to_vec(), 2, dec_b)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, to_precision!(100, dec_a)),
					AssetAmount::new(asset_b, to_precision!(100, dec_b)),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				to_precision!(30, dec_b),
				to_precision!(31, dec_a),
			));

			let expected_to_sell = 30_098_072_706_880_214_087u128;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_a, to_precision!(200, dec_a) - expected_to_sell);
			assert_balance!(BOB, asset_b, to_precision!(30, dec_b));
			assert_balance!(pool_account, asset_a, to_precision!(100, dec_a) + expected_to_sell);
			assert_balance!(pool_account, asset_b, to_precision!(70, dec_b));
		});
}
