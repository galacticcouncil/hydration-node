use crate as dispatcher;
use crate::mock::*;
use crate::Event;
use crate::Weight;
use frame_support::dispatch::Pays;
use frame_support::{assert_noop, assert_ok, dispatch::PostDispatchInfo};
use orml_traits::MultiCurrency;
use sp_runtime::{
	traits::{BlakeTwo256, Hash},
	DispatchError,
};

#[test]
fn dispatch_as_treasury_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: ALICE,
			currency_id: HDX,
			amount: 1_000,
		}));

		let call_hash = BlakeTwo256::hash_of(&call).into();
		let treasury_balance_before = Tokens::free_balance(HDX, &TreasuryAccount::get());

		assert_ok!(Dispatcher::dispatch_as_treasury(RuntimeOrigin::root(), call));

		let treasury_balance_after = Tokens::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_balance_after, treasury_balance_before - 1_000);

		expect_events(vec![Event::TreasuryManagerCallDispatched {
			call_hash: call_hash,
			result: Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes,
			}),
		}
		.into()]);
	});
}

#[test]
fn dispatch_as_treasury_should_fail_when_bad_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
			remark: vec![1],
		}));

		assert_noop!(
			Dispatcher::dispatch_as_treasury(RuntimeOrigin::signed(ALICE), call),
			DispatchError::BadOrigin
		);
		expect_events(vec![]);
	});
}
