use core::u128;

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
fn valdiate_unsigned_should_work_when_call_id_is_top_of_queue() {
	ExtBuilder::default().build().execute_with(|| {
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

		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0_u128));

		//Act&Assert
		assert_eq!(
			LazyExecutor::validate_unsigned(
				TransactionSource::Local,
				&LazyExecutorCall::dispatch_top {
					call_id: LazyExecutor::dispatch_next_id()
				}
			),
			Ok(ValidTransaction {
				//provides itself
				provides: vec![(OCW_TAG_PREFIX, LazyExecutor::dispatch_next_id()).encode()],
				//first call so it doesn't require previous
				requires: vec![],
				priority: <Test as Config>::UnsignedPriority::get(),
				longevity: <Test as Config>::UnsignedLongevity::get(),
				propagate: false,
			})
		)
	});
}

#[test]
fn validate_unsigned_should_work_when_call_id_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		MaxTxPerBlock::<Test>::set(4);
		let bounded_call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(20_000, 10_000),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundedCall");

		for i in 0..10 {
			assert_ok!(LazyExecutor::add_to_queue(Source::ICE(i), ALICE, bounded_call.clone()));
		}

		assert_eq!(LazyExecutor::next_call_id(), 10);
		//NOTE: dispatch few calls so we are not testing always from zero
		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));
		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 1));

		let dispatch_call_id = LazyExecutor::dispatch_next_id() + 1;
		//Act&Assert
		assert_eq!(
			LazyExecutor::validate_unsigned(
				TransactionSource::Local,
				&LazyExecutorCall::dispatch_top {
					call_id: dispatch_call_id
				}
			),
			Ok(ValidTransaction {
				//provides itself
				provides: vec![(OCW_TAG_PREFIX, dispatch_call_id).encode()],
				//requires previous
				requires: vec![(OCW_TAG_PREFIX, dispatch_call_id - 1).encode()],
				priority: <Test as Config>::UnsignedPriority::get(),
				longevity: <Test as Config>::UnsignedLongevity::get(),
				propagate: false,
			})
		)
	});
}

#[test]
fn validate_unsigned_should_work_when_dispatching_last_call_from_ocw_range() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		MaxTxPerBlock::<Test>::set(3);

		let bounded_call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(20_000, 10_000),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundedCall");

		for i in 0..10 {
			assert_ok!(LazyExecutor::add_to_queue(Source::ICE(i), ALICE, bounded_call.clone()));
		}

		assert_eq!(LazyExecutor::next_call_id(), 10);
		//NOTE: dispatch few calls so we are not testing always from zero
		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));
		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 1));

		//NOTE: -1 to calculate last dispatched id + range
		let dispatch_call_id = (LazyExecutor::dispatch_next_id() - 1) + LazyExecutor::max_txs_per_block() as u128;
		//Act&Assert
		assert_eq!(
			LazyExecutor::validate_unsigned(
				TransactionSource::Local,
				&LazyExecutorCall::dispatch_top {
					call_id: dispatch_call_id
				}
			),
			Ok(ValidTransaction {
				//it's "last" call so it provides none
				provides: vec![],
				//requires previous call
				requires: vec![(OCW_TAG_PREFIX, dispatch_call_id - 1).encode()],
				priority: <Test as Config>::UnsignedPriority::get(),
				longevity: <Test as Config>::UnsignedLongevity::get(),
				propagate: false,
			})
		);
	});
}

#[test]
fn validate_unsigned_should_fail_when_source_is_not_local() {
	ExtBuilder::default().build().execute_with(|| {
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
			LazyExecutor::validate_unsigned(
				TransactionSource::External,
				&LazyExecutorCall::dispatch_top { call_id: 0_u128 }
			),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}

#[test]
fn validate_unsigned_should_fail_when_call_id_is_not_in_queue() {
	ExtBuilder::default().build().execute_with(|| {
		let bounded_call: BoundedCall = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![BOB],
			weight: Weight::from_parts(10_000, 20_000),
		})
		.encode()
		.try_into()
		.expect("failed to create BoundedCall");

		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), ALICE, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(1), ALICE, bounded_call.clone()));
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(1), ALICE, bounded_call));

		assert_noop!(
			LazyExecutor::validate_unsigned(
				TransactionSource::Local,
				&LazyExecutorCall::dispatch_top {
					call_id: u128::max_value()
				}
			),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}

#[test]
fn valdiate_unsigned_should_fail_when_call_was_alredy_dispatched() {
	ExtBuilder::default().build().execute_with(|| {
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

		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0_u128));

		//Act&Assert
		assert_noop!(
			LazyExecutor::validate_unsigned(
				TransactionSource::InBlock,
				&LazyExecutorCall::dispatch_top { call_id: 0_u128 }
			),
			TransactionValidityError::Invalid(InvalidTransaction::Call)
		);
	});
}
