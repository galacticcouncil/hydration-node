//  Copyright (c) 2019 Alain Brenzikofer, modified by GalacticCouncil(2021)
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//
// Original source: https://github.com/encointer/substrate-fixed

#![allow(clippy::result_unit_err)]

use core::convert::From;
use core::ops::{AddAssign, BitOrAssign, ShlAssign, Shr, ShrAssign};
use fixed::traits::{FixedUnsigned, ToFixed};
use num_traits::{One, SaturatingMul, Zero};

/// right-shift with rounding
fn rs<T>(operand: T) -> T
where
	T: FixedUnsigned + One,
{
	let lsb = T::one() >> T::FRAC_NBITS;
	(operand >> 1_u32) + (operand & lsb)
}

/// base 2 logarithm assuming self >=1
fn log2_inner<S, D>(operand: S) -> D
where
	S: FixedUnsigned + PartialOrd<D> + One,
	D: FixedUnsigned + One,
	D::Bits: Copy + ToFixed + AddAssign + BitOrAssign + ShlAssign,
{
	let two = D::from_num(2);
	let mut x = operand;
	let mut result = D::from_num(0).to_bits();
	let lsb = (D::one() >> D::FRAC_NBITS).to_bits();

	while x >= two {
		result += lsb;
		x = rs(x);
	}

	if x == D::one() {
		return D::from_num(result);
	}

	for _i in (0..D::FRAC_NBITS).rev() {
		x *= x;
		result <<= lsb;
		if x >= two {
			result |= lsb;
			x = rs(x);
		}
	}
	D::from_bits(result)
}

/// base 2 logarithm
///
/// Returns tuple(D,bool) where bool indicates whether D is negative. This happens when operand is < 1.
pub fn log2<S, D>(operand: S) -> Result<(D, bool), ()>
where
	S: FixedUnsigned,
	D: FixedUnsigned + From<S> + One,
	D::Bits: Copy + ToFixed + AddAssign + BitOrAssign + ShlAssign,
{
	if operand <= S::from_num(0) {
		return Err(());
	}

	let operand = D::from(operand);
	if operand < D::one() {
		let inverse = D::one().checked_div(operand).unwrap(); // Unwrap is safe because operand is always > 0
		return Ok((log2_inner::<D, D>(inverse), true));
	}
	Ok((log2_inner::<D, D>(operand), false))
}

/// natural logarithm
/// Returns tuple(D,bool) where bool indicates whether D is negative. This happens when operand is < 1.
pub fn ln<S, D>(operand: S) -> Result<(D, bool), ()>
where
	S: FixedUnsigned,
	D: FixedUnsigned + From<S> + One,
	D::Bits: Copy + ToFixed + AddAssign + BitOrAssign + ShlAssign,
	S::Bits: Copy + ToFixed + AddAssign + BitOrAssign + ShrAssign + Shr,
{
	let log2_e = S::from_num(fixed::consts::LOG2_E);
	let log_result = log2::<S, D>(operand)?;
	Ok((log_result.0 / D::from(log2_e), log_result.1))
}

/// exponential function e^(operand)
/// neg - bool indicates that operand is negative value.
pub fn exp<S, D>(operand: S, neg: bool) -> Result<D, ()>
where
	S: FixedUnsigned + PartialOrd<D> + One,
	D: FixedUnsigned + PartialOrd<S> + From<S> + One,
{
	if operand.is_zero() {
		return Ok(D::one());
	}
	if operand == S::one() && !neg {
		let e = S::from_num(fixed::consts::E);
		return Ok(D::from(e));
	}

	let operand = D::from(operand);
	let mut result = operand + D::one();
	let mut term = operand;

	let max_iter = D::FRAC_NBITS.checked_mul(3).ok_or(())?;

	result = (2..max_iter).try_fold(result, |acc, i| -> Result<D, ()> {
		term = term.checked_mul(operand).ok_or(())?;
		term = term.checked_div(D::from_num(i)).ok_or(())?;
		acc.checked_add(term).ok_or(())
	})?;

	if neg {
		result = D::one().checked_div(result).ok_or(())?;
	}

	Ok(result)
}

/// power function with arbitrary fixed point number exponent
pub fn pow<S, D>(operand: S, exponent: S) -> Result<D, ()>
where
	S: FixedUnsigned + One + PartialOrd<D> + Zero,
	D: FixedUnsigned + From<S> + One + Zero,
	D::Bits: Copy + ToFixed + AddAssign + BitOrAssign + ShlAssign,
	S::Bits: Copy + ToFixed + AddAssign + BitOrAssign + ShlAssign + Shr + ShrAssign,
{
	if operand.is_zero() {
		return Ok(D::zero());
	} else if exponent == S::zero() {
		return Ok(D::one());
	} else if exponent == S::one() {
		return Ok(D::from(operand));
	}

	let (r, neg) = ln::<S, D>(operand)?;

	let r: D = r.checked_mul(exponent.into()).ok_or(())?;
	let r: D = exp(r, neg)?;

	let (result, oflw) = r.overflowing_to_num::<D>();
	if oflw {
		return Err(());
	};
	Ok(result)
}

