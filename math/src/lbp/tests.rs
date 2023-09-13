#![allow(clippy::type_complexity)]
use crate::lbp::lbp;

use crate::types::{Balance, LBPWeight, HYDRA_ONE};
use crate::MathError::{Overflow, ZeroDuration, ZeroReserve};

use std::vec;

#[test]
fn spot_price_should_work() {
	let cases = vec![
		(1000, 2000, 500, 500, 100, Ok(200), "Easy case"),
		(0, 0, 0, 0, 100, Err(ZeroReserve), "Zero reserves and weights"),
		(0, 1, 1, 1, 1, Err(ZeroReserve), "Zero sell_reserve"),
		(1, 0, 1, 1, 1, Ok(0), "Zero buy_reserve"),
		(1, 1, 0, 1, 1, Ok(0), "Zero amount"),
		(Balance::MAX, Balance::MAX - 1, 1, 1, 1, Ok(0), "Truncated result"),
		(
			1,
			Balance::MAX,
			LBPWeight::MAX,
			LBPWeight::MAX,
			Balance::MAX,
			Err(Overflow),
			"Overflow weights",
		),
	];

	for case in cases {
		assert_eq!(
			lbp::calculate_spot_price(case.0, case.1, case.2, case.3, case.4),
			case.5,
			"{}",
			case.6
		);
	}
}

#[test]
fn out_given_in_should_work() {
	let cases: Vec<(u128, u128, u32, u32, u128, Result<u128, crate::MathError>, &str)> = vec![
		(1000, 2000, 500, 500, 100, Ok(181), "Easy case"),
		(0, 0, 0, 0, 100, Err(Overflow), "Zero reserves and weights"),
		(1, 1, 1, 1, 0, Ok(0), "Zero out reserve and amount"),
		(0, 0, 1, 1, Balance::MAX, Ok(0), "Zero buy reserve and sell reserve"),
	];

	for case in cases {
		assert_eq!(
			lbp::calculate_out_given_in(case.0, case.1, case.2, case.3, case.4),
			case.5,
			"{}",
			case.6
		);
	}
}

#[test]
fn in_given_out_should_work() {
	let prec: u128 = HYDRA_ONE;
	let cases = vec![
		(1000, 2000, 500, 500, 100, Ok(54), "Easy case"),
		(
			100 * prec,
			20 * prec,
			5_000_000,
			10_000_000,
			prec,
			Ok(10803324099724),
			"Easy case",
		),
		(
			100 * prec,
			20 * prec,
			10_000_000,
			5_000_000,
			prec,
			Ok(2597835208517),
			"Easy case",
		),
		(
			100 * prec,
			340 * prec,
			10_000_000,
			120_000_000,
			2 * prec,
			Ok(7336295198685),
			"Easy case",
		),
		(0, 0, 0, 0, 100, Err(Overflow), "Zero reserves and weights"),
	];

	for case in cases {
		assert_eq!(
			lbp::calculate_in_given_out(case.0, case.1, case.2, case.3, case.4),
			case.5,
			"{}",
			case.6
		);
	}
}

#[test]
fn linear_weights_should_work() {
	let u32_cases = vec![
		(100u32, 200u32, 1_000u32, 2_000u32, 170u32, Ok(1_700), "Easy case"),
		(
			100u32,
			200u32,
			2_000u32,
			1_000u32,
			170u32,
			Ok(1_300),
			"Easy decreasing case",
		),
		(
			100u32,
			200u32,
			2_000u32,
			2_000u32,
			170u32,
			Ok(2_000),
			"Easy constant case",
		),
		(100u32, 200u32, 1_000u32, 2_000u32, 100u32, Ok(1_000), "Initial weight"),
		(
			100u32,
			200u32,
			2_000u32,
			1_000u32,
			100u32,
			Ok(2_000),
			"Initial decreasing weight",
		),
		(
			100u32,
			200u32,
			2_000u32,
			2_000u32,
			100u32,
			Ok(2_000),
			"Initial constant weight",
		),
		(100u32, 200u32, 1_000u32, 2_000u32, 200u32, Ok(2_000), "Final weight"),
		(
			100u32,
			200u32,
			2_000u32,
			1_000u32,
			200u32,
			Ok(1_000),
			"Final decreasing weight",
		),
		(
			100u32,
			200u32,
			2_000u32,
			2_000u32,
			200u32,
			Ok(2_000),
			"Final constant weight",
		),
		(
			200u32,
			100u32,
			1_000u32,
			2_000u32,
			170u32,
			Err(Overflow),
			"Invalid interval",
		),
		(
			100u32,
			100u32,
			1_000u32,
			2_000u32,
			100u32,
			Err(ZeroDuration),
			"Invalid interval",
		),
		(100u32, 200u32, 1_000u32, 2_000u32, 10u32, Err(Overflow), "Out of bound"),
		(
			100u32,
			200u32,
			1_000u32,
			2_000u32,
			210u32,
			Err(Overflow),
			"Out of bound",
		),
	];
	let u64_cases = vec![
		(100u64, 200u64, 1_000u32, 2_000u32, 170u64, Ok(1_700), "Easy case"),
		(
			100u64,
			u64::MAX,
			1_000u32,
			2_000u32,
			200u64,
			Err(Overflow),
			"Interval too long",
		),
	];

	for case in u32_cases {
		assert_eq!(
			lbp::calculate_linear_weights(case.0, case.1, case.2, case.3, case.4),
			case.5,
			"{}",
			case.6
		);
	}
	for case in u64_cases {
		assert_eq!(
			lbp::calculate_linear_weights(case.0, case.1, case.2, case.3, case.4),
			case.5,
			"{}",
			case.6
		);
	}
}
