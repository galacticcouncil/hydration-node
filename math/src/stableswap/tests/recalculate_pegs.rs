use super::*;
use crate::stableswap::math::*;
use num_traits::CheckedSub;
use num_traits::Zero;
use sp_arithmetic::FixedU128;
use sp_arithmetic::Perbill;
use sp_arithmetic::Permill;

#[test]
fn recalculate_pegs_should_reach_target_pegs_when_change_is_less_than_max_peg_update() {
	let current_pegs: Vec<(Balance, Balance)> = vec![(85473939039997170, 57767685517430457), (1, 1)];
	let target_pegs: Vec<((Balance, Balance), u128)> = vec![((85561836215176576, 57778334052239089), 10), ((1, 1), 10)];

	let current_pegs_updated_at: u128 = 10;
	let block = 20;

	let max_peg_update: Perbill = Perbill::from_float(0.01);
	let pool_fee: Permill = Permill::from_float(0.02);

	let expected: (Permill, Vec<(Balance, Balance)>) = (
		Permill::from_float(0.02),
		vec![
			(
				259686997534693321553635504599698430064_u128,
				175361852389992385604687093330695209669_u128,
			),
			(1, 1),
		],
	);

	let act = recalculate_pegs(
		&current_pegs,
		current_pegs_updated_at,
		&target_pegs,
		block,
		max_peg_update,
		pool_fee,
	)
	.expect("racalculate_pegs should work");

	assert_eq!(act, expected);

	assert_eq!(
		FixedU128::from_rational(act.1[0].0, act.1[0].1),
		FixedU128::from_rational(target_pegs[0].0 .0, target_pegs[0].0 .1)
	);
}

#[test]
fn recalculate_pegs_should_change_by_max_peg_upade_when_target_peg_change_is_too_big() {
	let current_pegs: Vec<(Balance, Balance)> = vec![(85473939039997170, 57767685517430457), (1, 1)];
	let target_pegs: Vec<((Balance, Balance), u128)> = vec![((95561836215176576, 57778334052239089), 10), ((1, 1), 10)];

	let current_pegs_updated_at: u128 = 10;
	let block = 15;

	let max_peg_update: Perbill = Perbill::from_float(0.01);
	let pool_fee: Permill = Permill::from_float(0.02);

	let expected: (Permill, Vec<(Balance, Balance)>) = (
		Permill::from_float(0.02),
		vec![
			(
				316437573969635447346966143762025421142_u128,
				203680755007369663482566395949026428223_u128,
			),
			(1, 1),
		],
	);

	let act = recalculate_pegs(
		&current_pegs,
		current_pegs_updated_at,
		&target_pegs,
		block,
		max_peg_update,
		pool_fee,
	)
	.expect("racalculate_pegs should work");

	assert_eq!(act, expected);

	let peg_0 = FixedU128::from_rational(current_pegs[0].0, current_pegs[0].1);
	let peg_1 = FixedU128::from_rational(act.1[0].0, act.1[0].1);

	//current_peg * max_peg_update * (block - current_pegs_updated_at) = 0.073...
	assert_eq!(peg_1.sub(peg_0), FixedU128::from_inner(73_980_754_356_349_281));
}

#[test]
fn recalculate_pegs_should_recalculate_fees_when_peg_changes() {
	let current_pegs: Vec<(Balance, Balance)> = vec![(85473939039997170, 57767685517430457), (1, 1)];
	let target_pegs: Vec<((Balance, Balance), u128)> = vec![((95561836215176576, 57778334052239089), 10), ((1, 1), 10)];

	let current_pegs_updated_at: u128 = 10;
	let block = 15;

	let max_peg_update: Perbill = Perbill::from_float(0.015);
	let pool_fee: Permill = Permill::from_float(0.02);

	let expected: (Permill, Vec<(Balance, Balance)>) = (
		Permill::from_float(0.029999),
		vec![
			(
				323971801921293434188560575756359359741_u128,
				203680755007369663482566395949026428223_u128,
			),
			(1, 1),
		],
	);

	let act = recalculate_pegs(
		&current_pegs,
		current_pegs_updated_at,
		&target_pegs,
		block,
		max_peg_update,
		pool_fee,
	)
	.expect("racalculate_pegs should work");

	assert_eq!(act, expected);
}

