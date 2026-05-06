// SPDX-License-Identifier: Apache-2.0

//! Arithmetic helpers for `pallet-gigahdx`.
//!
//! All operations use `u128` for inputs/outputs and lift to `U256` for any
//! intermediate product that can exceed `u128::MAX`. Division is floor.
//! Overflow returns `ArithmeticError::Overflow`; never panics.

use primitives::Balance;
use sp_core::U256;
use sp_runtime::ArithmeticError;

/// stHDX to mint for a given HDX `amount`, given current totals
/// `s = total_st_hdx_supply`, `t = TotalLocked + gigapot_balance`.
///
/// Bootstrap (`s == 0` or `t == 0`) returns `amount` unchanged (1:1 rate).
/// Otherwise returns `floor(amount * s / t)` via U256 to avoid overflow.
pub fn st_input_for_stake(amount: Balance, s: Balance, t: Balance) -> Result<Balance, ArithmeticError> {
	if s == 0 || t == 0 {
		return Ok(amount);
	}
	let num = U256::from(amount)
		.checked_mul(U256::from(s))
		.ok_or(ArithmeticError::Overflow)?;
	let q = num.checked_div(U256::from(t)).ok_or(ArithmeticError::DivisionByZero)?;
	q.try_into().map_err(|_| ArithmeticError::Overflow)
}

/// Total HDX paid out for unstaking `st_amount`, given totals
/// `t = TotalLocked + gigapot_balance` and `s = total_st_hdx_supply` BEFORE the
/// unstake.
///
/// Returns `floor(st_amount * t / s)`.
pub fn total_payout(st_amount: Balance, t: Balance, s: Balance) -> Result<Balance, ArithmeticError> {
	if s == 0 {
		return Err(ArithmeticError::DivisionByZero);
	}
	let num = U256::from(st_amount)
		.checked_mul(U256::from(t))
		.ok_or(ArithmeticError::Overflow)?;
	let q = num.checked_div(U256::from(s)).ok_or(ArithmeticError::DivisionByZero)?;
	q.try_into().map_err(|_| ArithmeticError::Overflow)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn st_input_for_stake_should_be_one_to_one_when_bootstrap() {
		assert_eq!(st_input_for_stake(100, 0, 0).unwrap(), 100);
		assert_eq!(st_input_for_stake(100, 0, 50).unwrap(), 100);
		assert_eq!(st_input_for_stake(100, 50, 0).unwrap(), 100);
	}

	#[test]
	fn st_input_for_stake_should_return_fewer_st_when_pot_funded() {
		// 100 staked, 30 in pot, 100 stHDX issued -> rate = 130/100
		// new stake of 60 HDX -> 60 * 100 / 130 = 46
		assert_eq!(st_input_for_stake(60, 100, 130).unwrap(), 46);
	}

	#[test]
	fn st_input_for_stake_should_use_u256_when_inputs_are_large() {
		// amount * s would overflow u128 but not u256
		let big = u128::MAX / 2;
		let r = st_input_for_stake(big, big, big).unwrap();
		assert_eq!(r, big); // s == t -> rate is 1
	}

	#[test]
	fn total_payout_should_include_yield_when_pot_funded() {
		// t = 130, s = 100, st_amount = 100 -> 130 (100 principal + 30 yield)
		assert_eq!(total_payout(100, 130, 100).unwrap(), 130);
	}

	#[test]
	fn total_payout_should_equal_input_when_round_trip_no_pot() {
		// Bootstrap-like state: t = s = 100 -> payout for 100 stHDX is 100.
		let st = st_input_for_stake(100, 0, 0).unwrap();
		assert_eq!(st, 100);
		assert_eq!(total_payout(st, 100, st).unwrap(), 100);
	}

	#[test]
	fn total_payout_should_error_when_supply_is_zero() {
		assert!(matches!(total_payout(10, 5, 0), Err(ArithmeticError::DivisionByZero)));
	}
}
