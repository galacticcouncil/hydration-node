const D_ITERATIONS: u8 = 128;
const Y_ITERATIONS: u8 = 64;

use crate::stableswap::types::AssetReserve;
use crate::stableswap::*;
use crate::types::Balance;
use sp_arithmetic::Permill;

const MAX_BALANCES: usize = 5;

#[test]
fn calculate_ann_should_work_when_correct_values_provided() {
	assert_eq!(calculate_ann(0, 100u128), Some(100u128));
	assert_eq!(calculate_ann(2, 1u128), Some(4u128));
	assert_eq!(calculate_ann(2, 10u128), Some(40u128));
	assert_eq!(calculate_ann(2, 100u128), Some(400u128));
}

#[test]
fn calculate_out_given_in_should_work_when_max_supported_nbr_of_balances_is_provided() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let idx_in: usize = 2;
	let idx_out: usize = 4;

	let amount_in: Balance = 2_000u128;

	let result = calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(&balances, idx_in, idx_out, amount_in, amp);

	assert!(result.is_some());
	let result = result.unwrap();

	assert_eq!(result, 1996u128);
}

#[test]
fn calculate_out_given_in_should_fail_when_asset_idx_is_incorrect() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let amount_in: Balance = 2_000u128;

	let result = calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(&balances, MAX_BALANCES, 1, amount_in, amp);

	assert!(result.is_none());

	let result = calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(&balances, 1, MAX_BALANCES, amount_in, amp);

	assert!(result.is_none());
}

#[test]
fn calculate_in_given_out_should_work_when_max_supported_nbr_of_balances_is_provided() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let idx_in: usize = 2;
	let idx_out: usize = 4;

	let amount_out: Balance = 2_000u128;

	let result = calculate_in_given_out::<D_ITERATIONS, Y_ITERATIONS>(&balances, idx_in, idx_out, amount_out, amp);

	assert!(result.is_some());
	let result = result.unwrap();

	assert_eq!(result, 2004u128);
}

#[test]
fn calculate_in_given_out_should_fail_when_asset_idx_is_incorrect() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let amount_out: Balance = 2_000u128;

	let result = calculate_in_given_out::<D_ITERATIONS, Y_ITERATIONS>(&balances, MAX_BALANCES, 1, amount_out, amp);

	assert!(result.is_none());

	let result = calculate_in_given_out::<D_ITERATIONS, Y_ITERATIONS>(&balances, 1, MAX_BALANCES, amount_out, amp);

	assert!(result.is_none());
}

#[test]
fn calculate_shares_should_work_when_correct_input_provided() {
	let amp = 100_u128;

	let initial_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	let mut updated_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	updated_balances[2].amount += 5000u128;

	let issuance: Balance = 100_000;

	let result = calculate_shares::<D_ITERATIONS>(&initial_balances, &updated_balances, amp, issuance);

	assert!(result.is_some());

	let result = result.unwrap();

	assert_eq!(result, 9993u128);
}

#[test]
fn calculate_shares_should_work_when_share_issuance_is_zero() {
	let amp = 100_u128;

	let initial_balances = [AssetReserve::new(0, 12); MAX_BALANCES];
	let mut updated_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	updated_balances[2].amount += 5000u128;

	let issuance: Balance = 0;

	let result = calculate_shares::<D_ITERATIONS>(&initial_balances, &updated_balances, amp, issuance);

	assert!(result.is_some());

	let result = result.unwrap();

	assert_eq!(result, 54_999u128);
}

#[test]
fn calculate_shares_should_fail_when_balances_len_is_not_equal() {
	let amp = 100_u128;

	let initial_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	let mut updated_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	updated_balances[2].amount += 5000u128;

	let issuance: Balance = 100_000;

	let result = calculate_shares::<D_ITERATIONS>(&initial_balances, &updated_balances, amp, issuance);

	assert!(result.is_none());
}

#[test]
fn calculate_shares_should_fail_when_updated_balances_are_less() {
	let amp = 100_u128;

	let initial_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	let mut updated_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	updated_balances[2].amount -= 5000u128;

	let issuance: Balance = 100_000;

	let result = calculate_shares::<D_ITERATIONS>(&initial_balances, &updated_balances, amp, issuance);

	assert!(result.is_none());
}

#[test]
fn calculate_withdraw_one_asset_should_work_when_max_supported_nbr_of_balances_is_provided() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let asset_index: usize = 2;

	let shares_to_withdraw: Balance = 2_000u128;
	let issuance = 52000u128;

	let fee = Permill::from_percent(50);

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		shares_to_withdraw,
		asset_index,
		issuance,
		amp,
		fee,
	);

	assert!(result.is_some());
	let result = result.unwrap();

	assert_eq!(result, (1440u128, 479u128));
}