/// power with integer exponent
pub fn powi<S, D>(operand: S, exponent: u32) -> Result<D, ()>
where
	S: FixedUnsigned + Zero,
	D: FixedUnsigned + From<S> + One + Zero,
{
	if operand == S::zero() {
		return Ok(D::zero());
	} else if exponent == 0 {
		return Ok(D::one());
	} else if exponent == 1 {
		return Ok(D::from(operand));
	}
	let operand = D::from(operand);

	let r = (1..exponent).try_fold(operand, |acc, _| acc.checked_mul(operand));

	r.ok_or(())
}

/// Determine `operand^n` for with higher precision for `operand` values close to but less than 1.
pub fn saturating_powi_high_precision<S, D>(operand: S, n: u32) -> D
where
	S: FixedUnsigned + One + Zero,
	D: FixedUnsigned + From<S> + One + Zero,
	S::Bits: From<u32>,
	D::Bits: From<u32>,
{
	if operand == S::zero() {
		return D::zero();
	} else if n == 0 {
		return D::one();
	} else if n == 1 {
		return D::from(operand);
	}

	// this determines when we use the taylor series approximation at 1
	// if boundary = 0, we will never use the taylor series approximation.
	// as boundary -> 1, we will use the taylor series approximation more and more
	// boundary > 1 can cause overflow in the taylor series approximation
	let boundary = S::one()
		.checked_div_int(10_u32.into())
		.expect("1 / 10 does not fail; qed");
	match (boundary.checked_div_int(n.into()), S::one().checked_sub(operand)) {
		(Some(b), Some(one_minus_operand)) if b > one_minus_operand => {
			powi_near_one(operand.into(), n).unwrap_or_else(|| saturating_pow(operand.into(), n))
		}
		_ => saturating_pow(operand.into(), n),
	}
}

fn saturating_pow<S>(operand: S, exp: u32) -> S
where
	S: FixedUnsigned + One + SaturatingMul,
	S::Bits: From<u32>,
{
	if exp == 0 {
		return S::one();
	}

	let msb_pos = 32 - exp.leading_zeros();

	let mut result = S::one();
	let mut pow_val = operand;
	for i in 0..msb_pos {
		if ((1 << i) & exp) > 0 {
			result = result.saturating_mul(pow_val);
		}
		pow_val = pow_val.saturating_mul(pow_val);
	}
	result
}

/// Determine `operand^n` for `operand` values close to but less than 1.
fn powi_near_one<S>(operand: S, n: u32) -> Option<S>
where
	S: FixedUnsigned + One + Zero,
	S::Bits: From<u32>,
{
	if n == 0 {
		return Some(S::one());
	} else if n == 1 {
		return Some(operand);
	}
	let one_minus_operand = S::one().checked_sub(operand)?;

	// prevents overflows
	debug_assert!(S::one().checked_div_int(n.into())? > one_minus_operand);
	if S::one().checked_div_int(n.into())? <= one_minus_operand {
		return None;
	}

	let mut s_pos = S::one();
	let mut s_minus = S::zero();
	let mut t = S::one();
	// increasing number of iterations will allow us to return a result for operands farther from 1,
	// or for higher values of n
	let iterations = 32_u32;
	for i in 1..iterations {
		// bare math fine because n > 1 and return condition below
		let b = one_minus_operand.checked_mul_int(S::Bits::from(n - i + 1))?;
		let t_factor = b.checked_div_int(i.into())?;
		t = t.checked_mul(t_factor)?;
		if i % 2 == 0 || operand > S::one() {
			s_pos = s_pos.checked_add(t)?;
		} else {
			s_minus = s_minus.checked_add(t)?;
		}

		// if i >= b, all future terms will be zero because kth derivatives of a polynomial
		// of degree n where k > n are zero
		// if t == 0, all future terms will be zero because they will be multiples of t
		if i >= n || t == S::zero() {
			return s_pos.checked_sub(s_minus);
		}
	}
	None // if we do not have convergence, we do not risk returning an inaccurate value
}

#[cfg(test)]
mod tests {
	use crate::fraction;
	use crate::types::Fraction;
	use core::str::FromStr;
	use fixed::traits::LossyInto;
	use fixed::types::U32F96;
	use fixed::types::U64F64;

	use super::*;

	#[test]
	fn exp_works() {
		type S = U64F64;
		type D = U64F64;

		let e = S::from_num(fixed::consts::E);

		let zero = S::from_num(0);
		let one = S::one();
		let two = S::from_num(2);

		assert_eq!(exp::<S, D>(zero, false), Ok(D::from_num(one)));
		assert_eq!(exp::<S, D>(one, false), Ok(D::from_num(e)));
		assert_eq!(
			exp::<S, D>(two, false),
			Ok(D::from_str("7.3890560989306502265").unwrap())
		);
		assert_eq!(
			exp::<S, D>(two, true),
			Ok(D::from_str("0.13533528323661269186").unwrap())
		);
		assert_eq!(
			exp::<S, D>(one, true),
			Ok(D::from_str("0.367879441171442321595523770161460867445").unwrap()),
		);
	}

