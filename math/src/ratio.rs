use crate::support::rational::{round_to_rational, Rounding};
use crate::to_u128_wrapper;
use codec::{Decode, Encode, MaxEncodedLen};
use core::cmp::{Ord, Ordering, PartialOrd};
use num_traits::{SaturatingAdd, SaturatingMul, SaturatingSub, Zero};
use primitive_types::U128;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_arithmetic::{helpers_128bit, Permill};
use sp_std::ops::{Add, Mul, Sub};

/// A rational number represented by a `n`umerator and `d`enominator.
#[derive(Clone, Copy, Default, PartialEq, Eq, Encode, Decode, Serialize, Deserialize, TypeInfo, MaxEncodedLen)]
pub struct Ratio {
	pub n: u128,
	pub d: u128,
}

impl Ratio {
	/// Build from a raw `n/d`. Ensures that `d > 0`.
	pub const fn new(n: u128, d: u128) -> Self {
		// reimplement `.max(1)` so this can be `const`
		let d = if d > 0 { d } else { 1 };
		Self { n, d }
	}

	/// Build from a raw `n/d`. This could lead to / 0 if not properly handled.
	pub const fn new_unchecked(n: u128, d: u128) -> Self {
		Self { n, d }
	}

	/// Return a representation of one.
	///
	/// Note that more than one combination of `n` and `d` can be one.
	pub const fn one() -> Self {
		Self::new_unchecked(1, 1)
	}

	/// Return whether `self` is one.
	///
	/// Should a denominator of 0 happen, this function will return `false`.
	///
	/// Note that more than one combination of `n` and `d` can be one.
	pub const fn is_one(&self) -> bool {
		self.d > 0 && self.n == self.d
	}

	/// Return a representation of zero.
	///
	/// Note that any combination of `n == 0` and `d` represents zero.
	pub const fn zero() -> Self {
		Self::new_unchecked(0, 1)
	}

	/// Return whether `self` is zero.
	///
	/// Note that any combination of `n == 0` and `d` represents zero.
	pub const fn is_zero(&self) -> bool {
		self.n == 0
	}

	/// Invert `n/d` to `d/n`.
	///
	/// NOTE: Zero inverts to zero.
	pub const fn inverted(self) -> Self {
		if self.is_zero() {
			self
		} else {
			Self { n: self.d, d: self.n }
		}
	}
}

impl From<Ratio> for (u128, u128) {
	fn from(ratio: Ratio) -> (u128, u128) {
		(ratio.n, ratio.d)
	}
}

#[cfg(test)]
impl From<Ratio> for rug::Rational {
	fn from(ratio: Ratio) -> rug::Rational {
		rug::Rational::from((ratio.n, ratio.d))
	}
}

impl From<u128> for Ratio {
	fn from(n: u128) -> Self {
		Self::new(n, 1)
	}
}

impl From<(u128, u128)> for Ratio {
	fn from((n, d): (u128, u128)) -> Self {
		Self::new(n, d)
	}
}

impl From<Permill> for Ratio {
	fn from(value: Permill) -> Self {
		(value.deconstruct() as u128, Permill::one().deconstruct() as u128).into()
	}
}

impl PartialOrd for Ratio {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

// Taken from Substrate's `Rational128`.
impl Ord for Ratio {
	fn cmp(&self, other: &Self) -> Ordering {
		if self.d == other.d {
			self.n.cmp(&other.n)
		} else if self.d.is_zero() {
			Ordering::Greater
		} else if other.d.is_zero() {
			Ordering::Less
		} else {
			let self_n = helpers_128bit::to_big_uint(self.n) * helpers_128bit::to_big_uint(other.d);
			let other_n = helpers_128bit::to_big_uint(other.n) * helpers_128bit::to_big_uint(self.d);
			self_n.cmp(&other_n)
		}
	}
}

impl Add for Ratio {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		if self.n.is_zero() {
			return rhs;
		} else if rhs.n.is_zero() {
			return self;
		}
		let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(self.n, self.d, rhs.n, rhs.d);
		let common_d = l_d.full_mul(r_d);
		let l_n_common = l_n.full_mul(r_d);
		let r_n_common = r_n.full_mul(l_d);
		let n = l_n_common.add(r_n_common);
		let d = common_d;
		round_to_rational((n, d), Rounding::Nearest).into()
	}
}

