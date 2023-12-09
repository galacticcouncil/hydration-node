use super::*;

use crate::fraction;
use crate::support::rational::Rounding;
use crate::test_utils::fraction_to_high_precision;
use crate::test_utils::{assert_approx_eq, assert_rational_approx_eq, assert_rational_relative_approx_eq};
use crate::transcendental::saturating_powi_high_precision;
use crate::types::Fraction;

mod high_precision;
mod invariants;
mod test_data;
use test_data::*;

use num_traits::One;
use primitive_types::{U128, U256, U512};
use rug::Rational;
use sp_arithmetic::{FixedPointNumber, FixedU128};

pub const TEN_MINUTES_PERIOD: u64 = 100;
pub const DAY_PERIOD: u64 = 14_400;
pub const WEEK_PERIOD: u64 = 100_800;

#[test]
fn saturating_sub_works() {
	assert_eq!(
		saturating_sub(EmaPrice::one(), EmaPrice::one()),
		(U256::zero(), U256::one())
	);
	assert_eq!(
		saturating_sub(EmaPrice::from(2), EmaPrice::one()),
		(U256::one(), U256::one())
	);
	assert_eq!(
		saturating_sub(EmaPrice::from(4), EmaPrice::new(30, 6)),
		(U256::zero(), U256::from(6))
	);
	assert_eq!(
		saturating_sub(EmaPrice::new(4, 5), EmaPrice::new(2, 3)),
		(U256::from(2), U256::from(15))
	);
	assert_eq!(
		saturating_sub(EmaPrice::new(1, 10), EmaPrice::new(2, u128::MAX)),
		(
			U256::from(U128::MAX - 20),
			U128::from(u128::MAX).full_mul(10_u128.into())
		)
	);
}

#[test]
fn round_to_rational_should_work() {
	let res = round_to_rational((U512::from(1), U512::from(1)), Rounding::Nearest);
	let expected = EmaPrice::new(1, 1);
	assert_eq!(res, expected,);

	let res = round_to_rational((U512::MAX, U512::MAX), Rounding::Nearest);
	let expected = EmaPrice::new(u128::MAX, u128::MAX);
	assert_eq!(res, expected,);

	let res = round_to_rational((U512::MAX, U512::from(1)), Rounding::Nearest);
	let expected = EmaPrice::new(u128::MAX, 1);
	assert_eq!(res, expected,);

	let res = round_to_rational((U512::from(1), U512::MAX), Rounding::Nearest);
	let expected = EmaPrice::new(1, u128::MAX);
	assert_eq!(res, expected,);

	let d = 323853616005226055489000679651893043332_u128;
	let res = round_to_rational(
		(
			U512::from_dec_str("34599284998074995708396179719034205723253966454380752564716172454912477882716")
				.unwrap(),
			U512::from(d),
		),
		Rounding::Down,
	);
	let boundary = Rational::from_str_radix(
		"34599284998074995708396179719034205723253966454380752564716172454912477882716",
		10,
	)
	.unwrap()
		/ d;
	assert!(Rational::from(res) <= boundary);
}

#[test]
fn weighted_averages_work_on_small_values_with_correct_ratios() {
	let smoothing = smoothing_from_period(7);

	// price
	let tolerance = Rational::from((100, u128::MAX));
	let start_price = EmaPrice::new(4, 1);
	let incoming_price = EmaPrice::new(8, 1);
	let next_price = price_weighted_average(start_price, incoming_price, smoothing);
	let expected = Rational::from((5, 1));
	// oracle should be biased towards previous value
	assert!(Rational::from(next_price) < expected);
	assert_rational_approx_eq!(Rational::from(next_price), expected, tolerance);

	let tolerance = Rational::from((100, u128::MAX));
	let start_price = EmaPrice::new(4, 100);
	let incoming_price = EmaPrice::new(8, 100);
	let next_price = price_weighted_average(start_price, incoming_price, smoothing);
	let expected = Rational::from((5, 100));
	// oracle should be biased towards previous value
	assert!(Rational::from(next_price) < expected);
	assert_rational_approx_eq!(Rational::from(next_price), expected, tolerance);

	let start_price = EmaPrice::new(8, 1);
	let incoming_price = EmaPrice::new(4, 1);
	let next_price = price_weighted_average(start_price, incoming_price, smoothing);
	let expected = Rational::from((7, 1));
	// oracle should be biased towards previous value
	assert!(Rational::from(next_price) > expected);
	assert_rational_approx_eq!(Rational::from(next_price), expected, tolerance);

	let start_price = EmaPrice::new(8, 100);
	let incoming_price = EmaPrice::new(4, 100);
	let next_price = price_weighted_average(start_price, incoming_price, smoothing);
	let expected = Rational::from((7, 100));
	// oracle should be biased towards previous value
	assert!(Rational::from(next_price) > expected);
	assert_rational_approx_eq!(Rational::from(next_price), expected, tolerance);

	// balance
	let start_balance = 4u128;
	let incoming_balance = 8u128;
	let next_balance = balance_weighted_average(start_balance, incoming_balance, smoothing);
	assert_eq!(next_balance, 5u128);

	// volume
	let start_volume = (4u128, 1u128, 8u128, 0u128);
	let incoming_volume = (8u128, 1u128, 4u128, 0u128);
	let next_volume = volume_weighted_average(start_volume, incoming_volume, smoothing);
	assert_eq!(next_volume, (5u128, 1u128, 7u128, 0u128));

	// liquidity
	let start_liquidity = (4u128, 8u128);
	let incoming_liquidity = (8u128, 4u128);
	let next_liquidity = liquidity_weighted_average(start_liquidity, incoming_liquidity, smoothing);
	assert_eq!(next_liquidity, (5u128, 7u128));
}

