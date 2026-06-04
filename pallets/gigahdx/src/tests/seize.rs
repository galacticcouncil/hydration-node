// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes, TotalLocked};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_traits::gigahdx::Seize;
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
		let prev = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		assert_eq!(prev, 100 * ONE);
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, 0);
	});
}

#[test]
fn finalise_seize_should_move_hdx_and_gigahdx_to_recipient() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		// Imitate the post-pre-seize state: gigahdx zeroed.
		let orig_gigahdx = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		let seize_g = 80 * ONE;
		let seize_h = 80 * ONE; // pro-rata 1:1 at seed rate
		let residual = orig_gigahdx - seize_g;

		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			seize_h,
			seize_g,
			orig_gigahdx
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

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			200 * ONE,
			orig_g,
			orig_g,
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

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			80 * ONE,
			80 * ONE,
			orig_g,
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

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			80 * ONE,
			80 * ONE,
			orig_g,
		));
		assert_eq!(TotalLocked::<Test>::get(), before);
	});
}

#[test]
fn finalise_seize_should_absorb_dust_when_slash_short_by_ed() {
	use frame_support::traits::{Currency, LockableCurrency, WithdrawReasons};
	ExtBuilder::default().build().execute_with(|| {
		// Stake so Alice has a `Stakes` consumer ref → `can_dec_provider` is
		// false → `slash` will refuse to push her below ED.
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let orig_g = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();

		// Foreign TRANSFER-scoped lock forces the seize down the slash branch.
		<pallet_balances::Pallet<Test> as LockableCurrency<_>>::set_lock(
			*b"foreign_",
			&ALICE,
			200 * ONE,
			WithdrawReasons::TRANSFER,
		);
		// Free balance == seize amount: zero slack above the seize, so
		// `slash` will short by exactly ED on a non-reapable account.
		assert_ok!(pallet_balances::Pallet::<Test>::force_set_balance(
			RawOrigin::Root.into(),
			ALICE,
			100 * ONE,
		));

		let ed = <pallet_balances::Pallet<Test> as Currency<AccountId>>::minimum_balance();
		assert!(ed > 0);
		// Force `can_dec_provider(ALICE) == false` (providers == 1, consumers > 0):
		// the H1 trigger condition where `slash` honours the ED floor on a
		// non-reapable account.
		while frame_system::Pallet::<Test>::providers(&ALICE) > 1 {
			frame_system::Pallet::<Test>::dec_providers(&ALICE).unwrap();
		}
		if frame_system::Pallet::<Test>::consumers(&ALICE) == 0 {
			frame_system::Pallet::<Test>::inc_consumers(&ALICE).unwrap();
		}
		assert!(!frame_system::Pallet::<Test>::can_dec_provider(&ALICE));
		let treasury_before = pallet_balances::Pallet::<Test>::free_balance(TREASURY);
		let alice_before = pallet_balances::Pallet::<Test>::free_balance(ALICE);

		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			100 * ONE,
			100 * ONE,
			orig_g,
		));

		// Slash moved exactly `seize_hdx - ed`; the recipient received the
		// same amount via resolve_creating, and ≤ED HDX stayed with Alice
		// as the unslashable existential-deposit dust.
		assert_eq!(
			pallet_balances::Pallet::<Test>::free_balance(TREASURY) - treasury_before,
			100 * ONE - ed,
			"recipient credited with what slash actually moved"
		);
		assert_eq!(
			alice_before - pallet_balances::Pallet::<Test>::free_balance(ALICE),
			100 * ONE - ed,
			"borrower's free balance dropped by exactly the slashed amount"
		);
	});
}

