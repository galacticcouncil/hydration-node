use crate::*;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;
use tests::{has_event, mock::*};

#[test]
fn add_to_queue_should_work_when_call_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE, BOB],
			weight: Weight::from_parts(1_000_u64, 1_000_u64),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundedCall");

		//Act&Assert
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), ALICE, call));

		assert!(has_event(
			Event::Queued {
				id: 0,
				src: Source::ICE(0),
				who: ALICE,
				fees: 107_077_175_u128
			}
			.into()
		))
	})
}

#[test]
fn add_to_queue_should_fail_when_call_is_not_decodeable() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		//NOTE: call encoded from PolkadotAPPs with removed last 2 characters
		let corrupted_call: BoundedCall = Into::<Vec<u8>>::into(hex_literal::hex![
			"070346f0b489ac07cb495852eba68e42250209e4d91f472d37a2fc8e4f0d9c74a828070010a5d4"
		])
		.try_into()
		.expect("failed to create BoundeCall");

		//Act&Assert
		assert_noop!(
			LazyExecutor::add_to_queue(Source::ICE(0), ALICE, corrupted_call),
			Error::<Test>::Corrupted
		);
	});
}

#[test]
fn add_to_queue_should_fail_when_call_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let max_allowed_weight = LazyExecutor::max_weight_per_call();

		//NOTE: this is overweight because weight of dispatching call is added to call's weight
		let overweight_ref_time_call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(max_allowed_weight.ref_time(), 1_u64),
		})
		.encode()
		.try_into()
		.expect("failed to create overweight_ref_time BoundedCall");

		//NOTE: this is overweight because weight of dispatching call is added to call's weight
		let overweight_proof_size_cal: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(1_u64, max_allowed_weight.proof_size()),
		})
		.encode()
		.try_into()
		.expect("failed to create overweight_proof_size BoundeCall");

		//Act&Assert - 1
		assert_noop!(
			LazyExecutor::add_to_queue(Source::ICE(0), ALICE, overweight_ref_time_call),
			Error::<Test>::Overweight
		);

		//Act&Assert - 2
		assert_noop!(
			LazyExecutor::add_to_queue(Source::ICE(0), ALICE, overweight_proof_size_cal),
			Error::<Test>::Overweight
		);
	});
}

#[test]
fn add_to_queue_should_fail_when_origin_cant_pay_fees() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			//NOTE: whole call includes dispatch overhead so we need to substract more
			weight: Weight::from_parts(100_u64, 100_u64),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundeCall");

		//Act&Assert
		assert_noop!(
			LazyExecutor::add_to_queue(Source::ICE(1), ACC_ZERO_BALANCE, call),
			Error::<Test>::FailedToPayFees
		);
	})
}