impl SaturatingAdd for Ratio {
	fn saturating_add(&self, rhs: &Self) -> Self {
		if self.n.is_zero() {
			return *rhs;
		} else if rhs.n.is_zero() {
			return *self;
		}
		let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(self.n, self.d, rhs.n, rhs.d);
		let common_d = l_d.full_mul(r_d);
		let l_n_common = l_n.full_mul(r_d);
		let r_n_common = r_n.full_mul(l_d);
		let n = l_n_common.saturating_add(r_n_common);
		let d = common_d;
		round_to_rational((n, d), Rounding::Down).into()
	}
}

impl Sub for Ratio {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		if self.n.is_zero() || rhs.n.is_zero() {
			return (self.n, self.d).into();
		}
		let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(self.n, self.d, rhs.n, rhs.d);
		// n = l.n * r.d - r.n * l.d
		let n = l_n.full_mul(r_d).sub(r_n.full_mul(l_d));
		// d = l.d * r.d
		let d = l_d.full_mul(r_d);
		round_to_rational((n, d), Rounding::Nearest).into()
	}
}

impl SaturatingSub for Ratio {
	fn saturating_sub(&self, rhs: &Self) -> Self {
		if self.n.is_zero() || rhs.n.is_zero() {
			return (self.n, self.d).into();
		}
		let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(self.n, self.d, rhs.n, rhs.d);
		// n = l.n * r.d - r.n * l.d
		let n = l_n.full_mul(r_d).saturating_sub(r_n.full_mul(l_d));
		// d = l.d * r.d
		let d = l_d.full_mul(r_d);
		round_to_rational((n, d), Rounding::Nearest).into()
	}
}

impl Mul for Ratio {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self::Output {
		if self.is_zero() || rhs.is_zero() {
			return Self::zero();
		}
		let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(self.n, self.d, rhs.n, rhs.d);
		let n = l_n.full_mul(r_n);
		let d = l_d.full_mul(r_d);
		round_to_rational((n, d), Rounding::Nearest).into()
	}
}

impl SaturatingMul for Ratio {
	fn saturating_mul(&self, rhs: &Self) -> Self {
		self.mul(*rhs)
	}
}

impl Ratio {
	pub fn saturating_div(&self, rhs: &Self) -> Self {
		if rhs.is_zero() {
			return Self::zero(); // Handle division by zero
		}
		let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(self.n, self.d, rhs.n, rhs.d);
		let n = l_n.full_mul(r_d);
		let d = l_d.full_mul(r_n);
		round_to_rational((n, d), Rounding::Nearest).into()
	}
}
#[cfg(feature = "std")]
impl sp_std::fmt::Debug for Ratio {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
		write!(
			f,
			"Ratio({} / {} ≈ {:.8})",
			self.n,
			self.d,
			self.n as f64 / self.d as f64
		)
	}
}

#[cfg(not(feature = "std"))]
impl sp_std::fmt::Debug for Ratio {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
		write!(f, "Ratio({} / {})", self.n, self.d)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rug::Rational;
	use test_case::test_case;
	#[test]
	fn test_add_ratios() {
		let ratio1 = Ratio::new(1, 2);
		let ratio2 = Ratio::new(1, 3);
		let result = ratio1 + ratio2;
		assert_eq!(result, Ratio::new(5, 6));
	}

	#[test]
	fn test_sub_ratios() {
		let ratio1 = Ratio::new(2, 1);
		let ratio2 = Ratio::new(1, 2);
		let result = ratio1 - ratio2;
		assert_eq!(result, Ratio::new(3, 2));
	}

	#[test]
	fn test_add_zero_ratio() {
		let ratio1 = Ratio::new(1, 2);
		let zero_ratio = Ratio::zero();
		let result = ratio1 + zero_ratio;
		assert_eq!(result, ratio1);
	}

	#[test]
	fn test_sub_zero_ratio() {
		let ratio1 = Ratio::new(1, 2);
		let zero_ratio = Ratio::zero();
		let result = ratio1 - zero_ratio;
		assert_eq!(result, ratio1);
	}

	#[test]
	fn test_add_one_ratio() {
		let ratio1 = Ratio::new(1, 2);
		let one_ratio = Ratio::one();
		let result = ratio1 + one_ratio;
		assert_eq!(result, Ratio::new(3, 2));
	}

