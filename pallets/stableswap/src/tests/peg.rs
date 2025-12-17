#![allow(deprecated)]

use crate::assert_balance;
use crate::tests::mock::*;
use crate::types::{BoundedPegSources, PegSource};
use frame_support::traits::Hooks;
use hydradx_traits::stableswap::AssetAmount;

use crate::tests::{get_share_price, spot_price, to_bounded_asset_vec};
use frame_support::{assert_ok, BoundedVec};
use hydradx_traits::OraclePeriod;
use num_traits::One;
use sp_runtime::{FixedPointNumber, FixedU128, Perbill, Permill};
use test_utils::assert_eq_approx;

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

	let max_peg_update = Perbill::from_percent(100);

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
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value(peg2),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

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

	let max_peg_update = Perbill::from_percent(100);

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
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value(peg2),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			assert_balance!(BOB, asset_b, 0);
			let bob_a_initial = Tokens::free_balance(asset_a, &BOB);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				199999950445584,
				250 * ONE,
			));

			let bob_a_final = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(bob_a_initial - bob_a_final, 100_000_000_000_001);
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

	let max_peg_update = Perbill::from_percent(100);

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
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				trade_fee,
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value(peg2),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

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

	let max_peg_update = Perbill::from_percent(100);

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
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				trade_fee,
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value(peg2),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

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

#[test]
fn sell_with_drifting_peg_should_work() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

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
			assert_balance!(BOB, asset_b, 190961826574751);
		});
}

#[test]
fn sell_with_drifting_peg_should_not_exceed_max_peg_update() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(1);

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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

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
			assert_balance!(BOB, asset_b, 197936260103269);
			let pegs = Stableswap::pool_peg_info(pool_id).unwrap();
			assert_eq!(pegs.current.to_vec(), vec![(1, 1), (1980000000, 4000000000), (1, 3)]);
		});
}

#[test]
fn share_pries_should_be_correct_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			let share0 = get_share_price(pool_id, 0);
			assert_eq!(share0, FixedU128::from_float(0.000001000748975604));

			let share1 = get_share_price(pool_id, 1);
			assert_eq!(share1, FixedU128::from_float(0.000002001497951207));

			let share2 = get_share_price(pool_id, 2);
			assert_eq!(share2, FixedU128::from_float(0.000002988112571400));
		});
}

#[test]
fn spot_prices_should_be_correct_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			let spot1 = spot_price(pool_id, asset_a, asset_b);
			assert_eq_approx!(
				spot1,
				FixedU128::from_float(0.5),
				FixedU128::from_float(0.0000000001),
				"spot price not equal"
			);
			let spot2 = spot_price(pool_id, asset_a, asset_c);
			assert_eq_approx!(
				spot2,
				FixedU128::from_float(0.334_910_065_029_717_1),
				FixedU128::from_float(0.0000000001),
				"spot price not equal"
			);
			let spot2 = spot_price(pool_id, asset_c, asset_a);
			assert_eq_approx!(
				spot2,
				FixedU128::from_float(2.985_876_222_953_372_4),
				FixedU128::from_float(0.0000000001),
				"spot price not equal"
			);
		});
}

#[test]
fn add_liquidity_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			(BOB, asset_b, 10 * ONE),
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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10 * ONE),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465477960257850); //same as python
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);
		});
}

#[test]
fn add_liquidity_shares_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			(BOB, asset_b, 10 * ONE),
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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				4713465477960257850,
				asset_b,
				u128::MAX,
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465477960257850);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 12261858024);
		});
}

#[test]
fn remove_liquidity_for_one_asset_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			(BOB, asset_b, 10 * ONE),
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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10 * ONE),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465477960257850);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				4713465477960257850,
				0,
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 0);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 9311925821564); // not same as python because fees are not recalculated in same block
		});
}

#[test]
fn remove_liquidity_given_asset_amount_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			(BOB, asset_b, 10 * ONE),
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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10 * ONE),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465477960257850);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				9 * ONE,
				u128::MAX,
			));

			let bob_shares_left = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares_left, 163475039151683765);
			let bob_shares_used = 4713465477960257850u128 - Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares_used, 4_549_990_438_808_574_085); //not same as python becasue fees are
														   //not recalculated in same block
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 9 * ONE);
		});
}

#[test]
fn remove_liquidity_uniform_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;

	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			(BOB, asset_b, 10 * ONE),
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
			System::set_block_number(1);
			set_peg_oracle_value(asset_a, asset_b, peg2, 1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(asset_a, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10 * ONE),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465477960257850);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				bob_shares,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, 0),
					AssetAmount::new(asset_b, 0),
					AssetAmount::new(asset_c, 0)
				])
			));

			let bob_shares_left = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares_left, 0);
			let bob_a_balance = Tokens::free_balance(asset_a, &BOB);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			let bob_c_balance = Tokens::free_balance(asset_c, &BOB);
			assert_eq!(bob_a_balance, 2_121_636_255_142);
			assert_eq!(bob_b_balance, 4_243_311_406_949);
			assert_eq!(bob_c_balance, 1_414_424_170_095); // same as python
		});
}

