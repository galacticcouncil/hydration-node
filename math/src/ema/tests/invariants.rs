use super::high_precision;

use crate::ema::*;
use crate::fraction;
use crate::support::rational::Rounding;
use crate::test_utils::{
	any_rational, bigger_and_smaller_rational, fraction_to_high_precision, prop_assert_approx_eq,
	prop_assert_rational_approx_eq, prop_assert_rational_relative_approx_eq,
};
use crate::test_utils::{MAX_BALANCE, MIN_BALANCE};
use crate::types::{Balance, Fraction};

use proptest::prelude::*;

use rug::Rational;
use sp_arithmetic::traits::{One, Zero};

/// 2 weeks at 6s block time
pub const MAX_ITERATIONS: u32 = 201_600;

//
// --- Strategies
//

/// Strategy for generating a random fixed point number between near 0 and 1.
fn fraction_above_zero_and_less_or_equal_one() -> impl Strategy<Value = Fraction> {
	(1..fraction::DIV).prop_map(Fraction::from_bits)
}

fn typical_period() -> impl Strategy<Value = u64> {
	1_u64..110_000
}

fn long_period() -> impl Strategy<Value = u64> {
	10_000_u64..110_000
}

fn realistic_balance() -> impl Strategy<Value = Balance> {
	MIN_BALANCE..MAX_BALANCE
}

fn iterations_up_to(max: u32) -> impl Strategy<Value = u32> {
	1_u32..max
}

fn iterations() -> impl Strategy<Value = u32> {
	1_u32..MAX_ITERATIONS
}

fn high_balance() -> impl Strategy<Value = Balance> {
	MIN_BALANCE..(MAX_BALANCE * 1000)
}

fn realistic_price() -> impl Strategy<Value = EmaPrice> {
	(MIN_BALANCE..MAX_BALANCE, MIN_BALANCE..MAX_BALANCE).prop_map(|(n, d)| EmaPrice::new_unchecked(n, d))
}

fn any_price() -> impl Strategy<Value = EmaPrice> {
	any_rational().prop_map(|(n, d)| EmaPrice::new_unchecked(n, d))
}

fn any_volume() -> impl Strategy<Value = EmaVolume> {
	(any::<Balance>(), any::<Balance>(), any::<Balance>(), any::<Balance>())
}

fn any_liquidity() -> impl Strategy<Value = EmaLiquidity> {
	(any::<Balance>(), any::<Balance>())
}

fn period_fraction() -> impl Strategy<Value = Fraction> {
	(typical_period()).prop_map(smoothing_from_period)
}

prop_compose! {
	fn period_and_iterations()(p in long_period())(
		period in Just(p),
		iterations in iterations_up_to(p as u32 * 2),
	) -> (u64, u32) {
	  (period, iterations)
	}
}

fn ema_price_history() -> impl Strategy<Value = Vec<(EmaPrice, u32)>> {
	prop::collection::vec(
		(
			any_rational().prop_map(|(n, d)| EmaPrice::new_unchecked(n, d)),
			iterations(),
		),
		2..50,
	)
}

prop_compose! {
	fn ema_balance_crash_history()(p in long_period())(
		period in Just(p),
		initial_balance in realistic_balance(),
		big_balance in (1e16 as Balance)..MAX_BALANCE, big_iter in iterations_up_to(p as u32 * 2),
		small_balance in MIN_BALANCE..100_000, small_iter in iterations_up_to(p as u32 * 2)
	) -> (u64, Vec<(Balance, u32)>) {
	  (period, vec![
		(initial_balance, 1),
		(big_balance, big_iter),
		(small_balance, small_iter)
	  ])
	}
}

fn ema_balance_history() -> impl Strategy<Value = Vec<(Balance, u32)>> {
	prop::collection::vec(((1e6 as Balance)..(1e28 as Balance), 1_u32..MAX_ITERATIONS), 2..50)
}

