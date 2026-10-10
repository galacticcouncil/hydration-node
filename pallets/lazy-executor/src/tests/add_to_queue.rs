use crate::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::lazy_executor::{ForwardAction, Source};
use pretty_assertions::assert_eq;
use tests::{has_event, mock::*};

fn forward(amount_out: Balance) -> ForwardAction {
	ForwardAction {
		contract: contract_address(),
		intent_id: 1,
		asset_in: HDX,
		amount_in: 10 * UNIT,
		asset_out: DOT,
		amount_out,
		data: Default::default(),
	}
}

#[test]
fn add_to_queue_should_queue_forward_and_charge_fee_when_owner_can_pay() {
	ExtBuilder::new().build().execute_with(|| {
		//Arrange
		let action = forward(100 * UNIT);
		let alice_balance_before = Balances::free_balance(ALICE);

		//Act
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(0), ALICE, action.clone()));

		//Assert
		assert_eq!(
			LazyExecutor::call_queue(0),
			Some(StoredForward {
				owner: ALICE,
				action: action.clone()
			})
		);
		assert_eq!(LazyExecutor::next_call_id(), 1);

		let fees = 108_158_175_u128;
		assert_eq!(Balances::free_balance(ALICE), alice_balance_before - fees);
		assert!(has_event(
			Event::Queued {
				id: 0,
				src: Source::ICE(0),
				who: ALICE,
				fees,
			}
			.into()
		));
	})
}

#[test]
fn add_to_queue_should_fail_when_owner_cannot_pay_fees() {
	ExtBuilder::new().build().execute_with(|| {
		//Act & Assert
		assert_noop!(
			LazyExecutor::add_to_queue(Source::ICE(1), ACC_ZERO_BALANCE, forward(100 * UNIT)),
			Error::<Test>::FailedToPayFees
		);

		assert_eq!(LazyExecutor::call_queue(0), None);
	})
}
