use num_traits::Zero;
use sp_arithmetic::{FixedPointNumber, FixedU128, Perbill, Permill};
use std::num::NonZeroU128;

use crate::staking::*;

const ONE: u128 = 1_000_000_000_000;

#[test]
fn calculate_accumulated_rps_should_work_when_total_stake_is_not_zero() {
	let rps_now = FixedU128::checked_from_rational(1_234_512_341_u128, 10_000_000_u128).unwrap();

	assert_eq!(
		calculate_accumulated_rps(rps_now, 10_000 * ONE, 534_232 * ONE).unwrap(),
		FixedU128::from_inner(123_469_952_559_395_917_878_u128)
	);

	assert_eq!(
		calculate_accumulated_rps(rps_now, 10_000_000_000 * ONE, 987_886_878 * ONE).unwrap(),
		FixedU128::from_inner(133_573_850_588_484_220_963_u128)
	);

	assert_eq!(
		calculate_accumulated_rps(FixedU128::from(0_u128), 434_000 * ONE, 200 * ONE).unwrap(),
		FixedU128::from(2_170_u128)
	);

	assert_eq!(calculate_accumulated_rps(rps_now, 0, 534_232 * ONE).unwrap(), rps_now);

	// rps should not change when rewards are too small
	assert_eq!(
		calculate_accumulated_rps(rps_now, 1_000_u128, 5_000_000_000_000_000_000_000_u128).unwrap(),
		rps_now
	);
}

#[test]
fn calculate_slashed_points_should_work_when_pramas_stake_weight_is_not_zero() {
	let points = 10_000_000;

	//weight is one so it should be slashed 1:1 => 100% slash
	assert_eq!(
		calculate_slashed_points(points, 1_000 * ONE, 1_000 * ONE, 1_u8).unwrap(),
		points
	);

	//weight is 2 so it should be slashed 1:2  => 50% slash
	assert_eq!(
		calculate_slashed_points(points, 1_000 * ONE, 1_000 * ONE, 2_u8).unwrap(),
		points / 2
	);

	//100% slash becasue big stake increase
	assert_eq!(
		calculate_slashed_points(points, 1_000 * ONE, 1_000_000 * ONE, 2_u8).unwrap(),
		points
	);

	//small slash because of big current stake and small increase
	assert_eq!(
		calculate_slashed_points(points, 10_000_000 * ONE, ONE, 1_u8).unwrap(),
		1
	);

	//no points in the first place so nothing to slash
	assert_eq!(
		calculate_slashed_points(0, 1_000 * ONE, 1_000_000_000 * ONE, 1_u8).unwrap(),
		0
	);
}

#[test]
fn calculate_period_number_should_work_when_period_length_is_not_zero() {
	assert_eq!(
		calculate_period_number(NonZeroU128::try_from(1_u128).unwrap(), 12_341_u128),
		12_341_u128
	);

	assert_eq!(
		calculate_period_number(NonZeroU128::try_from(1_000_u128).unwrap(), 12_341_u128,),
		12_u128
	);

	assert_eq!(
		calculate_period_number(NonZeroU128::try_from(1_000_u128).unwrap(), 1_u128),
		0_u128
	);

	assert_eq!(
		calculate_period_number(NonZeroU128::try_from(82_u128).unwrap(), 12_341_u128),
		150_u128
	);
}

#[test]
fn calculate_points_should_work() {
	let time_points_per_period = 2_u8;

	let action_points = 100_u128;
	assert_eq!(
		calculate_points(
			39_u128,
			42_u128,
			time_points_per_period,
			Permill::from_percent(60),
			action_points,
			Perbill::from_percent(40),
			0
		)
		.unwrap(),
		43
	);

	let action_points = 0_u128;
	assert_eq!(
		calculate_points(
			40_u128,
			82_u128,
			time_points_per_period,
			Permill::from_percent(60),
			action_points,
			Perbill::from_percent(40),
			0
		)
		.unwrap(),
		50
	);

	let action_points = 1_000_000_u128;
	assert_eq!(
		calculate_points(
			150_u128,
			192_u128,
			time_points_per_period,
			Permill::from_percent(80),
			action_points,
			Perbill::from_percent(10),
			200
		)
		.unwrap(),
		99_867
	);
}

#[test]
fn sigmoid_should_work() {
	let a = FixedU128::from_inner(8_000_000_000_000_000);
	let b = 2;

	assert_eq!(sigmoid(0, a, b).unwrap(), FixedU128::zero());

	assert_eq!(sigmoid(1, a, b).unwrap(), FixedU128::from_inner(2_047_999_995_u128));

	assert_eq!(
		sigmoid(10, a, b).unwrap(),
		FixedU128::from_inner(20_479_580_578_189_u128)
	);

	assert_eq!(
		sigmoid(538, a, b).unwrap(),
		FixedU128::from_inner(994_205_484_888_725_524_u128)
	);

	assert_eq!(
		sigmoid(1_712_904, a, b).unwrap(),
		FixedU128::from_inner(999_999_999_999_999_943_u128)
	);

	let a = FixedU128::from_inner(250_000_000_000_000_000);
	let b = 9_340_000;

	assert_eq!(sigmoid(0, a, b).unwrap(), FixedU128::zero());

	assert_eq!(sigmoid(1, a, b).unwrap(), FixedU128::from_inner(418_228_051_u128));

	assert_eq!(
		sigmoid(10, a, b).unwrap(),
		FixedU128::from_inner(4_182_263_022_521_u128)
	);

	assert_eq!(
		sigmoid(538, a, b).unwrap(),
		FixedU128::from_inner(972_251_695_722_892_328_u128)
	);

	assert_eq!(
		sigmoid(500_000, a, b).unwrap(),
		FixedU128::from_inner(999_999_999_999_961_743_u128)
	);
}

#[test]
fn calculate_percentage_amount_should_work() {
	assert_eq!(
		calculate_percentage_amount(3_000_000_u128, FixedU128::from_float(0.5)),
		1_500_000_u128
	);

	assert_eq!(calculate_percentage_amount(3_000_000_u128, FixedU128::from(0)), 0_u128);

	assert_eq!(
		calculate_percentage_amount(3_000_000_u128, FixedU128::from(1)),
		3_000_000_u128
	);

	assert_eq!(
		calculate_percentage_amount(3_000_000_u128, FixedU128::from_float(0.13264959)),
		397_948_u128
	);
}

#[test]
fn calculate_rewards_should_work() {
	let accumulated_rps = FixedU128::from_inner(23_423_523_230_000_000_000);
	let rps = FixedU128::from_inner(23_423_000_000_000_000_000);
	let amount = 1_000 * ONE;
	assert_eq!(
		calculate_rewards(accumulated_rps, rps, amount).unwrap(),
		523_230_000_000_u128
	);

	let accumulated_rps = FixedU128::from_inner(23_423_523_230_000_000_000);
	let rps = FixedU128::from_inner(19_423_000_000_000_000_000);
	let amount = 1_000 * ONE;
	assert_eq!(
		calculate_rewards(accumulated_rps, rps, amount).unwrap(),
		4_000_523_230_000_000_u128
	);
}
