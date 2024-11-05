#![allow(clippy::excessive_precision)]

use crate::assert_balance;
use crate::tests::mock::*;
use crate::types::{AssetAmount, PoolInfo};
use frame_support::assert_ok;
use hydradx_traits::router::PoolType;
use hydradx_traits::router::TradeExecution;
use orml_traits::MultiCurrencyExtended;
use proptest::prelude::ProptestConfig;
use proptest::proptest;
use proptest::strategy::Strategy;
use sp_runtime::FixedPointNumber;
use sp_runtime::{FixedU128, Permill};
use std::num::NonZeroU16;
use test_utils::assert_eq_approx;

#[test]
fn spot_price_calculation_should_work_when_asset_in_is_share_with_6_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 50 * ONE), (ALICE, 1, 50 * ONE), (ALICE, 2, 50 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 6)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![AssetAmount::new(asset_a, 40 * ONE), AssetAmount::new(asset_b, 40 * ONE)],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let bob_share_balance = 20 * ONE;
			Tokens::update_balance(pool_id, &BOB, bob_share_balance as i128).unwrap();

			let sell_amount = 10 * ONE;
			let total_issuance = Tokens::total_issuance(pool_id);
			let initial_issuance = 80000000000020000000000000;
			assert_eq!(total_issuance, initial_issuance);

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				pool_id,
				asset_b,
				sell_amount,
				0,
			));

			let expected = 9;

			assert_balance!(BOB, pool_id, bob_share_balance - sell_amount);
			assert_balance!(BOB, asset_b, expected);
			let total_issuance = Tokens::total_issuance(pool_id);
			assert_eq!(total_issuance, initial_issuance - sell_amount);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), pool_id, asset_b).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = calculated_amount_out - expected;
			let relative_difference = FixedU128::from_rational(difference, expected);
			let tolerated_difference = FixedU128::from_rational(1, 1000);
			// The difference of the amount out calculated with spot price should be less than 0.1%
			assert_eq!(relative_difference, FixedU128::from_float(0.000000000000000000));
			assert!(relative_difference < tolerated_difference);
		});
}

#[test]
fn spot_price_calculation_should_work_when_asset_in_is_share_with_12_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 5000 * ONE),
			(ALICE, 1, 5000 * ONE),
			(ALICE, 2, 5000 * ONE),
		])
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
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 4000 * ONE),
					AssetAmount::new(asset_b, 4000 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let bob_share_balance = 100 * ONE;
			Tokens::update_balance(pool_id, &BOB, bob_share_balance as i128).unwrap();

			let sell_amount = ONE;
			let total_issuance = Tokens::total_issuance(pool_id);
			let initial_issuance = 8000000100000000000000;
			assert_eq!(total_issuance, initial_issuance);

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				pool_id,
				asset_b,
				sell_amount,
				0,
			));

			let expected = 994999;

			assert_balance!(BOB, pool_id, bob_share_balance - sell_amount);
			assert_balance!(BOB, asset_b, expected);
			let total_issuance = Tokens::total_issuance(pool_id);
			assert_eq!(total_issuance, initial_issuance - sell_amount);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), pool_id, asset_b).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = if calculated_amount_out > expected {
				calculated_amount_out - expected
			} else {
				expected - calculated_amount_out
			};
			let relative_difference = FixedU128::from_rational(difference, expected);
			let tolerated_difference = FixedU128::from_rational(1, 100);
			// The difference of the amount out calculated with spot price should be less than 1%
			assert_eq!(relative_difference, FixedU128::from_float(0.001021106553875934));
			assert!(relative_difference < tolerated_difference);
		});
}

#[test]
fn spot_price_calculation_should_work_when_asset_in_is_share_with_18_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200000000 * ONE),
			(ALICE, 1, 200000000 * ONE),
			(ALICE, 2, 200000000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 18)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 18)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100000000 * ONE),
					AssetAmount::new(asset_b, 100000000 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let bob_share_balance = 100000 * ONE;
			Tokens::update_balance(pool_id, &BOB, bob_share_balance as i128).unwrap();

			let sell_amount = 1000;
			let total_issuance = Tokens::total_issuance(pool_id);
			let initial_issuance = 200100000000000000000;
			assert_eq!(total_issuance, initial_issuance);

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				pool_id,
				asset_b,
				sell_amount,
				0,
			));

			let expected = 991;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, pool_id, bob_share_balance - sell_amount);
			assert_balance!(BOB, asset_b, expected);
			let total_issuance = Tokens::total_issuance(pool_id);
			assert_eq!(total_issuance, initial_issuance - sell_amount);
			assert_balance!(pool_account, asset_b, 100000000 * ONE - expected);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), pool_id, asset_b).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = if expected > calculated_amount_out {
				expected - calculated_amount_out
			} else {
				calculated_amount_out - expected
			};
			let relative_difference = FixedU128::from_rational(difference, expected);
			let tolerated_difference = FixedU128::from_rational(1, 100);
			// The difference of the amount out calculated with spot price should be less than 1%
			assert_eq_approx!(
				relative_difference,
				FixedU128::from_float(0.004036326942482341),
				FixedU128::from((2, (ONE / 10_000))),
				"the relative difference is not as expected"
			);
			assert!(relative_difference < tolerated_difference);
		});
}

