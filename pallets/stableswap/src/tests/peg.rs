use crate::tests::mock::*;
use crate::types::{BoundedPegSources, BoundedPegs, PegSource, PoolPegInfo};
use crate::{assert_balance, Event};
use hydradx_traits::stableswap::AssetAmount;

use frame_support::{assert_ok, BoundedVec};
use num_traits::One;
use pallet_broadcast::types::{Asset, Destination, Fee};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{FixedPointNumber, FixedU128, Permill};

#[test]
fn sell_with_peg_should_work_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = (u128::MAX, 1);

	let peg2_fixed = FixedU128::from_rational(peg2.0, peg2.1);
	let peg3_fixed = FixedU128::from_rational(peg3.0, peg3.1);
	let p1 = peg2_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p2 = FixedU128::one() / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p3 = peg3_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let liquid_a = p1.saturating_mul_int(tvl);
	let liquid_b = p2.saturating_mul_int(tvl);
	let liquid_c = p3.saturating_mul_int(tvl);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b, asset_c],
				amp,
				Permill::from_percent(0),
				PoolPegInfo {
					source: BoundedPegSources::truncate_from(vec![
						PegSource::Value((1, 1)),
						PegSource::Value(peg2),
						PegSource::Value(peg3)
					]),
					max_target_update: max_peg_update,
					current: BoundedPegs::truncate_from(vec![(1, 1), peg2, peg3]),
				}
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				])
			));

			let pool_account = pool_account(pool_id);

			//let pool_liquid_a = Tokens::free_balance(asset_a, &pool_account);
			//let pool_liquid_b = Tokens::free_balance(asset_b, &pool_account);
			//let pool_liquid_c = Tokens::free_balance(asset_c, &pool_account);

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				100 * ONE,
				25 * ONE,
			));

			assert_balance!(BOB, asset_a, 0);
			assert_balance!(BOB, asset_b, 199999950445584);
		});
}

#[test]
fn buy_with_peg_should_work_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = (u128::MAX, 1);

	let peg2_fixed = FixedU128::from_rational(peg2.0, peg2.1);
	let peg3_fixed = FixedU128::from_rational(peg3.0, peg3.1);
	let p1 = peg2_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p2 = FixedU128::one() / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p3 = peg3_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let liquid_a = p1.saturating_mul_int(tvl);
	let liquid_b = p2.saturating_mul_int(tvl);
	let liquid_c = p3.saturating_mul_int(tvl);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE + 1),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b, asset_c],
				amp,
				Permill::from_percent(0),
				PoolPegInfo {
					source: BoundedPegSources::truncate_from(vec![
						PegSource::Value((1, 1)),
						PegSource::Value(peg2),
						PegSource::Value(peg3)
					]),
					max_target_update: max_peg_update,
					current: BoundedPegs::truncate_from(vec![(1, 1), peg2, peg3]),
				}
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				])
			));

			let pool_account = pool_account(pool_id);

			//let pool_liquid_a = Tokens::free_balance(asset_a, &pool_account);
			//let pool_liquid_b = Tokens::free_balance(asset_b, &pool_account);
			//let pool_liquid_c = Tokens::free_balance(asset_c, &pool_account);

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				199999950445584,
				250 * ONE,
			));

			assert_balance!(BOB, asset_a, 0);
			assert_balance!(BOB, asset_b, 199999950445584);
		});
}

#[test]
fn sell_with_peg_with_fee_should_work_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let trade_fee = Permill::from_float(0.01);

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = (u128::MAX, 1);

	let peg2_fixed = FixedU128::from_rational(peg2.0, peg2.1);
	let peg3_fixed = FixedU128::from_rational(peg3.0, peg3.1);
	let p1 = peg2_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p2 = FixedU128::one() / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p3 = peg3_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let liquid_a = p1.saturating_mul_int(tvl);
	let liquid_b = p2.saturating_mul_int(tvl);
	let liquid_c = p3.saturating_mul_int(tvl);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b, asset_c],
				amp,
				trade_fee,
				PoolPegInfo {
					source: BoundedPegSources::truncate_from(vec![
						PegSource::Value((1, 1)),
						PegSource::Value(peg2),
						PegSource::Value(peg3)
					]),
					max_target_update: max_peg_update,
					current: BoundedPegs::truncate_from(vec![(1, 1), peg2, peg3]),
				}
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				])
			));

			let pool_account = pool_account(pool_id);

			//let pool_liquid_a = Tokens::free_balance(asset_a, &pool_account);
			//let pool_liquid_b = Tokens::free_balance(asset_b, &pool_account);
			//let pool_liquid_c = Tokens::free_balance(asset_c, &pool_account);

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				100 * ONE,
				25 * ONE,
			));

			assert_balance!(BOB, asset_a, 0);
			assert_balance!(BOB, asset_b, 197999950941129);
		});
}

#[test]
fn buy_with_peg_with_fee_should_work_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let trade_fee = Permill::from_float(0.01);

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = (u128::MAX, 1);

	let peg2_fixed = FixedU128::from_rational(peg2.0, peg2.1);
	let peg3_fixed = FixedU128::from_rational(peg3.0, peg3.1);
	let p1 = peg2_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p2 = FixedU128::one() / (peg2_fixed + peg3_fixed + FixedU128::one());
	let p3 = peg3_fixed / (peg2_fixed + peg3_fixed + FixedU128::one());
	let liquid_a = p1.saturating_mul_int(tvl);
	let liquid_b = p2.saturating_mul_int(tvl);
	let liquid_c = p3.saturating_mul_int(tvl);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE + 1),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b, asset_c],
				amp,
				trade_fee,
				PoolPegInfo {
					source: BoundedPegSources::truncate_from(vec![
						PegSource::Value((1, 1)),
						PegSource::Value(peg2),
						PegSource::Value(peg3)
					]),
					max_target_update: max_peg_update,
					current: BoundedPegs::truncate_from(vec![(1, 1), peg2, peg3]),
				}
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				])
			));

			let pool_account = pool_account(pool_id);

			//let pool_liquid_a = Tokens::free_balance(asset_a, &pool_account);
			//let pool_liquid_b = Tokens::free_balance(asset_b, &pool_account);
			//let pool_liquid_c = Tokens::free_balance(asset_c, &pool_account);

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				197999950941129,
				250 * ONE,
			));

			assert_balance!(BOB, asset_a, 10000247746);
			assert_balance!(BOB, asset_b, 197999950941129);
		});
}
