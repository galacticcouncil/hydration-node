use crate::tests::mock::*;
use crate::types::{AssetLiquidity, PoolInfo};
use crate::{assert_balance, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;
use std::num::NonZeroU16;

#[test]
fn remove_liquidity_should_work_when_withdrawing_all_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				amplification: NonZeroU16::new(100).unwrap(),
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
						amount: 200 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_c,
						amount: 300 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				shares,
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 199999999999994u128);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 100 * ONE + amount_added);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_received);
		});
}

#[test]
fn remove_liquidity_should_apply_fee_when_withdrawing_all_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				amplification: NonZeroU16::new(100).unwrap(),
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(10),
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
						amount: 200 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_c,
						amount: 300 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				shares,
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 190632279384122);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 100 * ONE + amount_added);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_received);
		});
}

#[test]
fn remove_liquidity_should_fail_when_shares_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(ALICE), 0u32, 1u32, 0u128),
			Error::<Test>::InvalidAssetAmount
		);
	});
}

#[test]
fn remove_liquidity_should_fail_when_shares_is_insufficient() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, pool_id, 100 * ONE)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, 1u32, 200 * ONE),
				Error::<Test>::InsufficientShares
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_remaining_shares_is_below_min_limit() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, pool_id, 100 * ONE)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(BOB),
					pool_id,
					1u32,
					100 * ONE - MinimumLiquidity::get() + 1
				),
				Error::<Test>::InsufficientShareBalance
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_pool_does_not_exists() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, pool_id, 100 * ONE)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, 1u32, 100 * ONE),
				Error::<Test>::PoolNotFound
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_requested_asset_not_in_pool() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				amplification: NonZeroU16::new(100).unwrap(),
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(10),
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
						amount: 200 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_c,
						amount: 300 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, asset_d, shares,),
				Error::<Test>::AssetNotInPool
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_remaining_shares_below_min_liquidity() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				amplification: NonZeroU16::new(100).unwrap(),
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(10),
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
						amount: 200 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_c,
						amount: 300 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_c,
					shares - MinimumLiquidity::get() + 1,
				),
				Error::<Test>::InsufficientShareBalance
			);
		});
}

#[test]
fn verify_remove_liquidity_against_research_impl() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 100_000 * ONE),
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
			(ALICE, asset_c, 1_000_000 * ONE),
			(ALICE, asset_d, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c)
		.with_registered_asset("four".as_bytes().to_vec(), asset_d)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId> {
				assets: vec![asset_a, asset_b, asset_c, asset_d].try_into().unwrap(),
				amplification: NonZeroU16::new(100).unwrap(),
				trade_fee: Permill::from_float(0.003),
				withdraw_fee: Permill::from_float(0.003),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetLiquidity {
						asset_id: asset_a,
						amount: 1_000_000 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_b,
						amount: 1_000_000 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_c,
						amount: 1_000_000 * ONE,
					},
					AssetLiquidity {
						asset_id: asset_d,
						amount: 1_000_000 * ONE,
					},
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 100_000 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				shares,
			));

			let amount_received = Tokens::free_balance(asset_b, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_b, 99847206046905539);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 1_000_000 * ONE + amount_added);
			assert_balance!(pool_account, asset_b, 1_000_000 * ONE - amount_received);
			assert_balance!(pool_account, asset_b, 900152793953094461);
		});
}
