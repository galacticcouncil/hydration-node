const D_ITERATIONS: u8 = 128;
const Y_ITERATIONS: u8 = 64;

use super::*;
use crate::stableswap::types::AssetReserve;
use crate::stableswap::*;
use sp_arithmetic::Permill;

#[test]
fn test_d() {
	let reserves = [1000u128, 1000u128];
	assert_eq!(
		calculate_d_internal::<D_ITERATIONS>(&reserves, 1),
		Some(2000u128 + 2u128)
	);

	let reserves = [1_000_000_000_000_000_000_000u128, 1_000_000_000_000_000_000_000u128];
	assert_eq!(
		calculate_d_internal::<D_ITERATIONS>(&reserves, 1),
		Some(2_000_000_000_000_000_000_000u128 + 2u128)
	);
}

#[test]
fn test_d_with_zero_reserves() {
	let reserves = [0u128, 0u128];
	assert_eq!(calculate_d_internal::<D_ITERATIONS>(&reserves, 1), Some(0u128));
}

#[test]
fn test_d_with_one_zero_reserves() {
	let reserves = [1000u128, 0u128];
	assert_eq!(calculate_d_internal::<D_ITERATIONS>(&reserves, 1), None);
}

#[test]
fn test_y_given_in() {
	let reserves = [1000u128, 2000u128];

	let amount_in = 100u128;
	assert_eq!(calculate_d_internal::<D_ITERATIONS>(&reserves, 1), Some(2914u128));
	assert_eq!(
		calculate_y_given_in::<D_ITERATIONS, Y_ITERATIONS>(amount_in, 0, 1, &reserves, 1),
		Some(1866u128)
	);
	assert_eq!(
		calculate_d_internal::<D_ITERATIONS>(&[1100u128, 2000u128 - 125u128], 1),
		Some(2925u128)
	);
}

#[test]
fn test_y_given_out() {
	let reserves = [1000u128, 2000u128];

	let amount_out = 100u128;

	let expected_in = 75u128;

	assert_eq!(calculate_d_internal::<D_ITERATIONS>(&reserves, 1), Some(2914u128));

	assert_eq!(
		calculate_y_given_out::<D_ITERATIONS, Y_ITERATIONS>(amount_out, 0, 1, &reserves, 1),
		Some(1000u128 + expected_in)
	);
	assert_eq!(
		calculate_d_internal::<D_ITERATIONS>(&[1000u128 + expected_in, 2000u128 - amount_out], 1),
		Some(2918u128)
	);
}

#[test]
fn test_d_case() {
	let amp = 400u128;

	let result = calculate_d_internal::<D_ITERATIONS>(&[500000000000008580273458u128, 10u128], amp);

	assert!(result.is_some());
}

#[test]
fn test_d_case2() {
	let amp = 168u128;

	let result = calculate_d_internal::<D_ITERATIONS>(&[500000000000000000000010u128, 11u128], amp);

	assert!(result.is_some());
}

#[test]
fn test_case_03() {
	let reserve_in: Balance = 95329220803912837655;
	let reserve_out: Balance = 57374284583541134907;
	let amp: u128 = 310;

	let d = calculate_d_internal::<D_ITERATIONS>(&[reserve_in, reserve_out], amp);

	assert!(d.is_some());
}

#[test]
fn test_shares() {
	let amp = 100u128;

	let initial_reserves = &[AssetReserve::new(0, 12); 2];
	let updated_reserves = &[AssetReserve::new(1000 * ONE, 12), AssetReserve::new(500, 12)];

	let result = calculate_shares::<D_ITERATIONS>(initial_reserves, updated_reserves, amp, 0u128, Permill::zero());

	assert!(result.is_some());
	assert_eq!(result.unwrap(), 736626243363217809);
}
#[test]
fn remove_one_asset_should_work() {
	let amp = 100u128;

	let reserves = &[AssetReserve::new(1000 * ONE, 12), AssetReserve::new(2000u128, 12)];

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		reserves,
		100u128,
		1,
		3000u128,
		amp,
		Permill::from_percent(10),
	);

	assert!(result.is_some());

	let result = result.unwrap();

	assert_eq!(result, (181, 12));
}
