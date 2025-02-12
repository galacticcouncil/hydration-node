use crate::*;

use super::*;
use frame_support::{assert_noop, assert_ok, weights::Weight};
use pretty_assertions::assert_eq;

#[test]
fn add_to_queue_should_work_when_call_is_valid_and_user_can_pay_fees() {
	ExtBuilder::default().build().execute_with(|| {
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(40_000, 70_000),
		});

		let intent_id: IntentId = 1;
		let origin: AccountId = BOB;
		let bounded_call_data: BoundedCall = call.encode().try_into().unwrap();
		let expected_fees = 107_116_179_u128;

		let bob_balance_0 = Balances::free_balance(BOB);
		assert_eq!(150_000_000_000_000_000_u128, Balances::free_balance(BOB));

		assert_ok!(LazyExecutor::add_to_queue(intent_id, origin, bounded_call_data.clone()));

		assert!(has_event(
			Event::Queued {
				id: 0,
				who: BOB,
				intent_id,
				fees: expected_fees.into()
			}
			.into()
		));

		assert_eq!(LazyExecutor::next_call_id(), 1);
		assert_eq!(LazyExecutor::dispatch_next_id(), 0);
		assert_eq!(
			crate::CallQueue::<Test>::get(0).unwrap(),
			CallData {
				origin: BOB,
				call: bounded_call_data,
			}
		);

		assert_eq!(bob_balance_0 - expected_fees, Balances::free_balance(BOB));
	});
}

#[test]
fn add_to_queue_should_fail_when_call_is_not_valid() {
	ExtBuilder::default().build().execute_with(|| {
		//NOTE: call encoded by PolkadotAPPs with removed last 2 characters
		let call_data: Vec<u8> =
			hex_literal::hex!["070346f0b489ac07cb495852eba68e42250209e4d91f472d37a2fc8e4f0d9c74a828070010a5d4"].into();
		let intent_id: IntentId = 1;
		let origin: AccountId = BOB;

		assert_noop!(
			LazyExecutor::add_to_queue(intent_id, origin, call_data.try_into().unwrap()),
			Error::<Test>::Corrupted
		);
	});
}

#[test]
fn add_to_queue_should_fail_when_origin_cant_pay_fees() {
	ExtBuilder::default().build().execute_with(|| {
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(40_000, 70_000),
		});

		let intent_id: IntentId = 1;
		let origin: AccountId = CHARLIE;
		let bounded_call_data: BoundedCall = call.encode().try_into().unwrap();
		let expected_fees = 107_116_179_u128;

		//NOTE: left Charlie with lower balance than fees
		assert_ok!(Balances::transfer_keep_alive(
			Origin::<Test>::Signed(origin).into(),
			BOB,
			Balances::free_balance(origin) - (expected_fees - 5)
		));

		assert_noop!(
			LazyExecutor::add_to_queue(intent_id, origin, bounded_call_data.clone()),
			Error::<Test>::FailedToPayFees
		);
	});
}
