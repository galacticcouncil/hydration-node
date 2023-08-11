//! # HydraDX Math
//!
//! A collection of utilities to make performing liquidity pool
//! calculations more convenient.
#![allow(clippy::bool_to_int_with_if)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(not(feature = "std"), test))]
extern crate std;

#[cfg(test)]
#[macro_use]
extern crate approx;

#[cfg(test)]
mod test_pow_accuracy;

pub mod dynamic_fees;
pub mod ema;
pub mod fee;
pub mod fraction;
pub mod lbp;
pub mod liquidity_mining;
pub mod omnipool;
pub mod omnipool_subpools;
pub mod rate_limiter;
pub mod ratio;
pub mod stableswap;
pub mod staking;
pub mod support;
#[cfg(test)]
pub mod test_utils;
pub mod transcendental;
pub mod types;
pub mod xyk;

#[macro_export]
macro_rules! ensure {
	($e:expr, $f:expr) => {
		match $e {
			true => (),
			false => {
				return Err($f);
			}
		}
	};
}

#[macro_export]
macro_rules! round_up {
	($e:expr) => {
		$e.checked_add(FIXED_ROUND_UP).ok_or(Overflow)
	};
}

#[macro_export]
macro_rules! to_u256 {
    ($($x:expr),+) => (
        {($(U256::from($x)),+)}
    );
}

#[macro_export]
macro_rules! to_u128_wrapper {
    ($($x:expr),+) => (
        {($(U128::from($x)),+)}
    );
}

#[macro_export]
macro_rules! to_balance {
	($x:expr) => {
		Balance::try_from($x).map_err(|_| Overflow)
	};
}

#[macro_export]
macro_rules! to_lbp_weight {
	($x:expr) => {
		LBPWeight::try_from($x).map_err(|_| Overflow)
	};
}

#[derive(Eq, PartialEq, Debug)]
pub enum MathError {
	Overflow,
	InsufficientOutReserve,
	ZeroWeight,
	ZeroReserve,
	ZeroDuration,
	DivisionByZero,
}

#[cfg(test)]
mod conversion_tests {
	use super::MathError::Overflow;
	use crate::types::Balance;
	use crate::types::LBPWeight;
	use core::convert::TryFrom;

	const FIXED_ROUND_UP: Balance = 1;

	#[test]
	fn test_conversion() {
		let one: u32 = 1;
		assert_eq!(to_balance!(one), Ok(1u128));
		assert_eq!(to_lbp_weight!(one), Ok(1u32));
		assert_eq!(round_up!(Balance::from(one)), Ok(2u128));
	}
}
