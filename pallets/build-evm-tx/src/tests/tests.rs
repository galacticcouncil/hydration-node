use super::mock::*;
use crate::Error;

#[test]
fn invalid_address_length_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let invalid_address = vec![0x42; 19]; // Should be 20 bytes

		let result = BuildEvmTx::build_evm_tx(
			RuntimeOrigin::signed(1u64),
			Some(invalid_address),
			0,
			vec![],
			0,
			21000,
			20000000000,
			1000000000,
			1,
		);

		assert_eq!(result, Err(Error::<Test>::InvalidAddress.into()));
	});
}

#[test]
fn data_too_long_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let large_data = vec![0xff; 100_001]; // Exceeds MaxDataLength

		let result = BuildEvmTx::build_evm_tx(RuntimeOrigin::signed(1u64), None, 0, large_data, 0, 21000, 20000000000, 1000000000, 1);

		assert_eq!(result, Err(Error::<Test>::DataTooLong.into()));
	});
}

#[test]
fn invalid_gas_price_relationship_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let result = BuildEvmTx::build_evm_tx(
			RuntimeOrigin::signed(1u64),
			None,
			0,
			vec![],
			0,
			21000,
			20000000000, // max_fee_per_gas
			30000000000, // max_priority_fee_per_gas (higher than max_fee)
			1,
		);

		assert_eq!(result, Err(Error::<Test>::InvalidGasPrice.into()));
	});
}