//
// --- Tests
//
proptest! {
	#[test]
	fn price_ema_stays_stable_if_the_value_does_not_change(
		smoothing in period_fraction(),
		price in realistic_price(),
	) {
		let next_price = price_weighted_average(price, price, smoothing);
		prop_assert_eq!(next_price, price);
	}
}

proptest! {
	#[test]
	fn balance_ema_stays_stable_if_the_value_does_not_change(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		balance in any::<Balance>()
	) {
		let next_balance = balance_weighted_average(balance, balance, smoothing);
		prop_assert_eq!(next_balance, balance);
	}
}

proptest! {
	#[test]
	fn volume_ema_stays_stable_if_the_value_does_not_change(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		volume in any_volume()
	) {
		let next_volume = volume_weighted_average(volume, volume, smoothing);
		prop_assert_eq!(next_volume, volume);
	}
}

proptest! {
	#[test]
	fn iterated_volume_ema_approaches_zero(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		iterations in 1_u32..MAX_ITERATIONS,
		volume in any_volume()
	) {
		let next_volume = iterated_volume_ema(iterations, volume, smoothing);
		let expected = (
			iterated_balance_ema(iterations, volume.0, 0, smoothing),
			iterated_balance_ema(iterations, volume.1, 0, smoothing),
			iterated_balance_ema(iterations, volume.2, 0, smoothing),
			iterated_balance_ema(iterations, volume.3, 0, smoothing),
		);
		prop_assert_eq!(next_volume, expected);
	}
}

proptest! {
	#[test]
	fn iterated_liquidity_ema_is_same_as_two_balance_emas(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		iterations in 1_u32..MAX_ITERATIONS,
		(prev, incoming) in (any_liquidity(), any_liquidity()),
	) {
		let next_liquidity = iterated_liquidity_ema(iterations, prev, incoming, smoothing);
		let expected = (
			iterated_balance_ema(iterations, prev.0, incoming.0, smoothing),
			iterated_balance_ema(iterations, prev.1, incoming.1, smoothing),
		);
		prop_assert_eq!(next_liquidity, expected);
	}
}

proptest! {
	#[test]
	fn one_price_iteration_ema_is_same_as_simple_version(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		(prev_price, incoming_price) in (any_price(), any_price())
	) {
		let iter_price = iterated_price_ema(1, prev_price, incoming_price, smoothing);
		let simple_price = price_weighted_average(prev_price, incoming_price, smoothing);
		prop_assert_eq!(iter_price, simple_price);
	}
}

proptest! {
	#[test]
	fn calculate_new_by_integrating_incoming_is_same_as_weighted_average(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		(prev_price, incoming_price) in (any_price(), any_price()),
		(prev_volume, incoming_volume) in (any_volume(), any_volume()),
		(prev_liquidity, incoming_liquidity) in (any_liquidity(), any_liquidity()),
	) {
		let simple_price = price_weighted_average(prev_price, incoming_price, smoothing);
		let simple_volume = volume_weighted_average(prev_volume, incoming_volume, smoothing);
		let simple_liquidity = liquidity_weighted_average(prev_liquidity, incoming_liquidity, smoothing);
		let new_oracle = calculate_new_by_integrating_incoming((prev_price, prev_volume, prev_liquidity), (incoming_price, incoming_volume, incoming_liquidity), smoothing);
		prop_assert_eq!(new_oracle, (simple_price, simple_volume, simple_liquidity));
	}
}

proptest! {
	#[test]
	fn update_outdated_to_current_is_same_as_iterated_ema(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		iterations in 1_u32..MAX_ITERATIONS,
		(prev_price, incoming_price) in (any_price(), any_price()),
		prev_volume in any_volume(),
		(prev_liquidity, incoming_liquidity) in (any_liquidity(), any_liquidity()),
	) {
		let iterated_price = iterated_price_ema(iterations, prev_price, incoming_price, smoothing);
		let iterated_volume = iterated_volume_ema(iterations, prev_volume, smoothing);
		let iterated_liquidity = iterated_liquidity_ema(iterations, prev_liquidity, incoming_liquidity, smoothing);
		let current_oracle = update_outdated_to_current(iterations, (prev_price, prev_volume, prev_liquidity), (incoming_price, incoming_liquidity), smoothing);
		prop_assert_eq!(current_oracle, (iterated_price, iterated_volume, iterated_liquidity));
	}
}

