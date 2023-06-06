use crate::fraction;
use crate::types::{Balance, Fraction};

use proptest::prelude::*;
use rand::Rng;
use rug::{Integer, Rational};
use sp_arithmetic::{FixedPointNumber, FixedU128};

// ----- Macros

/// Asserts that two expressions `$x` and `$y` are approximately equal to each other up to a delta `$z`.
#[macro_export]
macro_rules! assert_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		assert_approx_eq!($x, $y, $z, "values are not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y {
			$x.clone() - $y.clone()
		} else {
			$y.clone() - $x.clone()
		};
		assert!(
			diff <= $z,
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			$x,
			$y,
			diff,
			$z
		);
	}};
}
pub(crate) use assert_approx_eq;

/// Asserts that two expressions `$x` and `$y` are approximately equal to each other up to a delta `$z`.
macro_rules! prop_assert_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		prop_assert_approx_eq!($x, $y, $z, "values are not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y {
			$x.clone() - $y.clone()
		} else {
			$y.clone() - $x.clone()
		};
		prop_assert!(
			diff <= $z,
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			$x,
			$y,
			diff,
			$z
		);
	}};
}
pub(crate) use prop_assert_approx_eq;

/// Asserts that two `Rational` numbers `$x` and `$y` are approximately equal to each other up to a delta `$z`.
/// Converts the `Rational` numbers to `f64` for display.
macro_rules! assert_rational_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		assert_rational_approx_eq!($x, $y, $z, "values are not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y {
			$x.clone() - $y.clone()
		} else {
			$y.clone() - $x.clone()
		};
		assert!(
			diff <= $z,
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			$x.to_f64(),
			$y.to_f64(),
			diff.to_f64(),
			$z.to_f64()
		);
	}};
}
pub(crate) use assert_rational_approx_eq;

/// Asserts that two `Rational` numbers `$x` and `$y` are approximately equal to each other up to a delta `$z`.
/// Converts the `Rational` numbers to `f64` for display.
macro_rules! prop_assert_rational_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		prop_assert_rational_approx_eq!($x, $y, $z, "values are not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y {
			$x.clone() - $y.clone()
		} else {
			$y.clone() - $x.clone()
		};
		prop_assert!(
			diff <= $z,
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			$x.to_f64(),
			$y.to_f64(),
			diff.to_f64(),
			$z.to_f64()
		);
	}};
}
pub(crate) use prop_assert_rational_approx_eq;

/// Asserts that two `Rational` numbers `$x` and `$y` are approximately equal to each other up to a
/// relative error `$z`.
/// `$y` is used as the reference value to determine the error.
/// Converts the `Rational` numbers to `f64` for display.
macro_rules! assert_rational_relative_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		assert_rational_relative_approx_eq!($x, $y, $z, "values are not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y {
			$x.clone() - $y.clone()
		} else {
			$y.clone() - $x.clone()
		};
		assert!(
			diff.clone() / $y.clone() <= $z.clone(),
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			$x.to_f64(),
			$y.to_f64(),
			diff.to_f64(),
			($y * $z).to_f64()
		);
	}};
}
pub(crate) use assert_rational_relative_approx_eq;

/// Asserts that two `Rational` numbers `$x` and `$y` are approximately equal to each other up to a
/// relative error `$z`.
/// `$y` is used as the reference value to determine the error.
/// Converts the `Rational` numbers to `f64` for display.
macro_rules! prop_assert_rational_relative_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		prop_assert_rational_relative_approx_eq!($x, $y, $z, "values are not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y {
			$x.clone() - $y.clone()
		} else {
			$y.clone() - $x.clone()
		};
		prop_assert!(
			diff.clone() / $y.clone() <= $z.clone(),
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			$x.clone().to_f64(),
			$y.to_f64(),
			diff.to_f64(),
			($y * $z).to_f64()
		);
	}};
}
pub(crate) use prop_assert_rational_relative_approx_eq;

// ----- Constants

/// Smallest balance expected to show up in prices.
/// Existential deposit for BTC will likely be 100 satoshis.
pub const MIN_BALANCE: Balance = 50;
/// Biggest balance expected to show up in prices.
/// Total issuance of BSX is about 1e22, total issuance of FRAX is about 1e27.
pub const MAX_BALANCE: Balance = 1e22 as Balance;

// ----- High Precision Test Util Functions

/// Convert a fixed point number to an arbitrary precision rational number.
pub fn fixed_to_high_precision(f: FixedU128) -> Rational {
	Rational::from((f.into_inner(), FixedU128::DIV))
}

#[test]
fn fixed_to_high_precision_works() {
	assert_eq!(
		fixed_to_high_precision(FixedU128::from_float(0.25)),
		Rational::from((1, 4))
	);
}

/// Convert a fixed point fraction to an arbitrary precision rational number.
pub fn fraction_to_high_precision(f: Fraction) -> Rational {
	Rational::from((f.to_bits(), fraction::DIV))
}

#[test]
fn fraction_to_high_precision_works() {
	assert_eq!(fraction_to_high_precision(fraction::frac(1, 4)), Rational::from((1, 4)));
}

/// Convert a `Rational` number into its rounded `Integer` equivalent.
pub(crate) fn into_rounded_integer(r: Rational) -> Integer {
	let (num, den) = r.into_numer_denom();
	num.div_rem_round(den).0
}

// ----- Property Test Strategies

/// Generates an arbitrary `FixedU128` number.
pub fn any_fixed() -> impl Strategy<Value = FixedU128> {
	any::<u128>().prop_map(FixedU128::from_inner)
}

/// Generates an arbitrary tuple representing a rational number, ensuring that the denominator is greater 0.
pub fn any_rational() -> impl Strategy<Value = (u128, u128)> {
	(any::<u128>(), 1..u128::MAX)
}

/// Generates two tuples representing two rational numbers with the first being bigger than the second.
/// `min` determines the minimum value a numerator or denominator can have, `max` the maximum.
pub fn bigger_and_smaller_rational(min: u128, max: u128) -> impl Strategy<Value = ((u128, u128), (u128, u128))> {
	((min + 1)..max, (min.max(1))..(max - 1))
		.prop_perturb(move |(a, b), mut rng| ((a, b), (rng.gen_range(min..a), rng.gen_range(b..max))))
}
