use crate::tests::mock::*;
use crate::tests::oracle::SingleValueOracle;
use sp_runtime::traits::{One, Zero};
use sp_runtime::FixedU128;

#[test]
fn asset_fee_should_increase_when_volume_out_increased() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.0 > initial_fee);

			assert_eq!(fee.0, Fee::from_percent(4));
		});
}

#[test]
fn asset_fee_should_decrease_when_volume_in_increased() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(2 * ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.0 < initial_fee);

			assert_eq!(fee.0, Fee::from_percent(18));
		});
}

#[test]
fn asset_fee_should_not_change_when_volume_has_not_changed_and_decay_is_0() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.0, initial_fee);
		});
}

#[test]
fn protocol_fee_should_increase_when_volume_in_increased() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(2 * ONE, ONE, 50 * ONE))
		.with_initial_fees(Fee::zero(), initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.1 > initial_fee);

			assert_eq!(fee.1, Fee::from_percent(4));
		});
}

#[test]
fn protocol_fee_should_decrease_when_volume_out_increased() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(Fee::zero(), initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.1 < initial_fee);

			assert_eq!(fee.1, Fee::from_percent(18));
		});
}

#[test]
fn protocol_fee_should_not_change_when_volume_has_not_changed_and_decay_is_0() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.1, initial_fee);
		});
}

#[test]
fn fees_should_update_correctly_when_volume_in_increased() {
	let initial_fee = Fee::from_percent(10);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(2 * ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.0 < initial_fee);
			assert!(fee.1 > initial_fee);

			assert_eq!(fee.0, Fee::from_percent(8));
			assert_eq!(fee.1, Fee::from_percent(12));
		});
}

#[test]
fn fees_should_update_correctly_when_volume_out_increased() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.0 > initial_fee);
			assert!(fee.1 < initial_fee);

			assert_eq!(fee.0, Fee::from_percent(22));
			assert_eq!(fee.1, Fee::from_percent(18));
		});
}

#[test]
fn fees_should_not_change_when_volume_has_not_changed_and_decay_is_0() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.0, initial_fee);
			assert_eq!(fee.1, initial_fee);
		});
}

#[test]
fn fees_should_not_change_when_already_update_within_same_block() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 1)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.0, initial_fee);
			assert_eq!(fee.1, initial_fee);
		});
}

#[test]
fn fees_should_be_recalculated_correctly_for_last_block_change_when_nothing_in_storage() {
	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 1_100_000_000_000, 50 * ONE))
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.with_protocol_fee_params(
			Fee::from_percent(2),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let (asset_fee, protocol_fee) = retrieve_fee_entry(HDX);
			assert_eq!(asset_fee, Fee::from_float(0.012));
			assert_eq!(protocol_fee, Fee::from_percent(2));
		});
}
