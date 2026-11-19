use crate::tests::mock::*;
use crate::tests::oracle::SingleValueOracle;
use crate::types::{AssetFeeConfig, FeeEntry, FeeParams};
use crate::UpdateAndRetrieveFees;
use frame_support::assert_ok;
use hydradx_traits::fee::GetDynamicFee;
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

			let fee = retrieve_fee_entry(HDX, 49 * ONE);

			assert!(fee.0 > initial_fee);
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

			let fee = retrieve_fee_entry(HDX, 51 * ONE);

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

			let fee = retrieve_fee_entry(HDX, 50 * ONE);

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

			let fee = retrieve_fee_entry(HDX, 51 * ONE);

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

			let fee = retrieve_fee_entry(HDX, 49 * ONE);

			assert!(fee.1 < initial_fee);
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

			let fee = retrieve_fee_entry(HDX, 50 * ONE);

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

			let fee = retrieve_fee_entry(HDX, 51 * ONE);

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

			let fee = retrieve_fee_entry(HDX, 49 * ONE);

			assert!(fee.0 > initial_fee);
			assert!(fee.1 < initial_fee);
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

			let fee = retrieve_fee_entry(HDX, 50 * ONE);

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
			let fee = retrieve_fee_entry(HDX, 49 * ONE);

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

			let (_, protocol_fee) = retrieve_fee_entry(HDX, 49_900_000_000_000);
			//assert_eq!(asset_fee, Fee::from_float(0.012));
			assert_eq!(protocol_fee, Fee::from_percent(2));
		});
}

fn retrieve_base_fee(asset_id: AssetId, liquidity: Balance) -> (Fee, Fee) {
	<UpdateAndRetrieveFees<Test> as GetDynamicFee<(AssetId, Balance)>>::get_base((asset_id, liquidity))
}

fn fee_params(min_fee: Fee) -> FeeParams<Fee> {
	FeeParams {
		min_fee,
		max_fee: Fee::from_percent(40),
		decay: FixedU128::zero(),
		amplification: FixedU128::one(),
	}
}

#[test]
fn get_base_should_return_min_fees_when_asset_config_is_dynamic() {
	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(Fee::from_percent(20), Fee::from_percent(25), 1)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(DynamicFees::set_asset_fee(
				RuntimeOrigin::root(),
				HDX,
				AssetFeeConfig::Dynamic {
					asset_fee_params: fee_params(Fee::from_percent(3)),
					protocol_fee_params: fee_params(Fee::from_percent(2)),
				}
			));

			assert_eq!(
				retrieve_base_fee(HDX, 49 * ONE),
				(Fee::from_percent(3), Fee::from_percent(2))
			);
		});
}

#[test]
fn get_base_should_return_default_min_fees_when_asset_has_no_config() {
	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(Fee::from_percent(20), Fee::from_percent(25), 1)
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

			assert_eq!(
				retrieve_base_fee(HDX, 49 * ONE),
				(Fee::from_percent(1), Fee::from_percent(2))
			);
		});
}

#[test]
fn get_base_should_return_fixed_fees_when_asset_config_is_fixed() {
	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(Fee::from_percent(20), Fee::from_percent(25), 1)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(DynamicFees::set_asset_fee(
				RuntimeOrigin::root(),
				HDX,
				AssetFeeConfig::Fixed {
					asset_fee: Fee::from_percent(5),
					protocol_fee: Fee::from_percent(2),
				}
			));

			assert_eq!(
				retrieve_base_fee(HDX, 49 * ONE),
				(Fee::from_percent(5), Fee::from_percent(2))
			);
		});
}

#[test]
fn get_base_should_not_change_stored_fee_when_called() {
	ExtBuilder::default()
		.with_oracle(SingleValueOracle::new(ONE, 2 * ONE, 50 * ONE))
		.with_initial_fees(Fee::from_percent(20), Fee::from_percent(25), 0)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(DynamicFees::set_asset_fee(
				RuntimeOrigin::root(),
				HDX,
				AssetFeeConfig::Dynamic {
					asset_fee_params: fee_params(Fee::from_percent(3)),
					protocol_fee_params: fee_params(Fee::from_percent(2)),
				}
			));

			retrieve_base_fee(HDX, 49 * ONE);

			assert_eq!(
				DynamicFees::current_fees(HDX),
				Some(FeeEntry {
					asset_fee: Fee::from_percent(20),
					protocol_fee: Fee::from_percent(25),
					timestamp: 0,
				})
			);
		});
}
