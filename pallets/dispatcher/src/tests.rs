use crate::mock::*;
use crate::{Event, ExtraGas};
use frame_support::dispatch::{DispatchErrorWithPostInfo, Pays};
use frame_support::pallet_prelude::Weight;
use frame_support::{assert_noop, assert_ok, dispatch::PostDispatchInfo};
use orml_tokens::Error;
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

		let call_hash = BlakeTwo256::hash_of(&call);
		let treasury_balance_before = Tokens::free_balance(HDX, &TreasuryAccount::get());

		assert_ok!(Dispatcher::dispatch_as_treasury(RuntimeOrigin::root(), call));

		let treasury_balance_after = Tokens::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_balance_after, treasury_balance_before - 1_000);

		expect_events(vec![Event::TreasuryManagerCallDispatched {
			call_hash,
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

#[test]
fn dispatch_with_extra_gas_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: 1_000,
		}));

		let alice_initial_balance = Tokens::free_balance(HDX, &ALICE);
		let bob_initial_balance = Tokens::free_balance(HDX, &BOB);
		let extra_gas = 1_000_000_000;

		// Act
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(ALICE),
			call,
			extra_gas
		));

		// Assert
		// Check balance was transferred
		assert_eq!(Tokens::free_balance(HDX, &ALICE), alice_initial_balance - 1_000);
		assert_eq!(Tokens::free_balance(HDX, &BOB), bob_initial_balance + 1_000);

		// Verify storage was cleaned up
		assert_eq!(Dispatcher::extra_gas(), 0);
	});
}

#[test]
fn dispatch_with_extra_gas_should_fail_when_call_fails() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange - try to transfer more than available balance
		let alice_initial_balance = Tokens::free_balance(HDX, &ALICE);
		let bob_initial_balance = Tokens::free_balance(HDX, &BOB);

		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: alice_initial_balance + 1, // more than ALICE has
		}));

		let extra_gas = 1_000_000_000;

		let r = DispatchErrorWithPostInfo {
			post_info: PostDispatchInfo {
				actual_weight: Some(Weight::zero()),
				pays_fee: Pays::Yes,
			},
			error: Error::<Test>::BalanceTooLow.into(),
		};
		// Act
		assert_noop!(
			Dispatcher::dispatch_with_extra_gas(RuntimeOrigin::signed(ALICE), call, extra_gas),
			r
		);

		// Assert
		// Check no balance was transferred
		assert_eq!(Tokens::free_balance(HDX, &ALICE), alice_initial_balance);
		assert_eq!(Tokens::free_balance(HDX, &BOB), bob_initial_balance);

		// Verify storage was cleaned up even after failure
		assert_eq!(Dispatcher::extra_gas(), 0u64);
	});
}

#[test]
fn get_gas_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Should return 0 when no limit is set
		assert_eq!(Dispatcher::extra_gas(), 0);

		// Set a gas limit through dispatch
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark { remark: vec![] }));
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(ALICE),
			call,
			1000
		));

		// Should return 0 after dispatch (storage is cleaned)
		assert_eq!(Dispatcher::extra_gas(), 0);

		// Manually insert a gas limit
		ExtraGas::<Test>::set(500u64);
		assert_eq!(Dispatcher::extra_gas(), 500);
	});
}

#[test]
fn decrease_gas_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Should do nothing when no limit is set
		Dispatcher::decrease_extra_gas(100);
		assert_eq!(Dispatcher::extra_gas(), 0);

		// Set initial gas limit
		ExtraGas::<Test>::set(1000u64);

		// Decrease by zero should not change anything
		Dispatcher::decrease_extra_gas(0);
		assert_eq!(Dispatcher::extra_gas(), 1000);

		// Decrease by some amount
		Dispatcher::decrease_extra_gas(300);
		assert_eq!(Dispatcher::extra_gas(), 700);

		// Decrease by more than remaining should remove the entry
		Dispatcher::decrease_extra_gas(800);
		assert_eq!(Dispatcher::extra_gas(), 0);
		assert_eq!(ExtraGas::<Test>::get(), 0u64);

		// Set initial gas limit again
		ExtraGas::<Test>::set(1000u64);

		// Decrease by exact amount should remove the entry
		Dispatcher::decrease_extra_gas(1000);
		assert_eq!(Dispatcher::extra_gas(), 0);
		assert_eq!(ExtraGas::<Test>::get(), 0u64);
	});
}