#[test]
fn recalculate_pegs_should_change_by_1_block_when_udapted_at_and_current_block_are_equal() {
	let current_pegs: Vec<(Balance, Balance)> = vec![(85473939039997170, 57767685517430457), (1, 1)];
	let target_pegs: Vec<((Balance, Balance), u128)> = vec![((95561836215176576, 57778334052239089), 10), ((1, 1), 10)];

	let current_pegs_updated_at: u128 = 10;
	let block = current_pegs_updated_at;

	let max_peg_update: Perbill = Perbill::from_float(0.015);
	let pool_fee: Permill = Permill::from_float(0.02);

	let expected: (Permill, Vec<(Balance, Balance)>) = (
		Permill::from_float(0.029999),
		vec![
			(
				305889654837314265768733938969957907104_u128,
				203680755007369663482566395949026428223_u128,
			),
			(1, 1),
		],
	);

	let act = recalculate_pegs(
		&current_pegs,
		current_pegs_updated_at,
		&target_pegs,
		block,
		max_peg_update,
		pool_fee,
	)
	.expect("racalculate_pegs should work");

	assert_eq!(act, expected);

	let peg_0 = FixedU128::from_rational(current_pegs[0].0, current_pegs[0].1);
	let peg_1 = FixedU128::from_rational(act.1[0].0, act.1[0].1);

	//current_peg * max_peg_update * (block - current_pegs_updated_at) = 0.022...
	assert_eq!(peg_1.sub(peg_0), FixedU128::from_inner(22_194_226_306_904_785));
}

#[test]
fn recalculate_pegs_should_not_change_when_current_and_target_pegs_are_equal() {
	let current_pegs: Vec<(Balance, Balance)> = vec![(85473939039997170, 57767685517430457), (1, 1)];
	let target_pegs: Vec<((Balance, Balance), u128)> = vec![(current_pegs[0], 10), ((1, 1), 10)];

	let current_pegs_updated_at: u128 = 10;
	let block = 100;

	let max_peg_update: Perbill = Perbill::from_float(0.015);
	let pool_fee: Permill = Permill::from_float(0.02);

	let expected: (Permill, Vec<(Balance, Balance)>) = (Permill::from_float(0.02), current_pegs.clone());

	let act = recalculate_pegs(
		&current_pegs,
		current_pegs_updated_at,
		&target_pegs,
		block,
		max_peg_update,
		pool_fee,
	)
	.expect("racalculate_pegs should work");

	assert_eq!(act, expected);
}

#[test]
fn recalculate_pegs_should_not_overshoot_target_pegs() {
	let current_pegs: Vec<(Balance, Balance)> = vec![(85473939039997170, 57767685517430457), (1, 1)];
	let target_pegs: Vec<((Balance, Balance), u128)> = vec![((95561836215176576, 57778334052239089), 10), ((1, 1), 10)];

	let current_pegs_updated_at: u128 = 10;
	let block = 1_000_000;

	let max_peg_update: Perbill = Perbill::from_float(1.0);
	let pool_fee: Permill = Permill::from_float(0.02);

	let expected: (Permill, Vec<(Balance, Balance)>) = (
		Permill::from_float(0.02),
		vec![
			(
				290037795159187406788421537626427421963_u128,
				175361852389992385604687093330695209669_u128,
			),
			(1, 1),
		],
	);

	let act = recalculate_pegs(
		&current_pegs,
		current_pegs_updated_at,
		&target_pegs,
		block,
		max_peg_update,
		pool_fee,
	)
	.expect("racalculate_pegs should work");

	assert_eq!(act, expected);

	let peg_target = FixedU128::from_rational(target_pegs[0].0 .0, target_pegs[0].0 .1);
	let peg_1 = FixedU128::from_rational(act.1[0].0, act.1[0].1);

	assert_eq!(
		peg_1
			.checked_sub(&peg_target)
			.expect("peg_1 - peg_target should now fail"),
		FixedU128::zero()
	);
}
