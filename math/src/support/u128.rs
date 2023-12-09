use crate::support::traits::{CheckedAddInto, CheckedDivInner, CheckedMulInner, CheckedMulInto};
use primitive_types::U256;

impl CheckedAddInto for u128 {
	type Output = U256;

	fn checked_add_into(&self, other: &Self) -> Option<Self::Output> {
		let s = Self::Output::from(*self);
		let o = Self::Output::from(*other);
		s.checked_add(o)
	}
}

impl CheckedMulInto for u128 {
	type Output = U256;

	fn checked_mul_into(&self, other: &Self) -> Option<Self::Output> {
		let s = Self::Output::from(*self);
		let o = Self::Output::from(*other);
		s.checked_mul(o)
	}
}

impl CheckedDivInner for U256 {
	type Inner = u128;

	fn checked_div_inner(&self, other: &Self::Inner) -> Option<Self> {
		self.checked_div(Self::from(*other))
	}
}

impl CheckedMulInner for U256 {
	type Inner = u128;

	fn checked_mul_inner(&self, other: &Self::Inner) -> Option<Self> {
		self.checked_mul(Self::from(*other))
	}
}

#[test]
fn checked_add_into_works_for_u128() {
	let r = 100u128;
	let result = r.checked_add_into(&200u128).unwrap();
	assert_eq!(result, U256::from(300u128));
}

#[test]
fn checked_mul_into_works_for_u128() {
	let r = 100u128;
	let result = r.checked_mul_into(&200u128).unwrap();
	assert_eq!(result, U256::from(20000u128));
}

#[test]
fn checked_add_into_works() {
	let a: u128 = 123456789;
	let b: u128 = 987654321;
	let expected = U256::from(a) + U256::from(b);

	assert_eq!(a.checked_add_into(&b), Some(expected));
}

#[test]
fn checked_mul_into_works() {
	let a: u128 = 123456789;
	let b: u128 = 987654321;
	let expected = U256::from(a) * U256::from(b);

	assert_eq!(a.checked_mul_into(&b), Some(expected));
}

#[test]
fn checked_div_inner_works() {
	let a: U256 = U256::from(123456789);
	let b: u128 = 987654321;
	let expected = a / U256::from(b);

	assert_eq!(a.checked_div_inner(&b), Some(expected));
}

#[test]
fn checked_mul_inner_works() {
	let a: U256 = U256::from(123456789);
	let b: u128 = 987654321;
	let expected = a * U256::from(b);

	assert_eq!(a.checked_mul_inner(&b), Some(expected));
}

#[test]
fn checked_add_into_handles_max_u128() {
	let a: u128 = u128::MAX;
	let b: u128 = 1;
	let expected = U256::from(a) + U256::from(b);

	assert_eq!(a.checked_add_into(&b), Some(expected));
}

#[test]
fn checked_mul_into_handles_max_u128() {
	let a: u128 = u128::MAX;
	let b: u128 = 2;
	let expected = U256::from(a) * U256::from(b);

	assert_eq!(a.checked_mul_into(&b), Some(expected));
}

#[test]
fn checked_div_inner_handles_max_u256() {
	let a: U256 = U256::MAX;
	let b: u128 = u128::MAX;

	let expected = a / U256::from(b);

	assert_eq!(a.checked_div_inner(&b), Some(expected));
}

#[test]
fn checked_div_inner_handles_zero_divisor() {
	let a: U256 = U256::from(123456789);
	let b: u128 = 0;

	assert_eq!(a.checked_div_inner(&b), None);
}

#[test]
fn checked_mul_inner_handles_max_u256() {
	let a: U256 = U256::MAX;
	let b: u128 = u128::MAX;

	assert_eq!(a.checked_mul_inner(&b), None);
}
