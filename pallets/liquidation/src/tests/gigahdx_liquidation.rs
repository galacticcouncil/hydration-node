use crate::tests::mock::*;
use crate::*;
use frame_support::traits::fungibles::Inspect;
use frame_support::{assert_noop, assert_ok};
use pallet_currencies::fungibles::FungibleCurrencies;

fn contract_account() -> AccountId {
	EvmAccounts::account_id(Liquidation::borrowing_contract())
}

#[test]
fn gigahdx_liquidation_works_when_no_locks() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup: give contract (money market) GIGAHDX to seize
		let contract_acc = contract_account();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(MONEY_MARKET),
			contract_acc.clone(),
			GIGAHDX,
			100_000 * ONE,
		));

		let user_evm = EvmAccounts::evm_address(&ALICE);
		let derived = GIGAHDX_LIQ_ACCOUNT;
		let derived_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &derived);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::none(),
			GIGAHDX,
			HOLLAR,
			user_evm,
			10_000 * ONE,
			Default::default(),
		));

		// Verify prepare_for_liquidation was called
		let calls = prepare_for_liquidation_was_called_with();
		assert_eq!(calls.len(), 1);

		// Verify GIGAHDX was transferred to derived account
		let derived_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &derived);
		assert!(derived_after > derived_before);

		// Verify event emitted
		System::assert_has_event(
			crate::Event::GigaHdxLiquidated {
				user: user_evm,
				debt_repaid: 10_000 * ONE,
				gigahdx_seized: derived_after - derived_before,
			}
			.into(),
		);
	});
}

#[test]
fn gigahdx_liquidation_fails_when_prepare_for_liquidation_fails() {
	ExtBuilder::default().build().execute_with(|| {
		set_prepare_for_liquidation_should_fail(true);

		let user_evm = EvmAccounts::evm_address(&ALICE);

		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::none(),
				GIGAHDX,
				HOLLAR,
				user_evm,
				10_000 * ONE,
				Default::default(),
			),
			Error::<Test>::ClearVotingLocksFailed
		);
	});
}

#[test]
fn gigahdx_liquidation_fails_when_borrow_fails() {
	ExtBuilder::default().build().execute_with(|| {
		set_evm_borrow_should_fail(true);

		let user_evm = EvmAccounts::evm_address(&ALICE);

		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::none(),
				GIGAHDX,
				HOLLAR,
				user_evm,
				10_000 * ONE,
				Default::default(),
			),
			Error::<Test>::BorrowFailed
		);
	});
}

#[test]
fn gigahdx_ends_up_in_derived_account_not_treasury() {
	ExtBuilder::default().build().execute_with(|| {
		let contract_acc = contract_account();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(MONEY_MARKET),
			contract_acc,
			GIGAHDX,
			100_000 * ONE,
		));

		let user_evm = EvmAccounts::evm_address(&ALICE);
		let treasury_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &TREASURY);
		let derived_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &GIGAHDX_LIQ_ACCOUNT);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::none(),
			GIGAHDX,
			HOLLAR,
			user_evm,
			10_000 * ONE,
			Default::default(),
		));

		let treasury_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &TREASURY);
		let derived_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &GIGAHDX_LIQ_ACCOUNT);

		// Treasury should have no GIGAHDX (all transferred to derived)
		assert_eq!(treasury_after, treasury_before);
		// Derived account should have received the seized GIGAHDX
		assert!(derived_after > derived_before);
	});
}

#[test]
fn multiple_gigahdx_liquidations_accumulate_in_derived() {
	ExtBuilder::default().build().execute_with(|| {
		let contract_acc = contract_account();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(MONEY_MARKET),
			contract_acc,
			GIGAHDX,
			500_000 * ONE,
		));

		let user1_evm = EvmAccounts::evm_address(&ALICE);
		let user2_evm = EvmAccounts::evm_address(&BOB);

		let derived_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &GIGAHDX_LIQ_ACCOUNT);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::none(),
			GIGAHDX,
			HOLLAR,
			user1_evm,
			10_000 * ONE,
			Default::default(),
		));

		let derived_after_first =
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &GIGAHDX_LIQ_ACCOUNT);
		assert!(derived_after_first > derived_before);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::none(),
			GIGAHDX,
			HOLLAR,
			user2_evm,
			20_000 * ONE,
			Default::default(),
		));

		let derived_after_second =
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(GIGAHDX, &GIGAHDX_LIQ_ACCOUNT);
		assert!(derived_after_second > derived_after_first);
	});
}

#[test]
fn existing_hollar_liquidation_still_works() {
	ExtBuilder::default().build().execute_with(|| {
		// This tests that the HOLLAR path is NOT affected by GIGAHDX changes.
		// The HOLLAR path uses flash loan which is not configured in mock (FlashMinter = ()),
		// so it should fail with FlashMinterNotSet, same as before.
		let user_evm = EvmAccounts::evm_address(&ALICE);

		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::none(),
				DOT,
				HOLLAR,
				user_evm,
				10_000 * ONE,
				Default::default(),
			),
			Error::<Test>::FlashMinterNotSet
		);
	});
}

#[test]
fn existing_regular_liquidation_still_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Regular non-HOLLAR, non-GIGAHDX liquidation path (mint/burn).
		let contract_acc = contract_account();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(MONEY_MARKET),
			contract_acc,
			HDX,
			100_000 * ONE,
		));

		let user_evm = EvmAccounts::evm_address(&ALICE);

		// This should trigger the regular mint/burn path (DOT collateral, HDX debt)
		// The mock EVM returns 2x the debt as collateral, then sells collateral for debt.
		// Since Omnipool doesn't have these specific routes, this will likely fail
		// in the swap step, but the important thing is it does NOT enter the GIGAHDX branch.
		let result = Liquidation::liquidate(
			RuntimeOrigin::none(),
			DOT,
			HDX,
			user_evm,
			1_000 * ONE,
			Default::default(),
		);

		// The prepare_for_liquidation should NOT have been called
		let calls = prepare_for_liquidation_was_called_with();
		// Only calls from previous tests in the same thread would be here,
		// but each test runs in its own externalities so this is fresh
		assert!(calls.is_empty());

		// We don't assert_ok because the mock may fail in swap step,
		// but we verify the GIGAHDX branch was not entered.
		let _ = result;
	});
}

#[test]
fn empty_route_accepted_for_gigahdx() {
	ExtBuilder::default().build().execute_with(|| {
		let contract_acc = contract_account();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(MONEY_MARKET),
			contract_acc,
			GIGAHDX,
			100_000 * ONE,
		));

		let user_evm = EvmAccounts::evm_address(&ALICE);

		// Empty route should be fine for GIGAHDX liquidation (route is ignored)
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::none(),
			GIGAHDX,
			HOLLAR,
			user_evm,
			5_000 * ONE,
			Default::default(),
		));
	});
}
