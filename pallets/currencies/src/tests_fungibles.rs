#![cfg(test)]

use super::*;
use crate::fungibles::FungibleCurrencies;
use frame_support::{assert_noop, assert_ok, traits::tokens::fungibles::*};
use mock::*;

#[test]
fn fungibles_inspect_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![(ALICE, NATIVE_CURRENCY_ID, 100), (BOB, X_TOKEN_ID, 200)])
		.build()
		.execute_with(|| {
			assert_eq!(FungibleCurrencies::<Runtime>::total_issuance(NATIVE_CURRENCY_ID), 100);
			assert_eq!(FungibleCurrencies::<Runtime>::total_issuance(X_TOKEN_ID), 200);

			assert_eq!(FungibleCurrencies::<Runtime>::minimum_balance(NATIVE_CURRENCY_ID), 2);
			assert_eq!(FungibleCurrencies::<Runtime>::minimum_balance(X_TOKEN_ID), 3);

			assert_eq!(PalletBalances::free_balance(&ALICE), 100);
			assert_eq!(Tokens::free_balance(X_TOKEN_ID, &BOB), 200);

			assert_eq!(
				FungibleCurrencies::<Runtime>::reducible_balance(NATIVE_CURRENCY_ID, &ALICE, true),
				98
			);
			assert_eq!(
				FungibleCurrencies::<Runtime>::reducible_balance(NATIVE_CURRENCY_ID, &ALICE, false),
				100
			);
			assert_eq!(
				FungibleCurrencies::<Runtime>::reducible_balance(X_TOKEN_ID, &BOB, true),
				197
			);
			assert_eq!(
				FungibleCurrencies::<Runtime>::reducible_balance(X_TOKEN_ID, &BOB, false),
				200
			);

			assert_ok!(FungibleCurrencies::<Runtime>::can_deposit(NATIVE_CURRENCY_ID, &ALICE, 1, false).into_result());
			assert_ok!(FungibleCurrencies::<Runtime>::can_deposit(X_TOKEN_ID, &BOB, 1, false).into_result());

			assert_ok!(FungibleCurrencies::<Runtime>::can_withdraw(NATIVE_CURRENCY_ID, &ALICE, 1).into_result());
			assert_ok!(FungibleCurrencies::<Runtime>::can_withdraw(X_TOKEN_ID, &BOB, 1).into_result());

			assert!(FungibleCurrencies::<Runtime>::asset_exists(NATIVE_CURRENCY_ID));
			assert!(FungibleCurrencies::<Runtime>::asset_exists(X_TOKEN_ID));
			assert!(!FungibleCurrencies::<Runtime>::asset_exists(100));
		});
}

#[test]
fn fungibles_mutate_trait_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(FungibleCurrencies::<Runtime>::mint_into(X_TOKEN_ID, &ALICE, 10));
		assert_eq!(Tokens::free_balance(X_TOKEN_ID, &ALICE), 10);

		assert_ok!(FungibleCurrencies::<Runtime>::mint_into(NATIVE_CURRENCY_ID, &BOB, 10));
		assert_eq!(PalletBalances::free_balance(&BOB), 10);

		assert_eq!(FungibleCurrencies::<Runtime>::burn_from(X_TOKEN_ID, &ALICE, 4), Ok(4));
		assert_eq!(Tokens::free_balance(X_TOKEN_ID, &ALICE), 6);

		assert_eq!(
			FungibleCurrencies::<Runtime>::burn_from(NATIVE_CURRENCY_ID, &BOB, 4),
			Ok(4)
		);
		assert_eq!(PalletBalances::free_balance(&BOB), 6);
	});
}

#[test]
fn fungibles_transfer_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![(ALICE, NATIVE_CURRENCY_ID, 100), (BOB, X_TOKEN_ID, 100)])
		.build()
		.execute_with(|| {
			assert_noop!(
				FungibleCurrencies::<Runtime>::transfer(NATIVE_CURRENCY_ID, &ALICE, &BOB, 100, true),
				pallet_balances::Error::<Runtime>::KeepAlive
			);
			assert_ok!(FungibleCurrencies::<Runtime>::transfer(
				NATIVE_CURRENCY_ID,
				&ALICE,
				&BOB,
				10,
				true
			));
			assert_eq!(PalletBalances::free_balance(&ALICE), 90);
			assert_eq!(PalletBalances::free_balance(&BOB), 10);

			assert_noop!(
				FungibleCurrencies::<Runtime>::transfer(X_TOKEN_ID, &BOB, &ALICE, 100, true),
				orml_tokens::Error::<Runtime>::KeepAlive
			);
			assert_ok!(FungibleCurrencies::<Runtime>::transfer(
				X_TOKEN_ID, &BOB, &ALICE, 10, true
			));
			assert_eq!(Tokens::free_balance(X_TOKEN_ID, &BOB), 90);
			assert_eq!(Tokens::free_balance(X_TOKEN_ID, &ALICE), 10);
		});
}
