use crate::tests::mock::*;
use crate::tests::oracle::SingleValueOracle;
use sp_runtime::traits::{Bounded, One, Zero};
use sp_runtime::FixedU128;

#[test]
fn asset_fee_should_not_exceed_max_limit_when_volume_out_increased() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(3),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.0 > initial_fee);

			assert_eq!(fee.0, Fee::from_percent(3));
		});
}

#[test]
fn asset_fee_should_not_fall_below_min_limit_when_volume_in_increased() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(2 * ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_percent(19),
			Fee::max_value(),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.0 < initial_fee);

			assert_eq!(fee.0, Fee::from_percent(19));
		});
}

#[test]
fn protocol_fee_should_not_exceed_max_limit_when_volume_in_increased() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(2 * ONE, ONE, 50 * ONE))
		.with_initial_fees(Fee::zero(), initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(3),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.1 > initial_fee);

			assert_eq!(fee.1, Fee::from_percent(3));
		});
}

#[test]
fn protocol_fee_should_not_fall_bellow_min_limit_when_volume_out_increased() {
	let initial_fee = Fee::from_percent(20);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(Fee::zero(), initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_percent(19),
			Fee::max_value(),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert!(fee.1 < initial_fee);

			assert_eq!(fee.1, Fee::from_percent(19));
		});
}
