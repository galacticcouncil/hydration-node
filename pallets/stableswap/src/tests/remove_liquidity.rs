use crate::tests::mock::*;
use crate::types::{AssetAmount, PoolInfo};
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added),]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				shares,
				0,
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 199999999999999u128);
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(10),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added)]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				shares,
				0
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 190688958461730);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 100 * ONE + amount_added);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_received);
		});
}

#[test]
fn remove_liquidity_should_fail_when_shares_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(ALICE), 0u32, 1u32, 0u128, 0),
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
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, 1u32, 200 * ONE, 0),
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
					100 * ONE - MinimumLiquidity::get() + 1,
					0,
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
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, 1u32, 100 * ONE, 0),
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(10),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added)]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, asset_d, shares, 0),
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(10),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added)]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_c,
					shares - MinimumLiquidity::get() + 1,
					0,
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("four".as_bytes().to_vec(), asset_d, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c, asset_d].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_float(0.003),
				withdraw_fee: Permill::from_float(0.003),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000 * ONE),
					AssetAmount::new(asset_b, 1_000_000 * ONE),
					AssetAmount::new(asset_c, 1_000_000 * ONE),
					AssetAmount::new(asset_d, 1_000_000 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added)]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				shares,
				0
			));

			let amount_received = Tokens::free_balance(asset_b, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_b, 99749116913542959); //TODO: double check python impl
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 1_000_000 * ONE + amount_added);
			assert_balance!(pool_account, asset_b, 1_000_000 * ONE - amount_received);
			assert_balance!(pool_account, asset_b, 900250883086457041);
		});
}

#[test]
fn remove_liquidity_fail_when_desired_min_limit_is_not_reached() {
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added)]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, asset_c, shares, 200 * ONE,),
				Error::<Test>::MinimumAmountNotReached
			);
		});
}

#[test]
fn scenario_add_remove_with_different_decimals() {
	let asset_a: AssetId = 2; // DAI
	let asset_b: AssetId = 7; // USDC
	let asset_c: AssetId = 10; // USDT

	let dec_a: u32 = 18;
	let dec_b: u32 = 6;
	let dec_c: u32 = 6;

	let one_a = 10u128.pow(dec_a);
	let one_b = 10u128.pow(dec_b);
	let one_c = 10u128.pow(dec_c);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_c, 20 * one_c),
			(ALICE, asset_a, 3000000 * one_a),
			(ALICE, asset_b, 2000000 * one_b),
			(ALICE, asset_c, 1000000 * one_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_float(0.0),
				withdraw_fee: Permill::from_float(0.0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000 * one_a),
					AssetAmount::new(asset_b, 1_000_000 * one_b),
					AssetAmount::new(asset_c, 1_000_000 * one_c),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_c, 20 * one_c)]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				shares,
				0,
			));

			let balance_received = Tokens::free_balance(asset_a, &BOB);
			//assert_eq!(balance_received, 9999703908493044130); //before decimals fix
			assert_eq!(balance_received, 19_999_999_955_560_493_353);
		});
}

#[test]
fn scenario_sell_with_different_decimals() {
	let asset_a: AssetId = 2; // DAI
	let asset_b: AssetId = 7; // USDC
	let asset_c: AssetId = 10; // USDT

	let dec_a: u32 = 18;
	let dec_b: u32 = 6;
	let dec_c: u32 = 6;

	let one_a = 10u128.pow(dec_a);
	let one_b = 10u128.pow(dec_b);
	let one_c = 10u128.pow(dec_c);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_c, 20 * one_c),
			(ALICE, asset_a, 3000000 * one_a),
			(ALICE, asset_b, 2000000 * one_b),
			(ALICE, asset_c, 1000000 * one_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_float(0.0),
				withdraw_fee: Permill::from_float(0.0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000 * one_a),
					AssetAmount::new(asset_b, 1_000_000 * one_b),
					AssetAmount::new(asset_c, 1_000_000 * one_c),
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
				asset_a,
				20 * one_c,
				0,
			));

			let balance_received = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance_received, 19_999_999_955_560_493_356);
		});
}

