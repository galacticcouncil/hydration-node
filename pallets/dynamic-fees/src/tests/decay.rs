use crate::tests::mock::*;
use crate::tests::oracle::SingleValueOracle;
use sp_runtime::traits::{Bounded, One};
use sp_runtime::FixedU128;

#[test]
fn asset_fee_should_decay_when_volume_has_not_changed() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::max_value(),
			FixedU128::from_float(0.0005),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.0, Fee::from_float(0.0195));
		});
}

#[test]
fn protocol_fee_should_decay_when_volume_has_not_changed() {
	let initial_fee = Fee::from_percent(2);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::max_value(),
			FixedU128::from_float(0.0005),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.1, Fee::from_float(0.0195));
		});
}

#[test]
fn asset_fee_should_not_decay_below_min_limit_when_volume_has_not_changed() {
	let initial_fee = Fee::from_percent(10);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, Fee::zero(), 0)
		.with_asset_fee_params(
			Fee::from_float(0.09),
			Fee::max_value(),
			FixedU128::from_float(0.02),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.0, Fee::from_float(0.09));
		});
}

#[test]
fn protocol_fee_should_not_decay_below_min_limit_when_volume_has_not_changed() {
	let initial_fee = Fee::from_percent(10);

	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, ONE, 50 * ONE))
		.with_initial_fees(initial_fee, initial_fee, 0)
		.with_protocol_fee_params(
			Fee::from_float(0.09),
			Fee::from_float(0.09),
			FixedU128::from_float(0.02),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let fee = retrieve_fee_entry(HDX);

			assert_eq!(fee.1, Fee::from_float(0.09));
		});
}