#[test]
fn balance_weighted_averages_work_on_typical_values_with_minutes_smoothing() {
	let smoothing = smoothing_from_period(TEN_MINUTES_PERIOD);
	let start_balance = 4_000_000_000_000u128;
	let incoming_balance = 8_000_000_000_000u128;
	let next_balance = balance_weighted_average(start_balance, incoming_balance, smoothing);
	let expected_balance: Rational =
		start_balance + Rational::from((incoming_balance - start_balance, 1)) * 2 / (TEN_MINUTES_PERIOD + 1);
	assert_eq!(next_balance, expected_balance.round());
}

#[test]
fn balance_weighted_averages_work_on_typical_values_with_day_smoothing() {
	let smoothing = smoothing_from_period(DAY_PERIOD);
	let start_balance = 4_000_000_000_000u128;
	let incoming_balance = 8_000_000_000_000u128;
	let next_balance = balance_weighted_average(start_balance, incoming_balance, smoothing);
	let expected: Rational =
		start_balance + Rational::from((incoming_balance - start_balance, 1)) * 2 / (DAY_PERIOD + 1);
	let tolerance = 1;
	let expected_balance = expected.round();
	assert_approx_eq!(
		next_balance,
		expected_balance,
		tolerance,
		"averaged balance values should be within 1 of the expected value"
	);
}

#[test]
fn balance_weighted_averages_work_on_typical_values_with_week_smoothing() {
	let smoothing = smoothing_from_period(WEEK_PERIOD);
	let start_balance = 4_000_000_000_000u128;
	let incoming_balance = 8_000_000_000_000u128;
	let next_balance = balance_weighted_average(start_balance, incoming_balance, smoothing);
	let expected_balance: Rational =
		start_balance + Rational::from((incoming_balance - start_balance, 1)) * 2 / (WEEK_PERIOD + 1);
	assert_eq!(next_balance, expected_balance.round());
}

#[test]
fn price_weighted_average_boundary_values() {
	let smoothing = fraction::frac(1, 2);

	let tolerance = Rational::from((1, 1e30 as u128));
	let max_price = EmaPrice::new(1_000_000_000_000_000_000_000_000_u128, 1); // 1e24
	let half_max_price = EmaPrice::new(1_000_000_000_000_000_000_000_000_u128, 2); // 1e24 / 2

	// previously zero, incoming max
	let next_price = price_weighted_average(EmaPrice::zero(), max_price, smoothing);
	assert_rational_relative_approx_eq!(Rational::from(next_price), Rational::from(half_max_price), tolerance);
	// previously max, incoming zero
	let next_price = price_weighted_average(max_price, EmaPrice::zero(), smoothing);
	assert_rational_relative_approx_eq!(Rational::from(next_price), Rational::from(half_max_price), tolerance);

	// we can only guarantee 14 digits of precision for extreme values
	let tolerance = Rational::from((1, 1e14 as u128));
	let max_price = EmaPrice::new(1_000_000_000_000_000_000_000_001_u128, 1); // 1e24 + 1
	let half_max_price = EmaPrice::new(1_000_000_000_000_000_000_000_002_u128, 2); // (1e24 + 2) / 2

	// previously one, incoming max
	let next_price = price_weighted_average(EmaPrice::one(), max_price, smoothing);
	assert_rational_relative_approx_eq!(Rational::from(next_price), Rational::from(half_max_price), tolerance);
	// previously max, incoming one
	let next_price = price_weighted_average(max_price, EmaPrice::one(), smoothing);
	assert_rational_relative_approx_eq!(Rational::from(next_price), Rational::from(half_max_price), tolerance);
}

#[test]
fn balance_weighed_average_does_not_saturate_on_big_balances() {
	let smoothing = Fraction::one();
	let start_balance = u128::MAX;
	let incoming_balance = u128::MAX;
	let next_balance = balance_weighted_average(start_balance, incoming_balance, smoothing);
	assert_eq!(next_balance, incoming_balance);
}

#[test]
fn exp_smoothing_works() {
	let smoothing = smoothing_from_period(7);
	let alpha = exp_smoothing(smoothing, 10);
	let expected_complement: Fraction = saturating_powi_high_precision(fraction::frac(3, 4), 10);
	assert_eq!(alpha, Fraction::one() - expected_complement);
}

