use super::*;
use frame_support::assert_ok;
use sp_core::crypto::AccountId32;
#[test]
fn vesting_account_should_not_get_dusted() {
	new_test_ext().execute_with(|| {
		let from: AccountId32 = vesting_account();
		let to: AccountId32 = BOB;

		// This transfer sends balance below Existential Deposit
		assert_ok!(Balances::transfer(
			RawOrigin::Signed(from.clone()).into(),
			to,
			49999500000000000
		));

		assert_eq!(Balances::free_balance(&from), 1_000_000_000_000);
	});
}

#[test]
fn treasury_account_should_not_get_dusted() {
	new_test_ext().execute_with(|| {
		let from: AccountId32 = treasury_account();
		let to: AccountId32 = BOB;

		// This transfer sends balance below Existential Deposit
		assert_ok!(Balances::transfer(
			RawOrigin::Signed(from.clone()).into(),
			to,
			49999500000000000
		));

		assert_eq!(Balances::free_balance(&from), 1_000_000_000_000);
	});
}

#[test]
fn other_accounts_should_get_dusted() {
	new_test_ext().execute_with(|| {
		let from: AccountId32 = ALICE;
		let to: AccountId32 = BOB;

		// This transfer sends balance below Existential Deposit
		assert_ok!(Balances::transfer(
			RawOrigin::Signed(from.clone()).into(),
			to,
			49999500000000000
		));

		assert_eq!(Balances::free_balance(&from), 0);
	});
}
