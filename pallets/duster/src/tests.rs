use super::*;
use crate::mock::{Duster, ExtBuilder, Origin, Test, Tokens, ALICE, DUSTER, TREASURY};
use frame_support::{assert_noop, assert_ok};

#[test]
fn dust_account_works() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1));
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 100);

			for (who, _, _) in orml_tokens::Accounts::<Test>::iter() {
				assert_ne!(who, *ALICE, "Alice account should have been removed!");
			}

			assert_eq!(Tokens::free_balance(0, &*DUSTER), 10_000);
		});
}
#[test]
fn dust_account_with_sufficient_balance_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 1_000_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_account_with_exact_dust_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_account_with_zero_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 0)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::ZeroBalance
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_nonexisting_account_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::dust_account(Origin::signed(*DUSTER), 123456, 1),
			Error::<Test>::ZeroBalance
		); // Fails with zero balance because total_balance for non-existing account returns default value = Zero.
		assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
	});
}

#[test]
fn dust_treasury_account_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::dust_account(Origin::signed(*DUSTER), *TREASURY, 1),
			Error::<Test>::AccountBlacklisted
		);
	});
}
