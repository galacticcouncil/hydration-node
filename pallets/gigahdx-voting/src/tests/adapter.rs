use super::mock::*;
use crate::adapter::GigaHdxVotingCurrency;
use frame_support::traits::{fungible::Inspect, LockIdentifier, LockableCurrency, WithdrawReasons};

const VOTING_LOCK: LockIdentifier = *b"pyconvot";

#[test]
fn combined_balance_includes_gigahdx_and_hdx() {
	ExtBuilder::default().build().execute_with(|| {
		let balance = <GigaHdxVotingCurrency<Test> as Inspect<AccountId>>::total_balance(&ALICE);
		// ALICE has 1_000 HDX + 500 GIGAHDX
		assert_eq!(balance, 1_000 * ONE + 500 * ONE);
	});
}

#[test]
fn set_lock_gigahdx_first_all_gigahdx() {
	ExtBuilder::default().build().execute_with(|| {
		// Lock 300 — ALICE has 500 GIGAHDX, so all goes to GIGAHDX.
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			300 * ONE,
			WithdrawReasons::all(),
		);

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 300 * ONE);
		assert_eq!(split.hdx_amount, 0);

		let evm_lock = crate::GigaHdxVotingLock::<Test>::get(&ALICE);
		assert_eq!(evm_lock, 300 * ONE);
	});
}

#[test]
fn set_lock_gigahdx_first_overflow_to_hdx() {
	ExtBuilder::default().build().execute_with(|| {
		// Lock 700 — ALICE has 500 GIGAHDX + 1000 HDX, so 500 in GIGAHDX + 200 in HDX.
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			700 * ONE,
			WithdrawReasons::all(),
		);

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 500 * ONE);
		assert_eq!(split.hdx_amount, 200 * ONE);

		let evm_lock = crate::GigaHdxVotingLock::<Test>::get(&ALICE);
		assert_eq!(evm_lock, 500 * ONE);
	});
}

#[test]
fn remove_lock_clears_storage() {
	ExtBuilder::default().build().execute_with(|| {
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			700 * ONE,
			WithdrawReasons::all(),
		);

		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::remove_lock(VOTING_LOCK, &ALICE);

		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 0);
		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
	});
}

#[test]
fn extend_lock_increases_split() {
	ExtBuilder::default().build().execute_with(|| {
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			300 * ONE,
			WithdrawReasons::all(),
		);

		// Extend to 600.
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
			VOTING_LOCK,
			&ALICE,
			600 * ONE,
			WithdrawReasons::all(),
		);

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 500 * ONE);
		assert_eq!(split.hdx_amount, 100 * ONE);
	});
}

#[test]
fn extend_lock_does_not_decrease() {
	ExtBuilder::default().build().execute_with(|| {
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			500 * ONE,
			WithdrawReasons::all(),
		);

		// Extend with smaller amount — should not decrease.
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
			VOTING_LOCK,
			&ALICE,
			200 * ONE,
			WithdrawReasons::all(),
		);

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 500 * ONE);
		assert_eq!(split.hdx_amount, 0);
	});
}

#[test]
fn hdx_only_voter_no_gigahdx_lock() {
	ExtBuilder::default()
		.with_endowed(vec![
			(CHARLIE, HDX, 1_000 * ONE),
			// No GIGAHDX for CHARLIE
		])
		.build()
		.execute_with(|| {
			<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
				VOTING_LOCK,
				&CHARLIE,
				500 * ONE,
				WithdrawReasons::all(),
			);

			let split = crate::LockSplit::<Test>::get(&CHARLIE);
			assert_eq!(split.gigahdx_amount, 0);
			assert_eq!(split.hdx_amount, 500 * ONE);

			assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&CHARLIE), 0);
		});
}
