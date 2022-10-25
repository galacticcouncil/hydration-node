use crate::tests::mock::*;
use crate::types::{AssetLiquidity, PoolInfo};
use crate::{assert_balance, Error};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::AccountIdFor;
use sp_runtime::Permill;

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

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_ok!(Stableswap::add_liquidity(
				Origin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				Origin::signed(BOB),
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
				amplification: 100u16,
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

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_a, asset_b], None);

			assert_ok!(Stableswap::add_liquidity(
				Origin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				Origin::signed(BOB),
				pool_id,
				asset_c,
				shares,
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 175017638623598u128);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 100 * ONE + amount_added);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_received);
		});
}

#[test]
fn remove_liquidity_should_fail_when_shares_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Stableswap::remove_liquidity_one_asset(Origin::signed(ALICE), 0u32, 1u32, 0u128),
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
				Stableswap::remove_liquidity_one_asset(Origin::signed(BOB), pool_id, 1u32, 200 * ONE),
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
					Origin::signed(BOB),
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
				Stableswap::remove_liquidity_one_asset(Origin::signed(BOB), pool_id, 1u32, 100 * ONE),
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
				amplification: 100u16,
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
				Origin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(Origin::signed(BOB), pool_id, asset_d, shares,),
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
				amplification: 100u16,
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
				Origin::signed(BOB),
				pool_id,
				vec![AssetLiquidity {
					asset_id: asset_a,
					amount: amount_added
				},]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(
					Origin::signed(BOB),
					pool_id,
					asset_c,
					shares - MinimumLiquidity::get() + 1,
				),
				Error::<Test>::InsufficientShareBalance
			);
		});
}