proptest! {
	#[test]
	fn one_balance_iteration_ema_is_same_as_simple_version(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		(prev_balance, incoming_balance) in (any::<Balance>(), any::<Balance>())
	) {
		let iter_balance = iterated_balance_ema(1, prev_balance, incoming_balance, smoothing);
		let simple_balance = balance_weighted_average(prev_balance, incoming_balance, smoothing);
		prop_assert_eq!(iter_balance, simple_balance);
	}
}

proptest! {
	#[test]
	fn new_balance_oracle_is_between_old_and_new_value(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		iterations in any::<u32>(),
		(prev_balance, incoming_balance) in
			(0..(Balance::MAX - 1)).prop_perturb(|n, mut rng| (n, rng.gen_range(n..Balance::MAX)))
	) {
		let balance = iterated_balance_ema(iterations, prev_balance, incoming_balance, smoothing);
		prop_assert!(balance <= incoming_balance, "{balance} not <= {incoming_balance}");
		prop_assert!(prev_balance <= balance, "{prev_balance} not <= {balance}");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn new_price_oracle_is_between_old_and_new_value_where_incoming_is_greater(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		i in iterations(),
		((incoming_n, incoming_d), (prev_n, prev_d)) in
			bigger_and_smaller_rational(1, MAX_BALANCE * 1_000)
	) {
		let prev_price = EmaPrice::new(prev_n, prev_d);
		let incoming_price = EmaPrice::new(incoming_n, incoming_d);
		let price = iterated_price_ema(i, prev_price, incoming_price, smoothing);
		prop_assert!(prev_price <= price);
		prop_assert!(price <= incoming_price);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn new_price_oracle_is_between_old_and_new_value_where_previous_is_greater(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		i in iterations(),
		((prev_n, prev_d), (incoming_n, incoming_d)) in
			bigger_and_smaller_rational(1, MAX_BALANCE * 1_000)
	) {
		let prev_price = EmaPrice::new_unchecked(prev_n, prev_d);
		let incoming_price = EmaPrice::new_unchecked(incoming_n, incoming_d);
		let price = iterated_price_ema(i, prev_price, incoming_price, smoothing);
		prop_assert!(incoming_price <= price);
		prop_assert!(price <= prev_price);
	}
}

proptest! {
	#[test]
	fn balance_weighted_averages_work_on_typical_values_with_typical_smoothing(
		period in typical_period(),
		(start_balance, incoming_balance) in
				(1e6 as Balance..(1e26 as Balance))
					.prop_perturb(|n, mut rng| (n, rng.gen_range(n..(1e26 as Balance))))
	) {
		let smoothing = smoothing_from_period(period);
		let next_balance = balance_weighted_average(start_balance, incoming_balance, smoothing);
		let expected: Rational =
			start_balance + Rational::from(incoming_balance - start_balance) * 2 / (period + 1);
		let tolerance = 1;
		let expected_balance = expected.round();
		prop_assert_approx_eq!(
			next_balance,
			expected_balance,
			tolerance,
			"averaged balance values should be within tolerance of the expected value"
		);
	}
}

proptest! {
	#[test]
	fn smoothing_is_greater_zero_and_less_equal_one(
		// We run into precision issues eventually, but any sane period value will be <10M
		period in 0_u64..2_000_000_000_000_000_000,
	) {
		let smoothing = smoothing_from_period(period);
		prop_assert!(smoothing > Fraction::zero());
		prop_assert!(smoothing <= Fraction::one());
	}
}

proptest! {
	#[test]
	fn smoothing_precision(
		// We run into precision issues eventually, but any sane period value will be <10M
		period in 0_u64..2_000_000_000_000_000_000,
	) {
		let smoothing = smoothing_from_period(period);
		let rug_smoothing = high_precision::smoothing_from_period(period);
		let epsilon = Rational::from((1, 1e18 as u128));
		let smoothing = fraction_to_high_precision(smoothing);
		prop_assert_rational_approx_eq!(smoothing, rug_smoothing, epsilon);
	}
}

proptest! {
	#[test]
	fn no_precision_loss_for_balances(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		(prev_balance, incoming_balance) in (any::<Balance>(), any::<Balance>())
	) {
		let balance = balance_weighted_average(prev_balance, incoming_balance, smoothing);
		let rug_balance = high_precision::precise_balance_weighted_average(
			prev_balance, incoming_balance, fraction_to_high_precision(smoothing));
		prop_assert_eq!(balance, rug_balance);
	}
}

proptest! {
	#[test]
	fn no_precision_loss_for_small_balance_values_with_small_smoothing_value(
		(prev_balance, incoming_balance) in (0..Balance::from(u64::MAX), 0..Balance::from(u64::MAX))
	) {
		let smoothing = fraction::SMALLEST_NON_ZERO;
		let balance = balance_weighted_average(prev_balance, incoming_balance, smoothing);
		let rug_balance = high_precision::precise_balance_weighted_average(
			prev_balance, incoming_balance,  fraction_to_high_precision(smoothing));
		prop_assert_eq!(balance, rug_balance);
	}
}

proptest! {
	#[test]
	fn no_precision_loss_for_small_balance_values_with_big_smoothing_value(
		(prev_balance, incoming_balance) in (0..Balance::from(u64::MAX), 0..Balance::from(u64::MAX))
	) {
		let smoothing = Fraction::from_bits(fraction::DIV - 1);
		let balance = balance_weighted_average(prev_balance, incoming_balance, smoothing);
		let rug_balance = high_precision::precise_balance_weighted_average(
			prev_balance, incoming_balance, fraction_to_high_precision(smoothing));
		prop_assert_eq!(balance, rug_balance);
	}
}

proptest! {
	#[test]
	fn low_precision_loss_for_prices(
		smoothing in fraction_above_zero_and_less_or_equal_one(),
		(prev_price, incoming_price) in (realistic_price(), realistic_price())
	) {
		let price = price_weighted_average(prev_price, incoming_price, smoothing);
		let rug_price = high_precision::precise_weighted_average(Rational::from(prev_price), Rational::from(incoming_price), fraction_to_high_precision(smoothing));
		let tolerance = Rational::from((1, 1e30 as u128));
		let price = Rational::from(price);
		prop_assert_rational_relative_approx_eq!(price, rug_price, tolerance);
	}
}

proptest! {
	#[test]
	fn exponential_smoothing_precision_should_be_high_enough(
		period in typical_period(),
		iterations in 1_u32..MAX_ITERATIONS,
	) {
		let smoothing = smoothing_from_period(period);
		let result = exp_smoothing(smoothing, iterations);
		let expected = high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations);

		let tolerance = Rational::from((1, 1e18 as u128));
		prop_assert_rational_approx_eq!(
			fraction_to_high_precision(result),
			expected,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

proptest! {
	#[test]
	fn iterated_balance_precision(
		period in typical_period(),
		iterations in 1_u32..MAX_ITERATIONS,
		(start_balance, incoming_balance) in
				(1e6 as Balance..(1e26 as Balance))
					.prop_perturb(|n, mut rng| (n, rng.gen_range(n..(1e26 as Balance))))
	) {
		let smoothing = smoothing_from_period(period);

		let expected = high_precision::precise_balance_weighted_average(start_balance, incoming_balance, high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations));
		let new_oracle = iterated_balance_ema(iterations, start_balance, incoming_balance, smoothing);

		let tolerance = 1;
		prop_assert_approx_eq!(
			new_oracle,
			expected,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn iterated_price_precision(
		(period, iterations) in period_and_iterations(),
		prev in realistic_price(),
		incoming in realistic_price(),
	) {
		let smoothing = smoothing_from_period(period);

		let res = iterated_price_ema(iterations, prev, incoming, smoothing);
		let smoothing_adj = high_precision::precise_exp_smoothing(fraction_to_high_precision(smoothing), iterations);
		let expected = high_precision::precise_weighted_average(Rational::from(prev), Rational::from(incoming), smoothing_adj);

		let res = Rational::from(res);
		let tolerance = Rational::from((1, 1e30 as u128));

		prop_assert_rational_relative_approx_eq!(
			res,
			expected,
			tolerance
		);
	}
}

proptest! {
	#[test]
	fn ema_balance_history_precision(
		history in ema_balance_history(),
		period in typical_period(),
	) {
		let smoothing = smoothing_from_period(period);
		let rug_ema = high_precision::precise_balance_ema(history.clone(), fraction_to_high_precision(smoothing));

		let mut ema = history[0].0;
		for (balance, iterations) in history.into_iter().skip(1) {
			ema = iterated_balance_ema(iterations, ema, balance, smoothing);
		}

		let tolerance = 1;
		prop_assert_approx_eq!(
			ema,
			rug_ema,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

proptest! {
	#[test]
	fn ema_balance_history_precision_crash_scenario(
		(period, history) in ema_balance_crash_history(),
	) {
		let smoothing = smoothing_from_period(period);
		let rug_ema = high_precision::precise_balance_ema(history.clone(), fraction_to_high_precision(smoothing));

		let mut ema = history[0].0;
		for (balance, iterations) in history.into_iter().skip(1) {
			ema = iterated_balance_ema(iterations, ema, balance, smoothing);
		}

		let tolerance = 1;
		prop_assert_approx_eq!(
			ema,
			rug_ema,
			tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

proptest! {
	#[ignore]
	#[test]
	fn ema_price_history_precision(
		history in ema_price_history(),
		period in typical_period(),
	) {
		let smoothing = smoothing_from_period(period);
		let rug_ema = high_precision::precise_price_ema(history.clone(), fraction_to_high_precision(smoothing));

		let mut ema = history[0].0;
		for (price, iterations) in history.into_iter().skip(1) {
			ema = iterated_price_ema(iterations, ema, price, smoothing);
		}

		let relative_tolerance = Rational::from((1, 1e24 as u128));
		prop_assert_rational_relative_approx_eq!(
			Rational::from(ema),
			rug_ema,
			relative_tolerance,
			"high precision should be equal to low precision within tolerance"
		);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn rational_rounding_add_should_have_high_enough_precision(
		(a, b) in (high_balance(), high_balance()),
		(c, d) in (high_balance(), high_balance()),
	) {
		let res = rounding_add(EmaPrice::new(a, b), (c.into(), d.into()), Rounding::Nearest);
		let expected = Rational::from((a, b)) + Rational::from((c, d));

		let res = Rational::from(res);
		// make sure the result has a precision of 100 bits
		let tolerance = Rational::from((1, 1u128 << 100));
		prop_assert_rational_relative_approx_eq!(res, expected, tolerance);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn rational_rounding_sub_should_have_high_enough_precision(
		((a, b), (c, d)) in bigger_and_smaller_rational(MIN_BALANCE, MAX_BALANCE * 1000),
	) {
		let res = rounding_sub(EmaPrice::new(a, b), (c.into(), d.into()), Rounding::Down);
		let expected = Rational::from((a, b)) - Rational::from((c, d));

		let res = Rational::from(res);
		// make sure the result has a precision of 77 bits
		let tolerance = Rational::from((1, 1u128 << 77));
		prop_assert_rational_relative_approx_eq!(res, expected, tolerance);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn rational_rounding_sub_result_should_be_smaller_or_equal_to_input(
		(a, b) in any_rational(),
		(c, d) in any_rational(),
	) {
		let res = rounding_sub(EmaPrice::new_unchecked(a, b), (c.into(), d.into()), Rounding::Down);
		prop_assert!(res <= EmaPrice::new_unchecked(a, b));
	}
}