	#[test]
	fn log2_works() {
		type S = U64F64;
		type D = U64F64;

		let zero = S::from_num(0);
		let one = S::one();
		let two = S::from_num(2);
		let four = S::from_num(4);

		assert_eq!(log2::<S, D>(zero), Err(()));

		assert_eq!(log2(two), Ok((D::from_num(one), false)));
		assert_eq!(log2(one / four), Ok((D::from_num(two), true)));
		assert_eq!(log2(S::from_num(0.5)), Ok((D::from_num(one), true)));
		assert_eq!(log2(S::from_num(1.0 / 0.5)), Ok((D::from_num(one), false)));
	}

	#[test]
	fn powi_works() {
		type S = U64F64;
		type D = U64F64;

		let zero = S::from_num(0);
		let one = S::one();
		let two = S::from_num(2);
		let four = S::from_num(4);

		assert_eq!(powi(two, 0), Ok(D::from_num(one)));
		assert_eq!(powi(zero, 2), Ok(D::from_num(zero)));
		assert_eq!(powi(two, 1), Ok(D::from_num(2)));
		assert_eq!(powi(two, 2), Ok(D::from_num(4)));
		assert_eq!(powi(two, 3), Ok(D::from_num(8)));
		assert_eq!(powi(one / four, 2), Ok(D::from_num(0.0625)));
	}

	#[test]
	fn saturating_powi_high_precision_works() {
		type S = U64F64;
		type D = U64F64;

		let zero = S::from_num(0);
		let one = S::one();
		let two = S::from_num(2);
		let four = S::from_num(4);

		assert_eq!(saturating_powi_high_precision::<S, D>(two, 0), D::from_num(one));
		assert_eq!(saturating_powi_high_precision::<S, D>(zero, 2), D::from_num(zero));
		assert_eq!(saturating_powi_high_precision::<S, D>(two, 1), D::from_num(2));
		assert_eq!(saturating_powi_high_precision::<S, D>(two, 2), D::from_num(4));
		assert_eq!(saturating_powi_high_precision::<S, D>(two, 3), D::from_num(8));
		assert_eq!(
			saturating_powi_high_precision::<S, D>(one / four, 2),
			D::from_num(0.0625)
		);
		assert_eq!(
			saturating_powi_high_precision::<S, D>(S::from_num(9) / 10, 2),
			D::from_num(81) / 100
		);

		let expected: D = powi(D::from_num(9) / 10, 2).unwrap();
		assert_eq!(saturating_powi_high_precision::<S, D>(S::from_num(9) / 10, 2), expected);
		let expected: D = powi(D::from_num(8) / 10, 2).unwrap();
		assert_eq!(saturating_powi_high_precision::<S, D>(S::from_num(8) / 10, 2), expected);
	}

	#[test]
	fn saturating_powi_high_precision_works_for_fraction() {
		assert_eq!(
			saturating_powi_high_precision::<Fraction, Fraction>(Fraction::one() / 4, 2),
			Fraction::from_num(0.0625)
		);
		assert_eq!(
			saturating_powi_high_precision::<Fraction, Fraction>(fraction::frac(6, 10), 2),
			fraction::frac(36, 100)
		);
		let expected: Fraction = powi(fraction::frac(8, 10), 2).unwrap();
		assert_eq!(
			saturating_powi_high_precision::<Fraction, Fraction>(fraction::frac(8, 10), 2),
			expected
		);
	}

	#[test]
	fn powi_near_one_works() {
		type S = U64F64;

		assert_eq!(powi_near_one(S::from_num(9) / 10, 2), Some(S::from_num(81) / 100));
	}

	#[test]
	fn pow_works() {
		type S = U32F96;
		type D = U32F96;
		let zero = S::from_num(0);
		let one = S::one();
		let two = S::from_num(2);
		let three = S::from_num(3);
		let four = S::from_num(4);

		assert_eq!(pow::<S, D>(two, zero), Ok(one));
		assert_eq!(pow::<S, D>(zero, two), Ok(zero));

		let result: f64 = pow::<S, D>(two, three).unwrap().lossy_into();
		assert_relative_eq!(result, 8.0, epsilon = 1.0e-6);

		let result: f64 = pow::<S, D>(one / four, two).unwrap().lossy_into();
		assert_relative_eq!(result, 0.0625, epsilon = 1.0e-6);

		assert_eq!(pow::<S, D>(two, one), Ok(two));

		let result: f64 = pow::<S, D>(one / four, one / two).unwrap().lossy_into();
		assert_relative_eq!(result, 0.5, epsilon = 1.0e-6);

		assert_eq!(
			pow(S::from_num(22.1234), S::from_num(2.1)),
			Ok(D::from_str("667.0969121771803182631954923946").unwrap())
		);

		assert_eq!(
			pow(S::from_num(0.986069911074), S::from_num(1.541748732743)),
			Ok(D::from_str("0.97860451447489653592682845716").unwrap())
		);
	}
}