#[test]
fn calculate_withdraw_one_asset_should_work_when_fee_is_zero() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let asset_index: usize = 2;

	let shares_to_withdraw: Balance = 2_000u128;
	let issuance = 52000u128;

	let fee = Permill::from_percent(0);

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		shares_to_withdraw,
		asset_index,
		issuance,
		amp,
		fee,
	);

	assert!(result.is_some());
	let result = result.unwrap();

	assert_eq!(result, (384u128 + 1535u128, 0u128));
}

#[test]
fn calculate_withdraw_one_asset_should_work_when_fee_hundred_percent() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let asset_index: usize = 2;

	let shares_to_withdraw: Balance = 2_000u128;
	let issuance = 52000u128;

	let fee = Permill::from_percent(100);

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		shares_to_withdraw,
		asset_index,
		issuance,
		amp,
		fee,
	);
	assert!(result.is_some());

	assert_eq!(result.unwrap(), (960, 959));
}

#[test]
fn calculate_withdraw_one_asset_should_fail_share_issuance_is_zero() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let asset_index: usize = 2;

	let shares_to_withdraw: Balance = 2_000u128;
	let issuance = 0u128;

	let fee = Permill::from_percent(0);

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		shares_to_withdraw,
		asset_index,
		issuance,
		amp,
		fee,
	);

	assert!(result.is_none());
}

#[test]
fn calculate_withdraw_one_asset_should_fail_when_share_issuance_is_less_then_withdrawal() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let asset_index: usize = 2;

	let shares_to_withdraw: Balance = 2_000u128;
	let issuance = 1_000u128;

	let fee = Permill::from_percent(0);

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		shares_to_withdraw,
		asset_index,
		issuance,
		amp,
		fee,
	);

	assert!(result.is_none());
}

#[test]
fn calculate_withdraw_one_asset_should_fail_asset_index_is_outside_boundaries() {
	let amp = 100_u128;

	let balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];

	let asset_index: usize = MAX_BALANCES;

	let shares_to_withdraw: Balance = 2_000u128;
	let issuance = 1_000u128;

	let fee = Permill::from_percent(0);

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		shares_to_withdraw,
		asset_index,
		issuance,
		amp,
		fee,
	);

	assert!(result.is_none());
}

#[test]
fn calculate_withdraw_should_return_correct_amount_when_removing_provided_shares() {
	let amp = 100_u128;

	let fee = Permill::from_percent(0);

	let initial_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	let mut updated_balances = [AssetReserve::new(10_000, 12); MAX_BALANCES];
	updated_balances[2].amount += 5000u128;

	let issuance: Balance = 100_000;

	let result = calculate_shares::<D_ITERATIONS>(&initial_balances, &updated_balances, amp, issuance);
	let shares = result.unwrap();

	let result = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
		&updated_balances,
		shares,
		2,
		issuance + shares,
		amp,
		fee,
	);

	assert!(result.is_some());

	let result = result.unwrap();

	assert_eq!(result, (4993u128, 0u128));
}

#[test]
fn calculate_out_given_in_with_fee_should_work_when_reserves_have_different_precision() {
	let amp = 1000_u128;

	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000, 6),
		AssetReserve::new(3_000_000_000, 6),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000,
		amp,
		Permill::from_percent(1),
	);
	assert_eq!(result.unwrap(), (824_786_715_118_092_963, 8_331_178_940_586_797));

	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		2,
		1,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(1),
	);
	assert_eq!(result.unwrap(), (1_187_653, 11996));
}

#[test]
fn calculate_out_given_in_with_zero_fee_should_work_when_reserves_have_different_precision() {
	let amp = 1000_u128;
	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000, 6),
		AssetReserve::new(3_000_000_000, 6),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000,
		amp,
		Permill::from_percent(0),
	);
	assert_eq!(result.unwrap(), (824_786_715_118_092_963 + 8_331_178_940_586_797, 0));

	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		2,
		1,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(0),
	);
	assert_eq!(result.unwrap(), (1_187_653 + 11996, 0));
}

#[test]
fn calculate_in_given_out_with_fee_should_work_when_reserves_have_different_precision() {
	let amp = 1000_u128;
	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000, 6),
		AssetReserve::new(3_000_000_000, 6),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let result = calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(1),
	);
	assert_eq!(result.unwrap(), (1212376, 12004));

	let result = calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		2,
		1,
		1_000_000,
		amp,
		Permill::from_percent(1),
	);
	assert_eq!(result.unwrap(), (841869902748480839, 8335345571767138));
}

#[test]
fn calculate_in_given_out_with_zero_fee_should_work_when_reserves_have_different_precision() {
	let amp = 1000_u128;

	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000, 6),
		AssetReserve::new(3_000_000_000, 6),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let result = calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(0),
	);
	assert_eq!(result.unwrap(), (1212376 - 12004, 0));

	let result = calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		2,
		1,
		1_000_000,
		amp,
		Permill::from_percent(0),
	);
	assert_eq!(result.unwrap(), (841869902748480839 - 8335345571767138, 0));
}