#[test]
fn spot_price_calculation_should_work_when_asset_out_is_share_with_12_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
			(ALICE, 3, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_registered_asset("three".as_bytes().to_vec(), 3, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(3),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 150 * ONE),
					AssetAmount::new(asset_b, 150 * ONE),
					AssetAmount::new(asset_c, 150 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let sell_amount = 1_000;
			let total_issuance = Tokens::total_issuance(pool_id);
			let initial_issuance = 450000000000000000000;
			assert_eq!(total_issuance, initial_issuance);

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				asset_a,
				pool_id,
				sell_amount,
				0,
			));

			let expected = 986999999;

			assert_balance!(BOB, asset_a, 200 * ONE - sell_amount);
			assert_balance!(BOB, pool_id, expected);
			let total_issuance = Tokens::total_issuance(pool_id);
			assert_eq!(total_issuance, initial_issuance + expected);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), asset_a, pool_id).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = if calculated_amount_out > expected {
				calculated_amount_out - expected
			} else {
				expected - calculated_amount_out
			};
			let relative_difference = FixedU128::from_rational(difference, expected);

			// The difference of the amount out calculated with spot price should be less than 0.1%
			let tolerated_difference = FixedU128::from_rational(1, 1000);
			assert_eq!(relative_difference, FixedU128::from_float(0.000000000000000000));

			assert!(relative_difference < tolerated_difference);
		});
}

#[test]
fn spot_price_calculation_should_work_when_asset_out_is_share_with_18_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200000000 * ONE),
			(ALICE, 1, 200000000 * ONE),
			(ALICE, 2, 200000000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 18)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 18)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(80).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(3),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100000000 * ONE),
					AssetAmount::new(asset_b, 150000000 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let sell_amount = 1_000_000_000_000_000;
			let total_issuance = Tokens::total_issuance(pool_id);
			let initial_issuance = 249935748846477223906;
			assert_eq!(total_issuance, initial_issuance);

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				asset_a,
				pool_id,
				sell_amount,
				0,
			));

			let expected = 984938537439245;

			assert_balance!(BOB, asset_a, 200000000 * ONE - sell_amount);
			assert_balance!(BOB, pool_id, expected);
			let total_issuance = Tokens::total_issuance(pool_id);
			assert_eq!(total_issuance, initial_issuance + expected);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), asset_a, pool_id).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = if calculated_amount_out > expected {
				calculated_amount_out - expected
			} else {
				expected - calculated_amount_out
			};
			let relative_difference = FixedU128::from_rational(difference, expected);
			// The difference of the amount out calculated with spot price should be less than 1%
			let tolerated_difference = FixedU128::from_rational(1, 1000);
			assert_eq!(relative_difference, FixedU128::from_float(0.000952889346461269));
			assert!(relative_difference < tolerated_difference);
		});
}

#[test]
fn spot_price_calculation_should_work_for_two_stableassets() {
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
				fee: Permill::from_percent(1),
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

			let sell_amount = 1_000_000_000;

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				asset_a,
				asset_b,
				sell_amount,
				0,
			));

			let expected = 989999901;

			assert_balance!(BOB, asset_a, 200 * ONE - sell_amount);
			assert_balance!(BOB, asset_b, expected);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), asset_a, asset_b).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = expected - calculated_amount_out;
			let relative_difference = FixedU128::from_rational(difference, expected);
			let tolerated_difference = FixedU128::from_rational(1, 1000);
			// The difference of the amount out calculated with spot price should be less than 0.1%
			assert_eq_approx!(
				relative_difference,
				FixedU128::from_float(0.000000097979807778),
				FixedU128::from((2, (ONE / 10_000))),
				"the relative difference is not as expected"
			);

			assert!(relative_difference < tolerated_difference);
		});
}

