use super::*;

// use codec::Encode;
use crate::{BoundedCall, CallData, Error};
use codec::Encode;
use frame_support::{assert_err, assert_noop, assert_ok, weights::Weight};
use pretty_assertions::assert_eq;

#[test]
fn validate_call_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let weight = MAX_ALLOWED_WEIGHT;
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call { weight });

		//Act
		assert_ok!(LazyExecutor::validate_call(&call));

		//Assert
		//NOTE: validation should not trigger call execution
		assert!(!has_event(
			mock_pallet::Event::<Test>::CallExecuted { who: ALICE, weight }.into()
		));
	});
}

#[test]
fn validate_call_should_should_return_overweight_when_ref_time_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			weight: Weight::from_parts(MAX_ALLOWED_WEIGHT.ref_time() + 1, MAX_ALLOWED_WEIGHT.proof_size()),
		});

		//Act & assert
		assert_err!(LazyExecutor::validate_call(&call), Error::<Test>::Overweight);
	});
}

#[test]
fn validate_call_should_should_return_overweight_when_proof_size_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			weight: Weight::from_parts(MAX_ALLOWED_WEIGHT.ref_time(), MAX_ALLOWED_WEIGHT.proof_size() + 1),
		});

		//Act & assert
		assert_err!(LazyExecutor::validate_call(&call), Error::<Test>::Overweight);
	});
}

#[test]
fn validate_call_should_should_return_overweight_when_call_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			weight: Weight::from_parts(u64::max_value(), u64::max_value()),
		});

		//Act & assert
		assert_err!(LazyExecutor::validate_call(&call), Error::<Test>::Overweight);
	});
}

#[test]
fn execute_should_queue_call_for_lazy_execution() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			weight: Weight::from_parts(
				MAX_ALLOWED_WEIGHT.ref_time() - 500,
				MAX_ALLOWED_WEIGHT.proof_size() - 1_000,
			),
		});
		let call_data: BoundedCall = call.encode().try_into().unwrap();

		assert_eq!(LazyExecutor::next_queue_id(), 0);
		assert_eq!(LazyExecutor::process_next_id(), 0);
		assert_eq!(LazyExecutor::call_queue(0), None);

		//Act
		assert_ok!(LazyExecutor::execute(ALICE, call_data.clone()));

		//Assert
		assert_eq!(LazyExecutor::next_queue_id(), 1);
		assert_eq!(LazyExecutor::process_next_id(), 0);
		assert_eq!(
			LazyExecutor::call_queue(0),
			Some(CallData {
				origin: ALICE,
				call: call_data,
				created_at: 1,
			})
		);
	});
}

#[test]
fn execute_should_fail_when_bounded_call_cant_be_decoded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		//NOTE: call encoded by PolkadotAPPs with removed last 2 characters
		let call_data: Vec<u8> =
			hex_literal::hex!["070346f0b489ac07cb495852eba68e42250209e4d91f472d37a2fc8e4f0d9c74a828070010a5d4"].into();

		//Act
		assert_noop!(
			LazyExecutor::execute(ALICE, call_data.try_into().unwrap()),
			Error::<Test>::Corrupted
		);
	});
}

#[test]
fn execute_should_fail_when_bounded_call_is_empty() {
	ExtBuilder::default().build().execute_with(|| {
		//Act & assert
		assert_noop!(
			LazyExecutor::execute(ALICE, Vec::new().try_into().unwrap()),
			Error::<Test>::Corrupted
		);
	});
}

#[test]
fn execute_should_fail_when_call_is_overweighted() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			weight: Weight::from_parts(MAX_ALLOWED_WEIGHT.ref_time() + 1, MAX_ALLOWED_WEIGHT.proof_size()),
		});

		//Act & assert
		assert_noop!(
			LazyExecutor::execute(ALICE, call.encode().try_into().unwrap()),
			Error::<Test>::Overweight
		);
	});
}