#[test]
fn test_compare_precision_results_01() {
	let amp = 1000_u128;

	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000_000_000_000_000, 18),
		AssetReserve::new(3_000_000_000_000_000_000_000, 18),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let just_amounts: Vec<u128> = balances.iter().map(|v| v.amount).collect();

	let d_before = calculate_d::<D_ITERATIONS>(&just_amounts, amp).unwrap();
	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(0),
	);
	let updated_reserves = [
		just_amounts[0],
		just_amounts[1] + 1_000_000_000_000_000_000,
		just_amounts[2] - result.unwrap().0,
	];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
	assert_eq!(result.unwrap(), (1_000_079_930_281_397_674, 0));

	let (amount_out, fee) = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		2,
		1,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(0),
	)
	.unwrap();
	assert_eq!((amount_out, fee), (999_919_974_816_739_669, 0));
	let updated_reserves = [
		just_amounts[0],
		just_amounts[1] - amount_out,
		just_amounts[2] + 1_000_000_000_000_000_000,
	];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
}

#[test]
fn test_compare_precision_results_02() {
	let amp = 1000_u128;

	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000, 6),
		AssetReserve::new(3_000_000_000, 6),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let just_amounts: Vec<u128> = balances.iter().map(|v| v.amount).collect();

	let d_before = calculate_d::<D_ITERATIONS>(&just_amounts, amp).unwrap();
	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000,
		amp,
		Permill::from_percent(0),
	);
	let updated_reserves = [just_amounts[0], just_amounts[1] + 1_000_000, just_amounts[2] - result.unwrap().0];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
	assert_eq!(result.unwrap(), (833_117_894_058_679_760, 0));

	let (amount_out, fee) = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		2,
		1,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(0),
	)
	.unwrap();
	assert_eq!((amount_out, fee), (1_187_653 + 11996, 0));
	let updated_reserves = [
		just_amounts[0],
		just_amounts[1] - amount_out,
		just_amounts[2] + 1_000_000_000_000_000_000,
	];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
}

#[test]
fn test_compare_precision_results_03() {
	let amp = 1000_u128;
let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000_000_000_000_000, 18),
		AssetReserve::new(3_000_000_000_000_000_000_000, 18),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let just_amounts: Vec<u128> = balances.iter().map(|v| v.amount).collect();

	let d_before = calculate_d::<D_ITERATIONS>(&just_amounts, amp).unwrap();
	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000_000_000_000_000,
		amp,
		Permill::from_percent(0),
	);
	let updated_reserves = [
		just_amounts[0],
		just_amounts[1] + 1_000_000_000_000_000_000,
		just_amounts[2] - result.unwrap().0,
	];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
	assert_eq!(result.unwrap(), (1_000_079_930_281_397_674, 0));

	let (amount_in, fee) = calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_079_930_281_397_674,
		amp,
		Permill::from_percent(0),
	)
	.unwrap();
	assert_eq!((amount_in, fee), (1000000000000000000, 0));
	let updated_reserves = [
		just_amounts[0],
		just_amounts[1] + amount_in,
		just_amounts[2] - 1_000_079_930_281_397_674,
	];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
}

#[test]
fn test_compare_precision_results_04() {
	let amp = 1000_u128;

	let balances: [AssetReserve; 3] = [
		AssetReserve::new(1_000_000_000, 6),
		AssetReserve::new(3_000_000_000, 6),
		AssetReserve::new(5_000_000_000_000_000_000_000, 18),
	];

	let just_amounts: Vec<u128> = balances.iter().map(|v| v.amount).collect();

	let d_before = calculate_d::<D_ITERATIONS>(&just_amounts, amp).unwrap();
	let result = calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		1_000_000,
		amp,
		Permill::from_percent(0),
	);
	let updated_reserves = [just_amounts[0], just_amounts[1] + 1_000_000, just_amounts[2] - result.unwrap().0];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
	assert_eq!(result.unwrap(), (833_117_894_058_679_760, 0));

	let (amount_in, fee) = calculate_in_given_out_with_fee::<D_ITERATIONS, Y_ITERATIONS>(
		&balances,
		1,
		2,
		833_117_894_058_679_760,
		amp,
		Permill::from_percent(0),
	)
	.unwrap();
	assert_eq!((amount_in, fee), (1000001, 0));
	let updated_reserves = [
		just_amounts[0],
		just_amounts[1] + amount_in,
		just_amounts[2] - 833_117_894_058_679_760,
	];
	let d_after = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
	assert!(d_after >= d_before);
}
