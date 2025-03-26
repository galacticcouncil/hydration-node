use super::*;
use crate::assert_approx_eq;
use crate::stableswap::math::*;
use crate::stableswap::types::AssetReserve;
use sp_arithmetic::FixedU128;

#[test]
fn test_normalize_value_same_decimals() {
	let amount = 1_000_000_000_000;
	let decimals = 12;
	let target_decimals = 12;
	let expected: Balance = amount;
	let actual = normalize_value(amount, decimals, target_decimals, Rounding::Down);
	assert_eq!(actual, expected);
}

#[test]
fn test_normalize_value_target_greater_than_decimals() {
	let amount = 1_000_000_000_000;
	let decimals = 12;
	let target_decimals = 18;
	let expected: Balance = 1_000_000_000_000_000_000;
	let actual = normalize_value(amount, decimals, target_decimals, Rounding::Down);
	assert_eq!(actual, expected);
}

#[test]
fn test_normalize_value_target_less_than_decimals() {
	let amount: Balance = 1_000_000_000_000_000_000;
	let decimals = 18;
	let target_decimals = 12;
	let expected: Balance = 1_000_000_000_000;
	let actual = normalize_value(amount, decimals, target_decimals, Rounding::Down);
	assert_eq!(actual, expected);
}

#[test]
fn spot_price_calculation_should_work_with_12_decimals() {
	let reserves = vec![
		AssetReserve::new(478_626_000_000_000_000_000, 12),
		AssetReserve::new(487_626_000_000_000_000_000, 12),
		AssetReserve::new(866_764_000_000_000_000_000, 12),
		AssetReserve::new(518_696_000_000_000_000_000, 12),
	];
	let amp = 319u128;
	let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amp, &default_pegs(reserves.len())).unwrap();
	let p =
		calculate_spot_price_between_two_stable_assets(&reserves, amp, d, 0, 1, None, &default_pegs(reserves.len()))
			.unwrap();
	assert_approx_eq!(
		p,
		FixedU128::from_rational(
			259416830506303392284340673024338472588,
			259437723055509887749072196895052016056
		),
		FixedU128::from((2, (1_000_000_000_000u128 / 10_000))),
		"the relative difference is not as expected"
	);
	let reserves = vec![
		AssetReserve::new(1_001_000_000_000_000_000, 12),
		AssetReserve::new(1_000_000_000_000_000_000, 12),
		AssetReserve::new(1_000_000_000_000_000_000, 12),
		AssetReserve::new(1_000_000_000_000_000_000, 12),
	];
	let amp = 10u128;
	let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amp, &default_pegs(reserves.len())).unwrap();
	let p =
		calculate_spot_price_between_two_stable_assets(&reserves, amp, d, 0, 1, None, &default_pegs(reserves.len()))
			.unwrap();
	assert_approx_eq!(
		p,
		FixedU128::from_rational(
			320469570070413807187663384895131457597,
			320440458954331380180651678529102355242
		),
		FixedU128::from((2, (1_000_000_000_000u128 / 10_000))),
		"the relative difference is not as expected"
	);
}

#[test]
fn spot_price_calculation_should_fail_gracefully_with_invalid_indexes() {
	let reserves = vec![
		AssetReserve::new(478_626_000_000_000_000_000, 12),
		AssetReserve::new(487_626_000_000_000_000_000, 12),
		AssetReserve::new(866_764_000_000_000_000_000, 12),
		AssetReserve::new(518_696_000_000_000_000_000, 12),
	];
	let amp = 10u128;
	let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amp, &default_pegs(reserves.len())).unwrap();

	assert!(calculate_spot_price_between_two_stable_assets(
		&reserves,
		amp,
		d,
		4,
		1,
		None,
		&default_pegs(reserves.len())
	)
	.is_none());
	assert!(calculate_spot_price_between_two_stable_assets(
		&reserves,
		amp,
		d,
		1,
		4,
		None,
		&default_pegs(reserves.len())
	)
	.is_none());
}

#[test]
fn share_price_calculation_should_fail_gracefully_with_invalid_indexes() {
	let reserves = vec![
		AssetReserve::new(478_626_000_000_000_000_000, 12),
		AssetReserve::new(487_626_000_000_000_000_000, 12),
		AssetReserve::new(866_764_000_000_000_000_000, 12),
		AssetReserve::new(518_696_000_000_000_000_000, 12),
	];
	let amp = 10u128;

	assert!(calculate_share_price::<MAX_D_ITERATIONS>(
		&reserves,
		amp,
		1000000000000000,
		4,
		None,
		&default_pegs(reserves.len())
	)
	.is_none());
}
