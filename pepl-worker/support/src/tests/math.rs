use crate::math::{pow10_u256, pow10_u512};
use ethabi::ethereum_types::U512;
use sp_core::U256;

fn ten_pow(exp: usize) -> String {
	format!("1{}", "0".repeat(exp))
}

#[test]
fn pow10_u256_should_return_the_largest_power_of_ten_that_fits() {
	let expected = U256::from_dec_str(&ten_pow(77)).expect("10^77 fits in U256");

	assert_eq!(pow10_u256(77), Some(expected));
}

// `uint`'s `pow` panics on overflow in release builds too, so the guard must reject before it.
#[test]
fn pow10_u256_should_return_none_when_the_exponent_overflows() {
	assert_eq!(pow10_u256(78), None);
	assert_eq!(pow10_u256(u8::MAX), None);
}

#[test]
fn pow10_u512_should_return_the_largest_power_of_ten_that_fits() {
	let expected = U512::from_dec_str(&ten_pow(154)).expect("10^154 fits in U512");

	assert_eq!(pow10_u512(154), Some(expected));
}

#[test]
fn pow10_u512_should_return_none_when_the_exponent_overflows() {
	assert_eq!(pow10_u512(155), None);
	assert_eq!(pow10_u512(u8::MAX), None);
}
