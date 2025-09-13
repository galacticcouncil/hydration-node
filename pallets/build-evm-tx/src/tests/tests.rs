// Essential input validation tests for build-evm-tx pallet

use crate::Error;
use frame_support::{assert_noop, assert_ok};

use super::mock::*;

#[test]
fn invalid_address_length_fails() {
	new_test_ext().execute_with(|| {
		// Test with address shorter than 20 bytes
		let short_address = vec![1u8; 19];
		assert_noop!(
			BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(1),
				Some(short_address),
				1000u128,
				vec![],
				1u64,
				21000u128,
				20u128,
				10u128,
				1u64,
			),
			Error::<Test>::InvalidAddressLength
		);

		// Test with address longer than 20 bytes
		let long_address = vec![1u8; 21];
		assert_noop!(
			BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(1),
				Some(long_address),
				1000u128,
				vec![],
				1u64,
				21000u128,
				20u128,
				10u128,
				1u64,
			),
			Error::<Test>::InvalidAddressLength
		);

		// Verify exactly 20 bytes works
		let valid_address = vec![1u8; 20];
		assert_ok!(BuildEvmTx::build_evm_transaction(
			RuntimeOrigin::signed(1),
			Some(valid_address),
			1000u128,
			vec![],
			1u64,
			21000u128,
			20u128,
			10u128,
			1u64,
		));
	});
}

#[test]
fn data_too_long_fails() {
	new_test_ext().execute_with(|| {
		// Create data that exceeds MaxDataLength
		let large_data = vec![0u8; 100_001];
		assert_noop!(
			BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(1),
				None,
				0u128,
				large_data,
				1u64,
				21000u128,
				20u128,
				10u128,
				1u64,
			),
			Error::<Test>::DataTooLong
		);

		// Verify data at exact limit works
		let max_data = vec![0u8; 100_000];
		assert_ok!(BuildEvmTx::build_evm_transaction(
			RuntimeOrigin::signed(1),
			None,
			0u128,
			max_data,
			1u64,
			21000u128,
			20u128,
			10u128,
			1u64,
		));
	});
}

#[test]
fn invalid_gas_price_relationship_fails() {
	new_test_ext().execute_with(|| {
		// Test when max_priority_fee > max_fee
		assert_noop!(
			BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(1),
				None,
				0u128,
				vec![],
				1u64,
				21000u128,
				10u128, // max_fee_per_gas
				20u128, // max_priority_fee_per_gas > max_fee_per_gas
				1u64,
			),
			Error::<Test>::InvalidGasPrice
		);

		// Verify equal values work (priority fee can equal max fee)
		assert_ok!(BuildEvmTx::build_evm_transaction(
			RuntimeOrigin::signed(1),
			None,
			0u128,
			vec![],
			1u64,
			21000u128,
			20u128, // max_fee_per_gas
			20u128, // max_priority_fee_per_gas = max_fee_per_gas
			1u64,
		));

		// Verify priority fee less than max fee works
		assert_ok!(BuildEvmTx::build_evm_transaction(
			RuntimeOrigin::signed(1),
			None,
			0u128,
			vec![],
			2u64,
			21000u128,
			20u128, // max_fee_per_gas
			10u128, // max_priority_fee_per_gas < max_fee_per_gas
			1u64,
		));
	});
}