#[test]
fn finalise_seize_should_fail_when_slash_short_by_more_than_ed() {
	use frame_support::traits::{LockableCurrency, WithdrawReasons};
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let orig_g = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();

		// Foreign lock blocks the transfer branch → slash branch fires.
		<pallet_balances::Pallet<Test> as LockableCurrency<_>>::set_lock(
			*b"foreign_",
			&ALICE,
			1_000 * ONE,
			WithdrawReasons::all(),
		);
		// Drop Alice's balance well below the seize amount so slash shorts
		// by much more than ED — this is the Root-A "stake/lock ledger is
		// broken" case the fail-loud tripwire is meant to catch.
		assert_ok!(pallet_balances::Pallet::<Test>::force_set_balance(
			RawOrigin::Root.into(),
			ALICE,
			50 * ONE,
		));

		assert_noop!(
			<GigaHdxSeize as Seize<AccountId>>::on_seize(&ALICE, &TREASURY, 100 * ONE, 100 * ONE, orig_g,),
			Error::<Test>::SeizeFailed
		);
	});
}

#[test]
fn finalise_seize_should_fail_when_borrower_has_no_stake() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			<GigaHdxSeize as Seize<AccountId>>::on_seize(&ALICE, &TREASURY, 10 * ONE, 10 * ONE, 10 * ONE),
			Error::<Test>::NoStake
		);
	});
}

/// REGRESSION FOR audit3 Finding 1 — `on_seize` refreshes the
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

		let orig_g = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		let seize_h = alice_balance / 10;
		let seize_g = orig_g / 10;

		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE, &TREASURY, seize_h, seize_g, orig_g,
		));

		// After the fix: lock refreshed before transfer, seize completes,
		// state moves cleanly to the recipient.
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().hdx, alice_balance - seize_h);
		assert_eq!(locked_under_ghdx(ALICE), alice_balance - seize_h);
		assert_eq!(Stakes::<Test>::get(TREASURY).unwrap().hdx, seize_h);
		assert_eq!(locked_under_ghdx(TREASURY), seize_h);
	});
}

// `on_seize` is handed `seize_gigahdx` measured against the borrower's *live*
// aToken balance, while `orig_gigahdx` is the pallet's ledger snapshot. They
// are equal while the stHDX invariants hold (mint-exclusive + non-borrowable).
// If a future config change ever lets the live balance exceed the snapshot, the
// defensive tripwire must fire: in debug/fuzz builds the `debug_assert` panics
// so the broken invariant is surfaced loudly; release builds compile the assert
// out and clamp so the seize lands instead of underflow-reverting the whole
// liquidation. The two variants pin both halves (CI runs `test --release`).
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "exceeds orig_gigahdx snapshot")]
fn on_seize_should_panic_in_debug_when_seize_gigahdx_exceeds_snapshot() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let orig_gigahdx = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		// Live aToken (150) > snapshot (100) → debug_assert panics.
		let _ = <GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			50 * ONE,
			orig_gigahdx + 50 * ONE,
			orig_gigahdx,
		);
	});
}

#[cfg(not(debug_assertions))]
#[test]
fn on_seize_should_clamp_in_release_when_seize_gigahdx_exceeds_snapshot() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let orig_gigahdx = <GigaHdxSeize as Seize<AccountId>>::on_pre_seize(&ALICE).unwrap();
		assert_eq!(orig_gigahdx, 100 * ONE);

		// Release degrades gracefully: clamps instead of underflow-reverting.
		assert_ok!(<GigaHdxSeize as Seize<AccountId>>::on_seize(
			&ALICE,
			&TREASURY,
			50 * ONE,
			orig_gigahdx + 50 * ONE,
			orig_gigahdx,
		));

		// Borrower's residual gigahdx clamps to zero (never underflows).
		let alice = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(alice.gigahdx, 0);
		assert_eq!(alice.hdx, 50 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 50 * ONE);

		// Recipient is credited the real seized aToken (unclamped — it genuinely
		// received that balance from Aave) and the seized HDX.
		let recipient = Stakes::<Test>::get(TREASURY).unwrap();
		assert_eq!(recipient.hdx, 50 * ONE);
		assert_eq!(recipient.gigahdx, orig_gigahdx + 50 * ONE);
	});
}