#[test]
fn spot_price_calculation_should_work_for_two_stableassets_on_different_positions() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, asset_c, 200 * ONE),
			(ALICE, 1, 1000 * ONE),
			(ALICE, 2, 1000 * ONE),
			(ALICE, 3, 1000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_registered_asset("thr".as_bytes().to_vec(), 3, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(2),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 500 * ONE),
					AssetAmount::new(asset_c, 900 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let sell_amount = 1_000_000_000;

			assert_ok!(Stableswap::execute_sell(
				RuntimeOrigin::signed(BOB),
				PoolType::Stableswap(pool_id),
				asset_c,
				asset_b,
				sell_amount,
				0,
			));

			let expected = 968488820;

			assert_balance!(BOB, asset_c, 200 * ONE - sell_amount);
			assert_balance!(BOB, asset_b, expected);

			let spot_price =
				Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), asset_c, asset_b).unwrap();

			//Check if spot price calculation is correct
			let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
			let difference = if expected > calculated_amount_out {
				expected - calculated_amount_out
			} else {
				calculated_amount_out - expected
			};
			let relative_difference = FixedU128::from_rational(difference, expected);
			let tolerated_difference = FixedU128::from_rational(1, 1000);
			assert_eq_approx!(
				relative_difference,
				FixedU128::from_float(0.000000038203848342),
				FixedU128::from((2, (ONE / 10_000))),
				"the relative difference is not as expected"
			);
			assert!(relative_difference < tolerated_difference);
		});
}

mod invariants {
	use super::*;

	fn decimals() -> impl Strategy<Value = u8> {
		6..18u8
	}

	proptest! {
		#![proptest_config(ProptestConfig::with_cases(100))]
		#[test]
		fn spot_price_between_share_and_asset(
			decimal in decimals()
		) {
		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, 1, 50000000 * ONE),
				(ALICE, 1, 50000000 * ONE),
				(ALICE, 2, 50000000 * ONE),
			])
			.with_registered_asset("one".as_bytes().to_vec(), 1, decimal)
			.with_registered_asset("two".as_bytes().to_vec(), 2, decimal)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(1),
				},
				InitialLiquidity {
					account: ALICE,
					assets: vec![
						AssetAmount::new(asset_a, 40000000 * ONE),
						AssetAmount::new(asset_b, 40000000 * ONE),
					],
				},
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);

				let bob_share_balance = 200000 * ONE;
				Tokens::update_balance(pool_id, &BOB, bob_share_balance as i128).unwrap();

				let sell_amount = 1000 * ONE;

				assert_ok!(Stableswap::execute_sell(
					RuntimeOrigin::signed(BOB),
					PoolType::Stableswap(pool_id),
					pool_id,
					asset_b,
					sell_amount,
					0,
				));

				assert_balance!(BOB, pool_id, bob_share_balance - sell_amount);
				let expected = Tokens::free_balance(asset_b, &BOB);

				let spot_price =
					Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), pool_id, asset_b)
						.unwrap();

				//Check if spot price calculation is correct
				let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
				let difference = if calculated_amount_out > expected  {
					calculated_amount_out - expected
				} else {
					expected - calculated_amount_out
				};
				let relative_difference = FixedU128::from_rational(difference, expected);
				let tolerated_difference = FixedU128::from_rational(1, 100);
				// The difference of the amount out calculated with spot price should be less than 1%
				assert!(relative_difference < tolerated_difference, "relative difference: {:?}, tolerated difference: {:?}", relative_difference, tolerated_difference);
			});
		}
	}

	proptest! {
		#![proptest_config(ProptestConfig::with_cases(100))]
		#[test]
		fn spot_price_between_asset_and_share(
			decimal in decimals()
		) {
		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		ExtBuilder::default()
			.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
			.with_registered_asset("one".as_bytes().to_vec(), 1, decimal)
			.with_registered_asset("two".as_bytes().to_vec(), 2, decimal)
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
						AssetAmount::new(asset_a, 150 * ONE),
						AssetAmount::new(asset_b, 150 * ONE),
					],
				},
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);

				let sell_amount = 10 * ONE;

				assert_ok!(Stableswap::execute_sell(
					RuntimeOrigin::signed(BOB),
					PoolType::Stableswap(pool_id),
					asset_a,
					pool_id,
					sell_amount,
					0,
				));


				assert_balance!(BOB, asset_a, 200 * ONE - sell_amount);
				let expected = Tokens::free_balance(pool_id, &BOB);

				let spot_price =
					Stableswap::calculate_spot_price_with_fee(PoolType::Stableswap(pool_id), asset_a, pool_id)
						.unwrap();

				//Check if spot price calculation is correct
				let calculated_amount_out = spot_price.reciprocal().unwrap().checked_mul_int(sell_amount).unwrap();
				let difference = if calculated_amount_out > expected {
					calculated_amount_out - expected
				} else {
					expected - calculated_amount_out
				};
				let relative_difference = FixedU128::from_rational(difference, expected);
				let tolerated_difference = FixedU128::from_rational(1, 100);
				// The difference of the amount out calculated with spot price should be less than 1%
				assert!(relative_difference < tolerated_difference);
			});
		}
	}
}
