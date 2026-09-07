use crate::*;
use frame_support::assert_ok;
use hydradx_traits::lazy_executor::{ForwardAction, Source};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use tests::{has_event, mock::*};

const AMOUNT_OUT: Balance = 100 * UNIT;
const OWNER_DOT: Balance = 1_000 * UNIT;

fn forward() -> ForwardAction {
	ForwardAction {
		contract: contract_address(),
		intent_id: 1,
		asset_in: HDX,
		amount_in: 10 * UNIT,
		asset_out: DOT,
		amount_out: AMOUNT_OUT,
		data: Default::default(),
	}
}

fn queue_forward() {
	assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), ALICE, forward()));
}

#[test]
fn dispatch_top_should_push_and_commit_when_evm_succeeds_with_correct_ack() {
	ExtBuilder::new()
		.with_tokens(vec![(ALICE, DOT, OWNER_DOT)])
		.build()
		.execute_with(|| {
			//Arrange
			queue_forward();
			set_evm_outcome(EvmOutcome::SucceedCorrectAck);

			//Act
			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

			//Assert
			assert_eq!(Tokens::free_balance(DOT, &ALICE), OWNER_DOT - AMOUNT_OUT);
			assert_eq!(Tokens::free_balance(DOT, &contract_account()), AMOUNT_OUT);
			assert_eq!(LazyExecutor::dispatch_next_id(), 1);
			assert!(has_event(Event::Executed { id: 0, result: Ok(()) }.into()));
		})
}

#[test]
fn dispatch_top_should_rollback_and_leave_owner_whole_when_evm_reverts() {
	ExtBuilder::new()
		.with_tokens(vec![(ALICE, DOT, OWNER_DOT)])
		.build()
		.execute_with(|| {
			//Arrange
			queue_forward();
			set_evm_outcome(EvmOutcome::Revert);

			//Act
			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

			//Assert
			assert_eq!(Tokens::free_balance(DOT, &ALICE), OWNER_DOT);
			assert_eq!(Tokens::free_balance(DOT, &contract_account()), 0);
			// `EvmErrorDecodeMock` yields `Other("Call failed")`, but the message is `#[codec(skip)]`
			// so the event-storage round-trip drops it.
			assert!(has_event(
				Event::Executed {
					id: 0,
					result: Err(DispatchError::Other("")),
				}
				.into()
			));
		})
}

#[test]
fn dispatch_top_should_rollback_when_ack_is_wrong() {
	ExtBuilder::new()
		.with_tokens(vec![(ALICE, DOT, OWNER_DOT)])
		.build()
		.execute_with(|| {
			//Arrange
			queue_forward();
			set_evm_outcome(EvmOutcome::SucceedWrongAck);

			//Act
			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

			//Assert
			assert_eq!(Tokens::free_balance(DOT, &ALICE), OWNER_DOT);
			assert_eq!(Tokens::free_balance(DOT, &contract_account()), 0);
			assert!(has_event(
				Event::Executed {
					id: 0,
					result: Err(Error::<Test>::ForwardFailed.into()),
				}
				.into()
			));
		})
}

#[test]
fn dispatch_top_should_skip_when_owner_has_insufficient_balance() {
	ExtBuilder::new().build().execute_with(|| {
		//Arrange — owner funded for fees (native) but has no `asset_out` to push.
		queue_forward();
		set_evm_outcome(EvmOutcome::SucceedCorrectAck);

		//Act
		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

		//Assert
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 0);
		assert_eq!(Tokens::free_balance(DOT, &contract_account()), 0);
		assert_eq!(LazyExecutor::dispatch_next_id(), 1);
		assert!(has_event(
			Event::Executed {
				id: 0,
				result: Err(orml_tokens::Error::<Test>::BalanceTooLow.into()),
			}
			.into()
		));
	})
}
