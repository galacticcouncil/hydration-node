use super::mock::*;
use crate::{Error, Event};

#[test]
fn invalid_address_length_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let invalid_address = vec![0x42; 19]; // Should be 20 bytes

		let result = BuildEvmTx::build_evm_tx(None, Some(invalid_address), 0, vec![], 0, 21000, 20000000000, 1000000000, 1);

		assert_eq!(result, Err(Error::<Test>::InvalidAddress.into()));
	});
}

#[test]
fn data_too_long_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let large_data = vec![0xff; 100_001]; // Exceeds MaxDataLength

		let result = BuildEvmTx::build_evm_tx(None, None, 0, large_data, 0, 21000, 20000000000, 1000000000, 1);

		assert_eq!(result, Err(Error::<Test>::DataTooLong.into()));
	});
}

#[test]
fn invalid_gas_price_relationship_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let result = BuildEvmTx::build_evm_tx(
			None,
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


#[test]
fn build_evm_tx_helper_emits_event_when_who_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let to_address = vec![0xaa; 20];
		let who = 42u64;

		// Call helper directly with Some(who)
		let rlp = BuildEvmTx::build_evm_tx(
			Some(who),
			Some(to_address),
			1000,
			vec![],
			1,
			21000,
			20_000_000_000,
			1_000_000_000,
			1
		).unwrap();

		// Verify event was emitted from the helper
		System::assert_has_event(RuntimeEvent::BuildEvmTx(Event::EvmTransactionBuilt {
			who,
			rlp_transaction: rlp,
		}));
	});
}

#[test]
fn build_evm_tx_helper_no_event_when_who_none() {
	ExtBuilder::default().build().execute_with(|| {
		let to_address = vec![0xbb; 20];

		// Call helper with None
		BuildEvmTx::build_evm_tx(
			None,
			Some(to_address),
			2000,
			vec![],
			2,
			21000,
			20_000_000_000,
			1_000_000_000,
			1
		).unwrap();

		// Verify no events were emitted
		assert_eq!(System::events().len(), 0);
	});
}
