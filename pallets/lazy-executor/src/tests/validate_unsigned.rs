use frame_support::pallet_prelude::{
	InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
};
use frame_support::{assert_noop, assert_ok, traits::Get};
use pretty_assertions::assert_eq;
use sp_runtime::traits::ValidateUnsigned;
use tests::mock::*;

use crate::*;

use super::mock::{ExtBuilder, LazyExecutor, RuntimeCall};

#[test]
fn valdiate_unsigned_should_work_when_queue_is_not_empty() {
	ExtBuilder.build().execute_with(|| {
		//Arrange
		MaxTxPerBlock::<Test>::set(3);

		let bounded_call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(30_000, 10_000),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundedCall");

		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), BOB, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(1), BOB, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(2), ALICE, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(3), BOB, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(4), ALICE, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(5), CHARLIE, bounded_call));

		//Act&Assert
		assert_eq!(
			LazyExecutor::validate_unsigned(TransactionSource::Local, &LazyExecutorCall::dispatch_top {}),
			Ok(ValidTransaction {
				//provides itself
				provides: vec![(OCW_TAG_PREFIX, OCW_PROVIDES.to_vec()).encode()],
				requires: vec![],
				priority: <Test as Config>::UnsignedPriority::get(),
				longevity: <Test as Config>::UnsignedLongevity::get(),
				propagate: false,
			})
		)
	});
}

#[test]
fn validate_unsigned_should_fail_when_source_is_not_local() {
	ExtBuilder.build().execute_with(|| {
		//Arrange
		let bounded_call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(10_000, 20_000),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundedCall");

		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(1), ALICE, bounded_call));

		//Act&Assert
		assert_noop!(
			LazyExecutor::validate_unsigned(TransactionSource::External, &LazyExecutorCall::dispatch_top {}),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}

#[test]
fn validate_unsigned_should_fail_when_queue_is_empty() {
	ExtBuilder.build().execute_with(|| {
		assert_noop!(
			LazyExecutor::validate_unsigned(TransactionSource::Local, &LazyExecutorCall::dispatch_top {}),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}