#[test]
fn scenario_1_withdrawing_shares_should_work_when_specifying_amount() {
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added),]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				10 * ONE,
				shares,
			));

			let remaining_shares = Tokens::free_balance(pool_id, &BOB);
			assert!(remaining_shares < shares);

			let shares_used = shares - remaining_shares;
			dbg!(shares_used);
			assert_eq!(shares_used, 9987736801555837999);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 10 * ONE);
			assert_balance!(pool_account, asset_c, 300 * ONE - 10 * ONE);
		});
}

#[test]
fn scenario_1_remove_liq() {
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added),]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				9987736801555837999,// + 3000,
				9 * ONE,
			));

			let remaining_shares = Tokens::free_balance(pool_id, &BOB);
			assert!(remaining_shares < shares);

			let received_remove_liq = Tokens::free_balance(asset_c, &BOB);
			dbg!(received_remove_liq);

			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 10 * ONE);
			assert_balance!(pool_account, asset_c, 300 * ONE - 10 * ONE);
		});
}

#[test]
fn scenario_2_withdrawing_shares_should_work_when_specifying_amount_with_fee() {
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_float(0.0001),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added),]
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				10 * ONE,
				shares,
			));

			let remaining_shares = Tokens::free_balance(pool_id, &BOB);
			assert!(remaining_shares < shares);

			let shares_used = shares - remaining_shares;
			dbg!(shares_used);
			assert_eq!(shares_used, 10034584891395272495);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 10 * ONE);
			assert_balance!(pool_account, asset_c, 300 * ONE - 10 * ONE);
		});
}

#[test]
fn scenario_2_remove_liq() {
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
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_float(0.0001),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
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
				vec![AssetAmount::new(asset_a, amount_added),]
			));

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				9988205282442480903,
				9 * ONE,
			));

			let received_remove_liq = Tokens::free_balance(asset_c, &BOB);
			dbg!(received_remove_liq);

			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 10 * ONE	);
			//assert_balance!(pool_account, asset_c, 300 * ONE - 10 * ONE);
		});
}

#[test]
fn scenario_3_remove() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_float(0.0001),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let pool_account = pool_account(pool_id);
			dbg!(pool_id);

			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let shares0 = Tokens::total_issuance(pool_id);
			let shares1 = Tokens::free_balance(pool_id, &ALICE);
			dbg!(shares0);
			dbg!(shares1);


			let to_withdraw = 599540994996813062914899;

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				asset_b,
				to_withdraw,
				0,
			));

			let received_remove_liq = Tokens::free_balance(asset_b, &ALICE);
			dbg!(received_remove_liq);
		});
}

#[test]
fn scenario_3_exact_amount() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_float(0.0001),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let pool_account = pool_account(pool_id);
			dbg!(pool_id);

			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let to_withdraw = 1_599540994996813062914899;
			//let exact_amount = 615488363754;
			let exact_amount = 615630069100;

			let shares = Tokens::free_balance(pool_id, &ALICE);

			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				asset_b,
				exact_amount,
				to_withdraw,
			));

			let remaining_shares = Tokens::free_balance(pool_id, &ALICE);

			let shares_used = shares - remaining_shares;
			dbg!(shares_used);

			assert_eq!(shares_used, to_withdraw);

		});
}

#[test]
fn scenario_3_diff() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_percent(0),
				withdraw_fee: Permill::from_float(0.0001),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let pool_account = pool_account(pool_id);
			dbg!(pool_id);

			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let shares0 = Tokens::total_issuance(pool_id);
			let shares1 = Tokens::free_balance(pool_id, &ALICE);
			dbg!(shares0);
			dbg!(shares1);


			let to_withdraw = 1986695389615175;
			//let to_withdraw = 1108899784017676500393;

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				asset_b,
				to_withdraw,
				0,
			));

			let received_remove_liq_diff = Tokens::free_balance(asset_b, &ALICE);
			dbg!(received_remove_liq_diff);
		});
}

#[test]
fn scenario_3_trade() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 2_000_000_000_000_000_000),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				trade_fee: Permill::from_float(0.0001),
				withdraw_fee: Permill::from_float(0.0005),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let pool_account = pool_account(pool_id);

			let amount = 2_000_000_000_000_000_000;

			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				amount,
				0,
			));

			let received = Tokens::free_balance(asset_b, &BOB);
			dbg!(received);
		});
}
