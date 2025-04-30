use crate::mock::*;
use crate::{AccountGasLimits, Event};
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
		assert!(Dispatcher::account_gas_limits(&ALICE).is_none());
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

		// Act
		let result = Dispatcher::dispatch_with_extra_gas(RuntimeOrigin::signed(ALICE), call, extra_gas);

		// Assert
		assert!(result.is_err());

		// Check no balance was transferred
		assert_eq!(Tokens::free_balance(HDX, &ALICE), alice_initial_balance);
		assert_eq!(Tokens::free_balance(HDX, &BOB), bob_initial_balance);

		// Verify storage was cleaned up even after failure
		assert!(Dispatcher::account_gas_limits(&ALICE).is_none());
	});
}

#[test]
fn get_gas_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Should return 0 when no limit is set
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 0);

		// Set a gas limit through dispatch
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark { remark: vec![] }));
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(ALICE),
			call,
			1000
		));

		// Should return 0 after dispatch (storage is cleaned)
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 0);

		// Manually insert a gas limit
		AccountGasLimits::<Test>::insert(&ALICE, 500u64);
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 500);
	});
}

#[test]
fn decrease_gas_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Should do nothing when no limit is set
		Dispatcher::decrease_gas_limit(&ALICE, 100);
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 0);

		// Set initial gas limit
		AccountGasLimits::<Test>::insert(&ALICE, 1000u64);

		// Decrease by zero should not change anything
		Dispatcher::decrease_gas_limit(&ALICE, 0);
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 1000);

		// Decrease by some amount
		Dispatcher::decrease_gas_limit(&ALICE, 300);
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 700);

		// Decrease by more than remaining should remove the entry
		Dispatcher::decrease_gas_limit(&ALICE, 800);
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 0);
		assert!(AccountGasLimits::<Test>::get(&ALICE).is_none());

		// Set initial gas limit again
		AccountGasLimits::<Test>::insert(&ALICE, 1000u64);

		// Decrease by exact amount should remove the entry
		Dispatcher::decrease_gas_limit(&ALICE, 1000);
		assert_eq!(Dispatcher::get_gas_limit(&ALICE), 0);
		assert!(AccountGasLimits::<Test>::get(&ALICE).is_none());
	});
}
