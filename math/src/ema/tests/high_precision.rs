use crate::ema::EmaPrice;
use crate::fraction;
use crate::test_utils::{fraction_to_high_precision, into_rounded_integer};
use crate::types::Balance;

use num_traits::{One, Pow};
use proptest::prelude::*;
use rug::ops::PowAssign;
use rug::{Integer, Rational};
use std::ops::{Mul, ShrAssign};

/// Round the given `r` to a close number where numerator and denominator have <= 256 bits.
pub(crate) fn round(r: &mut Rational) {
	r.mutate_numer_denom(|n, d| {
		let n_digits = n.significant_digits::<bool>();
		let d_digits = d.significant_digits::<bool>();
		if n_digits > 256 || d_digits > 256 {
			let shift = n_digits.saturating_sub(256).max(d_digits.saturating_sub(256));
			n.shr_assign(shift);
			d.shr_assign(shift);
		}
	});
}

/// Calculate the power of `r` via stepwise squaring and rounding to keep the memory size of `r`
/// within reasonable bounds.
///
/// Much faster for big `i` but less accurate than the built-in `pow` function.
pub(crate) fn stepwise_pow_approx(mut r: Rational, i: u32) -> Rational {
	if i <= 256 {
		return r.pow(i);
	}
	let next_power = i.next_power_of_two();
	let mut iter = if next_power == i { i } else { next_power / 2 };
	let rest = i - iter;
	let mut res_rest = stepwise_pow_approx(r.clone(), rest);
	round(&mut res_rest);
	while iter > 1 {
		iter /= 2;
		r.pow_assign(2);
		round(&mut r);
	}
	r * res_rest
}

/// Determine the smoothing factor from the given `period` as an arbitrary precision `Rational`.
pub fn smoothing_from_period(period: u64) -> Rational {
	Rational::from((2u64, period.max(1).saturating_add(1)))
}

/// Determine the final smoothing factor from initial `smoothing` and the number of `iterations`.
///
/// Uses a `pow` approximation with 256 bit precision to reduce execution time.
pub fn precise_exp_smoothing(smoothing: Rational, iterations: u32) -> Rational {
	debug_assert!(smoothing <= Rational::one());
	let complement = Rational::one() - smoothing;
	// in order to determine the iterated smoothing factor we exponentiate the complement
	let exp_complement = stepwise_pow_approx(complement, iterations);
	debug_assert!(exp_complement <= Rational::one());
	Rational::one() - exp_complement
}

/// Calculate the weighted average for the given balances by using arbitrary precision math.
///
/// Note: Rounding is biased very slightly towards `incoming` (on equal distance rounds away from
/// zero).
pub fn precise_balance_weighted_average(prev: Balance, incoming: Balance, weight: Rational) -> Integer {
	if incoming >= prev {
		prev + into_rounded_integer(weight.mul(incoming - prev))
	} else {
		prev - into_rounded_integer(weight.mul(prev - incoming))
	}
}

/// Calculate the weighted average for the given values by using arbitrary precision math.
/// Returns a `Rational` of arbitrary precision.
pub fn precise_weighted_average(prev: Rational, incoming: Rational, weight: Rational) -> Rational {
	prev.clone() + weight.mul(incoming - prev)
}

/// Determine the exponential moving average of a history of balance values.
/// Starts the EMA with the first value.
/// Keeps track of arbitrary precision values during calculation but returns an `Integer` (rounded down).
pub fn naive_precise_balance_ema(history: Vec<Balance>, smoothing: Rational) -> Integer {
	assert!(!history.is_empty());
	let mut current = Rational::from(history[0]);
	for balance in history.into_iter().skip(1) {
		current = precise_weighted_average(current.clone(), balance.into(), smoothing.clone());
	}
	// return rounded down integer
	into_rounded_integer(current)
}

