use crate::*;
use frame_support::pallet_prelude::{
	InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
};
use frame_support::{assert_noop, assert_ok, traits::Get};
use hydradx_traits::lazy_executor::{ForwardAction, Source};
use pretty_assertions::assert_eq;
use sp_runtime::traits::ValidateUnsigned;
use tests::mock::*;

fn forward() -> ForwardAction {
	ForwardAction {
		contract: contract_address(),
		intent_id: 1,
		asset_in: HDX,
		amount_in: 10 * UNIT,
		asset_out: DOT,
		amount_out: 100 * UNIT,
		data: Default::default(),
	}
}

#[test]
fn validate_unsigned_should_work_when_queue_is_not_empty() {
	ExtBuilder::new().build().execute_with(|| {
		//Arrange
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), ALICE, forward()));

		let id = 0_u128;

		//Act & Assert
		assert_eq!(
			LazyExecutor::validate_unsigned(TransactionSource::Local, &LazyExecutorCall::dispatch_top { id }),
			Ok(ValidTransaction {
				provides: vec![(OCW_TAG_PREFIX, id).encode()],
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
	ExtBuilder::new().build().execute_with(|| {
		//Arrange
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), ALICE, forward()));

		//Act & Assert
		assert_noop!(
			LazyExecutor::validate_unsigned(TransactionSource::External, &LazyExecutorCall::dispatch_top { id: 0 }),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}

#[test]
fn validate_unsigned_should_fail_when_queue_is_empty() {
	ExtBuilder::new().build().execute_with(|| {
		assert_noop!(
			LazyExecutor::validate_unsigned(TransactionSource::Local, &LazyExecutorCall::dispatch_top { id: 0 }),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}