#[test]
fn asset_oracle_peg_should_work() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let oracle_asset: AssetId = 4; // Special asset used for oracle pegging
	let pool_id = 100;

	let amp = 1000;
	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

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
			(ALICE, asset_a, 2_000_000 * ONE),
			(ALICE, asset_b, 2_000_000 * ONE),
			(ALICE, asset_c, 2_000_000 * ONE),
			(BOB, asset_a, 1_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("oracle_asset".as_bytes().to_vec(), oracle_asset, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			// Set up oracle with asset_a
			set_peg_oracle_value(oracle_asset, asset_b, peg2, 1);

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, oracle_asset)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			// Change the oracle price using the oracle_asset instead of first_asset
			set_peg_oracle_value(oracle_asset, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				100 * ONE,
				25 * ONE,
			));

			assert_balance!(BOB, asset_a, 900000000000000);
			assert_balance!(BOB, asset_b, 190961826574751);
			let pegs = Stableswap::pool_peg_info(pool_id).unwrap();
			assert_eq!(pegs.current.to_vec(), vec![(1, 1), (192, 400), (1, 3)]);
		});
}

#[test]
fn pegs_should_not_change_when_multiple_trades_happen_in_same_block() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let oracle_asset: AssetId = 4; // Special asset used for oracle pegging
	let pool_id = 100;

	let amp = 1000;
	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_parts(10_000);

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
			(ALICE, asset_a, 2_000_000 * ONE),
			(ALICE, asset_b, 2_000_000 * ONE),
			(ALICE, asset_c, 2_000_000 * ONE),
			(BOB, asset_a, 1_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("oracle_asset".as_bytes().to_vec(), oracle_asset, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			// Set up oracle with asset_a
			set_peg_oracle_value(oracle_asset, asset_b, peg2, 1);

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, oracle_asset)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			set_peg_oracle_value(oracle_asset, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				50 * ONE,
				15 * ONE,
			));

			let pegs_0 = Stableswap::pool_peg_info(pool_id).expect("pool pegs should exists");

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				50 * ONE,
				10 * ONE,
			));

			assert_eq!(
				Stableswap::pool_peg_info(pool_id).expect("pool pegs should exists"),
				pegs_0
			);
		});
}

#[test]
fn trade_fee_should_not_change_when_multiple_trades_happen_in_same_block() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let oracle_asset: AssetId = 4; // Special asset used for oracle pegging
	let pool_id = 100;

	let amp = 1000;
	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_parts(10_000);

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
			(ALICE, asset_a, 2_000_000 * ONE),
			(ALICE, asset_b, 2_000_000 * ONE),
			(ALICE, asset_c, 2_000_000 * ONE),
			(BOB, asset_a, 1_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("oracle_asset".as_bytes().to_vec(), oracle_asset, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			// Set up oracle with asset_a
			set_peg_oracle_value(oracle_asset, asset_b, peg2, 1);

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, oracle_asset)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			Stableswap::on_finalize(System::block_number());
			assert_eq!(Stableswap::block_fee(pool_id), None);

			set_peg_oracle_value(oracle_asset, asset_b, (48, 100), 2);
			System::set_block_number(2);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				50 * ONE,
				15 * ONE,
			));

			let trade_fee_1 = Stableswap::block_fee(pool_id);
			assert!(trade_fee_1.is_some());

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				50 * ONE,
				10 * ONE,
			));

			assert_eq!(Stableswap::block_fee(pool_id), trade_fee_1);
		});
}

#[test]
fn trade_fee_should_be_removed_on_finalize() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let oracle_asset: AssetId = 4; // Special asset used for oracle pegging
	let pool_id = 100;

	let amp = 1000;
	let tvl: u128 = 2_000_000 * ONE;

	let peg2 = (1, 2);
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_parts(10_000);

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
			(ALICE, asset_a, 2_000_000 * ONE),
			(ALICE, asset_b, 2_000_000 * ONE),
			(ALICE, asset_c, 2_000_000 * ONE),
			(BOB, asset_a, 1_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("oracle_asset".as_bytes().to_vec(), oracle_asset, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			// Set up oracle with asset_a
			set_peg_oracle_value(oracle_asset, asset_b, peg2, 1);

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((*b"testtest", OraclePeriod::Short, oracle_asset)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			assert!(Stableswap::block_fee(pool_id).is_some());

			//Act
			Stableswap::on_finalize(System::block_number());

			//Assert
			assert!(Stableswap::block_fee(pool_id).is_none());
		});
}