#[test]
fn smoothing_from_period_works() {
	let period = 0;
	assert_eq!(smoothing_from_period(period), Fraction::one());

	let period = 3;
	assert_eq!(smoothing_from_period(period), fraction::frac(1, 2));

	let period = 999;
	assert_eq!(smoothing_from_period(period), fraction::frac(2, 1_000));
}

#[test]
fn exponential_smoothing_small_period() {
	let smoothing = Fraction::from_num(0.999);
	let iterations = 100_000;
	let exp = exp_smoothing(smoothing, iterations);
	let rug_exp = high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations);

	let tolerance = Rational::from((1, FixedU128::DIV));
	assert_rational_approx_eq!(
		fraction_to_high_precision(exp),
		rug_exp,
		tolerance,
		"high precision should be equal to low precision within low precision tolerance"
	);
}

#[test]
fn precision_of_ema_over_price_history_should_be_high_enough_in_crash_scenario() {
	let history = vec![
		(EmaPrice::zero(), 1),
		(EmaPrice::new(1e15 as u128, 1), invariants::MAX_ITERATIONS),
		(EmaPrice::new(1, 1e15 as u128), invariants::MAX_ITERATIONS),
	];
	let smoothing = smoothing_from_period(WEEK_PERIOD);
	let precise_ema = high_precision::precise_price_ema(history.clone(), fraction_to_high_precision(smoothing));

	let mut ema = history[0].0;
	for (price, iterations) in history.into_iter().skip(1) {
		ema = iterated_price_ema(iterations, ema, price, smoothing);
	}

	let tolerance = Rational::from((1, 1e20 as u128));
	assert_rational_relative_approx_eq!(
		Rational::from(ema),
		precise_ema,
		tolerance,
		"high precision should be equal to low precision within tolerance"
	);
}

// balancer history
#[test]
fn precision_of_ema_over_balancer_three_months_data_scrape_history_should_be_high_enough() {
	let data = balancer_data_weth_wbtc_three_months();
	let history: Vec<(EmaPrice, u32)> = data.into_iter().map(|(n, d)| (EmaPrice::new(n, d), 1_u32)).collect();
	let smoothing = smoothing_from_period(WEEK_PERIOD);

	let mut precise_ema = Rational::from(history[0].0);
	let mut ema = history[0].0;
	for (price, iterations) in history.into_iter().skip(1) {
		let smoothing_adj = high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations);
		precise_ema =
			high_precision::precise_weighted_average(precise_ema.clone(), Rational::from(price), smoothing_adj);
		ema = iterated_price_ema(iterations, ema, price, smoothing);

		let tolerance = Rational::from((1, 1e25 as u128));
		assert_rational_relative_approx_eq!(
			Rational::from(ema),
			precise_ema,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

#[ignore]
#[test]
fn precision_of_ema_over_balancer_one_year_data_scrape_history_should_be_high_enough() {
	let data = balancer_data_weth_wbtc_one_year();
	let history: Vec<(EmaPrice, u32)> = data.into_iter().map(|(n, d)| (EmaPrice::new(n, d), 1_u32)).collect();
	let smoothing = smoothing_from_period(WEEK_PERIOD);

	let mut precise_ema = Rational::from(history[0].0);
	let mut ema = history[0].0;
	for (price, iterations) in history.into_iter().skip(1) {
		let smoothing_adj = high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations);
		precise_ema =
			high_precision::precise_weighted_average(precise_ema.clone(), Rational::from(price), smoothing_adj);
		ema = iterated_price_ema(iterations, ema, price, smoothing);

		let tolerance = Rational::from((1, 1e20 as u128));
		assert_rational_relative_approx_eq!(
			Rational::from(ema),
			precise_ema,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

#[ignore]
#[test]
fn precision_of_ema_over_balancer_expanded_one_year_data_scrape_history_should_be_high_enough() {
	let data = balancer_data_weth_wbtc_one_year();
	let mut expanded_data = vec![];
	for _i in 0..20 {
		expanded_data.extend(data.clone());
	}
	let history: Vec<(EmaPrice, u32)> = expanded_data
		.into_iter()
		.map(|(n, d)| (EmaPrice::new(n, d), 1_u32))
		.collect();
	let smoothing = smoothing_from_period(WEEK_PERIOD);

	let mut precise_ema = Rational::from(history[0].0);
	let mut ema = history[0].0;
	for (price, iterations) in history.into_iter().skip(1) {
		let smoothing_adj = high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations);
		precise_ema =
			high_precision::precise_weighted_average(precise_ema.clone(), Rational::from(price), smoothing_adj);
		// reduces precision of the high precision comparison ema but speeds up the test by orders
		// of magnitude.
		high_precision::round(&mut precise_ema);
		ema = iterated_price_ema(iterations, ema, price, smoothing);

		let tolerance = Rational::from((1, 1e20 as u128));
		assert_rational_relative_approx_eq!(
			Rational::from(ema),
			precise_ema,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}