	#[test]
	fn test_sub_one_ratio() {
		let ratio1 = Ratio::new(3, 2);
		let one_ratio = Ratio::one();
		let result = ratio1 - one_ratio;
		assert_eq!(result, Ratio::new(1, 2));
	}

	#[test]
	fn test_add_large_ratios() {
		let ratio1 = Ratio::new(u128::MAX, 111111111111);
		let ratio2 = Ratio::new(u128::MAX, 444444444444);
		let result = ratio1 + ratio2;
		assert_eq!(result, Ratio::new(171936116567241990952755394961819566079, 44913318605));
	}

	#[test]
	fn test_sub_large() {
		let ratio1 = Ratio::new(u128::MAX, 2);
		let ratio2 = Ratio::new(u128::MAX, 3);
		let result = ratio1 - ratio2;
		assert_eq!(result, Ratio::new(340282366920938463463374607431768211455, 6));
	}

	#[test]
	fn test_sub_large_ratios() {
		let ratio1 = Ratio::new(u128::MAX, 1);
		let ratio2 = Ratio::new(u128::MAX / 2, 1);
		let result = ratio1 - ratio2;
		assert_eq!(result, Ratio::new(u128::MAX / 2 + 1, 1));
	}

	#[test]
	fn test_add_small_ratios() {
		let ratio1 = Ratio::new(1, u128::MAX);
		let ratio2 = Ratio::new(1, u128::MAX);
		let result = ratio1 + ratio2;
		assert_eq!(result, Ratio::new(1, u128::MAX - 1));
	}

	#[test]
	fn test_sub_small_ratios() {
		let ratio1 = Ratio::new(1, u128::MAX);
		let ratio2 = Ratio::new(1, u128::MAX);
		let result = ratio1 - ratio2;
		assert!(result.is_zero());
	}

	#[test]
	fn test_mul_ratios() {
		let ratio1 = Ratio::new(1, 2);
		let ratio2 = Ratio::new(2, 3);
		let result = ratio1 * ratio2;
		assert_eq!(result, Ratio::new(2, 6));
	}

	#[test]
	fn test_mul_zero_ratio() {
		let ratio1 = Ratio::new(1, 2);
		let zero_ratio = Ratio::zero();
		let result = ratio1 * zero_ratio;
		assert_eq!(result, Ratio::zero());
	}

	#[test]
	fn test_mul_one_ratio() {
		let ratio1 = Ratio::new(1, 2);
		let one_ratio = Ratio::one();
		let result = ratio1 * one_ratio;
		assert_eq!(result, ratio1);
	}

	#[test]
	fn test_mul_large_ratios() {
		let ratio1 = Ratio::new(u128::MAX, 2);
		let ratio2 = Ratio::new(2, 3);
		let result = ratio1 * ratio2;
		assert_eq!(result, Ratio::new(u128::MAX, 3));
	}

	#[test]
	fn test_mul_small_ratios() {
		let ratio1 = Ratio::new(1, u128::MAX);
		let ratio2 = Ratio::new(1, u128::MAX);
		let result = ratio1 * ratio2;
		assert_eq!(result, Ratio::new(1, u128::MAX - 1));
	}

	#[test_case(Ratio::new(1, 2), Ratio::new(1, 2), Ratio::new(1, 1) ; "Dividing 1/2 by 1/2 should yield 1")]
	#[test_case(Ratio::new(1, 2), Ratio::new(1, 4), Ratio::new(2, 1) ; "Dividing 1/2 by 1/4 should yield 2")]
	#[test_case(Ratio::new(1, 2), Ratio::new(0, 1), Ratio::zero() ; "Dividing by zero should yield zero")]
	#[test_case(Ratio::new(0, 1), Ratio::new(1, 2), Ratio::zero() ; "Dividing zero by any number should yield zero")]
	#[test_case(Ratio::new(1, 2), Ratio::new(1, 1), Ratio::new(1, 2) ; "Dividing 1/2 by 1 should yield 1/2")]
	#[test_case(Ratio::new(1, 1), Ratio::new(1, 2), Ratio::new(2, 1) ; "Dividing 1 by 1/2 should yield 2")]
	#[test_case(Ratio::new(u128::MAX, 1), Ratio::new(1, 1), Ratio::new(u128::MAX, 1) ; "Dividing max value by 1 should yield max value")]
	#[test_case(Ratio::new(1, 1), Ratio::new(u128::MAX, 1), Ratio::new(1, u128::MAX) ; "Dividing 1 by max value should yield small value")]
	fn test_saturating_div(numerator: Ratio, denominator: Ratio, expected: Ratio) {
		let calculated = numerator.saturating_div(&denominator);
		let expected_rug: Rational = expected.into();
		let calculated_rug: Rational = calculated.into();
		assert_eq!(calculated_rug, expected_rug);
	}

