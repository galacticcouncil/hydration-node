use super::types::*;
use crate::dynamic_fees::{compute_dynamic_fee, recalculate_asset_fee, recalculate_protocol_fee};
use num_traits::Zero;
use sp_arithmetic::{FixedU128, Permill};

#[test]
fn asset_fee_should_decrease_when_in_is_greater_than_out() {
	let volume = OracleEntry {
		amount_in: 25,
		amount_out: 20,
		liquidity: 1000,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 1;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::zero(),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_percent(9);

	let calculated_fee = recalculate_asset_fee(volume, 1025, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn asset_fee_should_increase_when_out_is_greater_than_int() {
	let volume = OracleEntry {
		amount_in: 5,
		amount_out: 20,
		liquidity: 1000,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 1;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::zero(),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_percent(13);

	let calculated_fee = recalculate_asset_fee(volume, 980, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn protocol_fee_should_decrease_when_out_is_greater_than_int() {
	let volume = OracleEntry {
		amount_in: 5,
		amount_out: 20,
		liquidity: 1000,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 1;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::zero(),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_percent(7);

	let calculated_fee = recalculate_protocol_fee(volume, 985, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn protocol_fee_should_increase_when_in_is_greater_than_out() {
	let volume = OracleEntry {
		amount_in: 25,
		amount_out: 20,
		liquidity: 1000,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 1;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::zero(),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_float(0.109999);

	let calculated_fee = recalculate_protocol_fee(volume, 1005, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn asset_fee_should_clamp_to_max_fee() {
	let volume = OracleEntry {
		amount_in: 5,
		amount_out: 20,
		liquidity: 100,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 3;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::from_float(0.001),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_percent(30);

	let calculated_fee = recalculate_asset_fee(volume, 85, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn asset_fee_should_clamp_to_min_fee() {
	let volume = OracleEntry {
		amount_in: 25,
		amount_out: 20,
		liquidity: 100,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 3;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::from_float(0.001),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_percent(1);

	let calculated_fee = recalculate_asset_fee(volume, 105, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn procotol_fee_should_clamp_to_min_fee() {
	let volume = OracleEntry {
		amount_in: 5,
		amount_out: 20,
		liquidity: 100,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 3;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::from_float(0.001),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(30),
	};

	let expected_fee = Permill::from_percent(1);

	let calculated_fee = recalculate_protocol_fee(volume, 85, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn protocol_fee_should_clamp_to_max_fee() {
	let volume = OracleEntry {
		amount_in: 25,
		amount_out: 20,
		liquidity: 100,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let previous_fee = Permill::from_percent(10);
	let last_block_diff = 3;
	let params = FeeParams {
		amplification: FixedU128::from(2),
		decay: FixedU128::from_float(0.001),
		min_fee: Permill::from_percent(1),
		max_fee: Permill::from_percent(15),
	};

	let expected_fee = Permill::from_percent(15);

	let calculated_fee = recalculate_protocol_fee(volume, 105, previous_fee, last_block_diff, params);
	assert_eq!(calculated_fee, expected_fee);
}

#[test]
fn compute_asset_fee_should_increase_when_in_is_greater_than_out() {
	let volume = OracleEntry {
		amount_out: 390_982798935286,
		amount_in: 0,
		liquidity: 200_000_000_000_000_000,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let current_liquidity = 198045_086_005_323_500;
	let initial_fee = Permill::from_float(0.0025);
	let last_block_diff = 1;
	let params = FeeParams {
		amplification: FixedU128::from(10),
		decay: FixedU128::from_rational(5, 10000),
		min_fee: Permill::from_float(0.001),
		max_fee: Permill::from_percent(30),
	};
	let expected_fee = Permill::from_float(0.021549);
	let calculated_asset_fee = compute_dynamic_fee(
		volume.clone(),
		current_liquidity,
		params.clone(),
		initial_fee,
		last_block_diff,
		NetVolumeDirection::OutIn,
	);
	assert!(calculated_asset_fee > initial_fee);
	assert_eq!(calculated_asset_fee, expected_fee);
}

#[test]
fn compute_protocol_fee_should_not_change() {
	let initial_fee = Permill::from_float(0.0005);
	let volume = OracleEntry {
		amount_out: 390_982798935286,
		amount_in: 0,
		liquidity: 200_000_000_000_000_000,
		decay_factor: FixedU128::from_rational(2, 10),
	};
	let current_liquidity = 198045_086_005_323_500;
	let last_block_diff = 1;
	let params = FeeParams {
		amplification: FixedU128::from(10),
		decay: FixedU128::from_rational(1, 10000),
		min_fee: Permill::from_float(0.0005),
		max_fee: Permill::from_percent(30),
	};
	let expected_fee = initial_fee;
	let calculated_protocol_fee = compute_dynamic_fee(
		volume.clone(),
		current_liquidity,
		params.clone(),
		initial_fee,
		last_block_diff,
		NetVolumeDirection::InOut,
	);
	assert_eq!(calculated_protocol_fee, expected_fee);
}

#[test]
fn asset_fee_should_decrease_due_to_decay() {
	let initial_fee = Permill::from_float(0.1);
	let volume = OracleEntry {
		amount_out: 0,
		amount_in: 0,
		liquidity: 100_000_000_000_000_000,
		decay_factor: FixedU128::from_rational(2, 10),
	};

	let current_liquidity = 100_000_000_000_000_000;
	let last_block_diff = 2;
	let params = FeeParams {
		amplification: FixedU128::from(10),
		decay: FixedU128::from_rational(5, 10000),
		min_fee: Permill::from_float(0.0025),
		max_fee: Permill::from_percent(30),
	};
	let calculated_asset_fee = compute_dynamic_fee(
		volume.clone(),
		current_liquidity,
		params.clone(),
		initial_fee,
		last_block_diff,
		NetVolumeDirection::OutIn,
	);
	assert!(calculated_asset_fee < initial_fee);
	let expected_fee = Permill::from_float(0.099);
	assert_eq!(calculated_asset_fee, expected_fee);
}

#[test]
fn protocol_fee_should_decrease_due_to_decay() {
	let initial_fee = Permill::from_float(0.1);
	let volume = OracleEntry {
		amount_out: 0,
		amount_in: 0,
		liquidity: 100_000_000_000_000_000,
		decay_factor: FixedU128::from_rational(2, 10),
	};

	let current_liquidity = 100_000_000_000_000_000;
	let last_block_diff = 2;
	let params = FeeParams {
		amplification: FixedU128::from(10),
		decay: FixedU128::from_rational(1, 10000),
		min_fee: Permill::from_float(0.0005),
		max_fee: Permill::from_percent(30),
	};
	let calculated_asset_fee = compute_dynamic_fee(
		volume.clone(),
		current_liquidity,
		params.clone(),
		initial_fee,
		last_block_diff,
		NetVolumeDirection::InOut,
	);
	assert!(calculated_asset_fee < initial_fee);
	let expected_fee = Permill::from_float(0.0998);
	assert_eq!(calculated_asset_fee, expected_fee);
}
