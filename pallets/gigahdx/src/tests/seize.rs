// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::traits::Seize;
use crate::{Error, Stakes, TotalLocked};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use primitives::Balance;

fn locked_under_ghdx(account: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

type GigaHdxSeize = crate::Pallet<Test>;

#[test]
fn snapshot_stake_should_return_current_hdx_and_gigahdx_when_active() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let (hdx, gigahdx) = <GigaHdxSeize as Seize<AccountId>>::snapshot_stake(&ALICE).unwrap();
		assert_eq!(hdx, 100 * ONE);
		assert_eq!(gigahdx, 100 * ONE);
	});
}

#[test]
fn snapshot_stake_should_fail_when_no_position() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			<GigaHdxSeize as Seize<AccountId>>::snapshot_stake(&ALICE),
			Error::<Test>::NoStake
		);
	});
}

#[test]
fn pre_seize_should_zero_stakes_gigahdx_and_return_prior_value() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let prev = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		assert_eq!(prev, 100 * ONE);
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, 0);
	});
}

#[test]
fn finalise_seize_should_move_hdx_and_gigahdx_to_recipient() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		// Imitate the post-pre-seize state: gigahdx zeroed.
		let orig_gigahdx = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		let seize_g = 80 * ONE;
		let seize_h = 80 * ONE; // pro-rata 1:1 at seed rate
		let residual = orig_gigahdx - seize_g;

		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
			&ALICE, &TREASURY, seize_h, seize_g, residual
		));

		let alice = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(alice.hdx, 120 * ONE);
		assert_eq!(alice.gigahdx, residual);

		let recipient = Stakes::<Test>::get(TREASURY).unwrap();
		assert_eq!(recipient.hdx, 80 * ONE);
		assert_eq!(recipient.gigahdx, 80 * ONE);
	});
}

#[test]
fn finalise_seize_should_remove_ghdxlock_when_fully_seized() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		assert_eq!(locked_under_ghdx(ALICE), 200 * ONE);

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
			&ALICE,
			&TREASURY,
			200 * ONE,
			orig_g,
			0,
		));

		let entry = pallet_balances::Locks::<Test>::get(ALICE)
			.iter()
			.find(|l| l.id == GIGAHDX_LOCK_ID)
			.cloned();
		assert!(entry.is_none());
	});
}

#[test]
fn finalise_seize_should_refresh_ghdxlock_on_both_accounts() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		assert_eq!(locked_under_ghdx(ALICE), 200 * ONE);
		assert_eq!(locked_under_ghdx(TREASURY), 0);

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
			&ALICE,
			&TREASURY,
			80 * ONE,
			80 * ONE,
			orig_g - 80 * ONE,
		));

		assert_eq!(locked_under_ghdx(ALICE), 120 * ONE);
		assert_eq!(locked_under_ghdx(TREASURY), 80 * ONE);
	});
}

#[test]
fn finalise_seize_should_keep_total_locked_unchanged() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let before = TotalLocked::<Test>::get();

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
			&ALICE,
			&TREASURY,
			80 * ONE,
			80 * ONE,
			orig_g - 80 * ONE,
		));
		assert_eq!(TotalLocked::<Test>::get(), before);
	});
}

#[test]
fn finalise_seize_should_clamp_frozen_to_remaining_hdx() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		crate::Pallet::<Test>::freeze(&ALICE, 150 * ONE);

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
			&ALICE,
			&TREASURY,
			100 * ONE,
			100 * ONE,
			orig_g - 100 * ONE,
		));
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.frozen, 100 * ONE);
	});
}

#[test]
fn finalise_seize_should_fail_when_slash_cannot_take_full_amount() {
	use frame_support::traits::{LockableCurrency, WithdrawReasons};
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let orig_g = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();

		// Foreign lock blocks the transfer branch → slash branch fires.
		<pallet_balances::Pallet<Test> as LockableCurrency<_>>::set_lock(
			*b"foreign_",
			&ALICE,
			1_000 * ONE,
			WithdrawReasons::all(),
		);
		// Drop Alice's balance below the seize amount so slash cannot take it all.
		assert_ok!(pallet_balances::Pallet::<Test>::force_set_balance(
			RawOrigin::Root.into(),
			ALICE,
			50 * ONE,
		));

		assert_noop!(
			<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
				&ALICE,
				&TREASURY,
				100 * ONE,
				100 * ONE,
				orig_g - 100 * ONE,
			),
			Error::<Test>::SeizeFailed
		);
	});
}

#[test]
fn finalise_seize_should_fail_when_borrower_has_no_stake() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			<GigaHdxSeize as Seize<AccountId>>::finalise_seize(&ALICE, &TREASURY, 10 * ONE, 10 * ONE, 0),
			Error::<Test>::NoStake
		);
	});
}

/// REGRESSION FOR audit3 Finding 1 — `seize_finalise` refreshes the
/// borrower's `ghdxlock` AFTER the HDX transfer. When the borrower's free
/// balance equals their staked amount (the realistic case: a user who
/// staked everything they had), the stale pre-seize lock blocks the
/// transfer with `LiquidityRestrictions`. The outer `#[transactional]`
/// rolls back, and the position becomes unliquidatable.
///
/// EXPECTED TO FAIL with the current code. Passes after moving
/// `refresh_lock(borrower)` before the `Currency::transfer` call.
#[test]
fn finalise_seize_should_succeed_when_borrower_has_no_unlocked_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Stake ALL of Alice's free balance — leaves zero transferable
		// headroom because ghdxlock covers the entire account.
		let alice_balance = pallet_balances::Pallet::<Test>::free_balance(ALICE);
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), alice_balance));
		assert_eq!(locked_under_ghdx(ALICE), alice_balance);

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::pre_seize(&ALICE).unwrap();
		let seize_h = alice_balance / 10;
		let seize_g = orig_g / 10;

		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::finalise_seize(
			&ALICE,
			&TREASURY,
			seize_h,
			seize_g,
			orig_g - seize_g,
		));

		// After the fix: lock refreshed before transfer, seize completes,
		// state moves cleanly to the recipient.
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().hdx, alice_balance - seize_h);
		assert_eq!(locked_under_ghdx(ALICE), alice_balance - seize_h);
		assert_eq!(Stakes::<Test>::get(TREASURY).unwrap().hdx, seize_h);
		assert_eq!(locked_under_ghdx(TREASURY), seize_h);
	});
}