	#[test_case(Ratio::new(1, 2), Ratio::new(1, 2), Ratio::new(1, 4) ; "Multiplying 1/2 by 1/2 should yield 1/4")]
	#[test_case(Ratio::new(1, 2), Ratio::new(1, 4), Ratio::new(1, 8) ; "Multiplying 1/2 by 1/4 should yield 1/8")]
	#[test_case(Ratio::new(1, 2), Ratio::new(0, 1), Ratio::zero() ; "Multiplying by zero should yield zero")]
	#[test_case(Ratio::new(0, 1), Ratio::new(1, 2), Ratio::zero() ; "Multiplying zero by any number should yield zero")]
	#[test_case(Ratio::new(1, 2), Ratio::new(2, 1), Ratio::new(1, 1) ; "Multiplying 1/2 by 2 should yield 1")]
	#[test_case(Ratio::new(2, 1), Ratio::new(1, 2), Ratio::new(1, 1) ; "Multiplying 2 by 1/2 should yield 1")]
	#[test_case(Ratio::new(u128::MAX, 1), Ratio::new(1, 1), Ratio::new(u128::MAX, 1) ; "Multiplying max value by 1 should yield max value")]
	#[test_case(Ratio::new(1, 1), Ratio::new(u128::MAX, 1), Ratio::new(u128::MAX, 1) ; "Multiplying 1 by max value should yield max value")]
	fn test_saturating_mul(numerator: Ratio, denominator: Ratio, expected: Ratio) {
		let calculated = numerator.saturating_mul(&denominator);
		let expected_rug: Rational = expected.into();
		let calculated_rug: Rational = calculated.into();
		assert_eq!(calculated_rug, expected_rug);
	}

	#[test_case(Ratio::new(1, 2), Ratio::new(1, 2), Ratio::new(1, 1) ; "Adding 1/2 and 1/2 should yield 1")]
	#[test_case(Ratio::new(1, 2), Ratio::new(1, 4), Ratio::new(3, 4) ; "Adding 1/2 and 1/4 should yield 3/4")]
	#[test_case(Ratio::new(1, 2), Ratio::new(0, 1), Ratio::new(1, 2) ; "Adding 1/2 and 0 should yield 1/2")]
	#[test_case(Ratio::new(0, 1), Ratio::new(1, 2), Ratio::new(1, 2) ; "Adding 0 and 1/2 should yield 1/2")]
	fn test_saturating_add(numerator: Ratio, denominator: Ratio, expected: Ratio) {
		let calculated = numerator.saturating_add(&denominator);
		let expected_rug: Rational = expected.into();
		let calculated_rug: Rational = calculated.into();
		assert_eq!(calculated_rug, expected_rug);
	}

	#[test_case(Ratio::new(1, 2), Ratio::new(1, 2), Ratio::new(0, 1) ; "Subtracting 1/2 from 1/2 should yield 0")]
	#[test_case(Ratio::new(1, 2), Ratio::new(1, 4), Ratio::new(1, 4) ; "Subtracting 1/4 from 1/2 should yield 1/4")]
	#[test_case(Ratio::new(1, 2), Ratio::new(0, 1), Ratio::new(1, 2) ; "Subtracting 0 from 1/2 should yield 1/2")]
	#[test_case(Ratio::new(0, 1), Ratio::new(1, 2), Ratio::zero() ; "Subtracting 1/2 from 0 should yield 0")]
	fn test_saturating_sub(numerator: Ratio, denominator: Ratio, expected: Ratio) {
		let calculated = numerator.saturating_sub(&denominator);
		let expected_rug: Rational = expected.into();
		let calculated_rug: Rational = calculated.into();
		assert_eq!(calculated_rug, expected_rug);
	}
}