/// Determine the exponential moving average of a history of balance values.
/// Starts the EMA with the first value.
/// Returns an arbitrary sized `Integer`.
/// Uses a `pow` approximation with 256 bit precision to reduce execution time.
pub fn precise_balance_ema(history: Vec<(Balance, u32)>, smoothing: Rational) -> Integer {
	assert!(!history.is_empty());
	let mut current = Rational::from((history[0].0, 1));
	for (balance, iterations) in history.into_iter().skip(1) {
		let smoothing_adj = precise_exp_smoothing(smoothing.clone(), iterations);
		current = precise_weighted_average(current.clone(), balance.into(), smoothing_adj.clone());
	}
	into_rounded_integer(current)
}

/// Determine the exponential moving average of a history of price values.
/// Starts the EMA with the first value.
/// Returns an arbitrary precision `Rational` number.
pub fn naive_precise_price_ema(history: Vec<EmaPrice>, smoothing: Rational) -> Rational {
	assert!(!history.is_empty());
	let mut current = Rational::from(history[0]);
	for price in history.into_iter().skip(1) {
		current = precise_weighted_average(current.clone(), Rational::from(price), smoothing.clone());
	}
	current
}

/// Determine the exponential moving average of a history of price values.
/// Starts the EMA with the first value.
/// Returns an arbitrary precision `Rational` number.
/// Uses a `pow` approximation with 256 bit precision to reduce execution time.
pub fn precise_price_ema(history: Vec<(EmaPrice, u32)>, smoothing: Rational) -> Rational {
	assert!(!history.is_empty());
	let mut current = Rational::from(history[0].0);
	for (price, iterations) in history.into_iter().skip(1) {
		let smoothing_adj = precise_exp_smoothing(smoothing.clone(), iterations);
		current = precise_weighted_average(current.clone(), Rational::from(price), smoothing_adj.clone());
	}
	current
}

// --- Tests

#[test]
fn precise_balance_ema_works() {
	let history = vec![1e12 as Balance, 2e12 as Balance, 3e12 as Balance, 4e12 as Balance];
	let smoothing = fraction::frac(1, 4);
	let expected = {
		let res =
			((Rational::from(history[0]) * 3 / 4 + history[1] / 4) * 3 / 4 + history[2] / 4) * 3 / 4 + history[3] / 4;
		into_rounded_integer(res)
	};
	let naive_ema = naive_precise_balance_ema(history.clone(), fraction_to_high_precision(smoothing));
	assert_eq!(expected, naive_ema);
	let history = history.into_iter().map(|b| (b, 1)).collect();
	let ema = precise_balance_ema(history, fraction_to_high_precision(smoothing));
	assert_eq!(expected, ema);
}

#[test]
fn precise_price_ema_works() {
	let history = vec![
		EmaPrice::new(1, 8),
		EmaPrice::new(1, 1),
		EmaPrice::new(8, 1),
		EmaPrice::new(4, 1),
	];
	let smoothing = fraction::frac(1, 4);
	let expected = ((Rational::from(history[0]) * 3 / 4 + Rational::from(history[1]) / 4) * 3 / 4
		+ Rational::from(history[2]) / 4)
		* 3 / 4 + Rational::from(history[3]) / 4;
	let naive_ema = naive_precise_price_ema(history.clone(), fraction_to_high_precision(smoothing));
	assert_eq!(expected, naive_ema);
	let history = history.into_iter().map(|p| (p, 1)).collect();
	let ema = precise_price_ema(history, fraction_to_high_precision(smoothing));
	assert_eq!(expected, ema);
}

fn small_rational_close_to_one() -> impl Strategy<Value = Rational> {
	(1u64..1_000, 5_000u64..200_000).prop_map(|(a, b)| Rational::one() - Rational::from((a, b)))
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(64))]
	#[test]
	fn stepwise_pow_close_enough(
		num in small_rational_close_to_one(),
		exponent in 1u32..200_000,
	) {
			let res_pow = num.clone().pow(exponent);
			let res_step = stepwise_pow_approx(num, exponent);
			let boundary = Rational::from((1, u128::MAX));
			prop_assert!((res_pow - res_step).abs() < boundary);
	}
}
