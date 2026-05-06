// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, PendingUnstakes, Stakes};
use frame_support::sp_runtime::traits::AccountIdConversion;
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::traits::{fungible::Inspect, LockIdentifier};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use primitives::Balance;

fn pot_account() -> AccountId {
	GigaHdxPalletId::get().into_account_truncating()
}

fn lock_amount(account: AccountId, id: LockIdentifier) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == id)
		.map(|l| l.amount)
		.unwrap_or(0)
}

fn reducible(account: AccountId) -> Balance {
	<Balances as Inspect<AccountId>>::reducible_balance(&account, Preservation::Expendable, Fortitude::Polite)
}

fn stake_alice_100() {
	assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
}

#[test]
fn giga_unstake_should_create_pending_position_when_called() {
	// Empty pot, stake 100, partial unstake 40.
	// payout = 40, active drops 100→60, position = 40, combined lock = 60+40 = 100.
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));

		let entry = PendingUnstakes::<Test>::get(ALICE).expect("entry exists");
		assert_eq!(entry.amount, 40 * ONE);
		assert_eq!(entry.expires_at, 1 + GigaHdxCooldownPeriod::get());

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 60 * ONE);
		assert_eq!(s.gigahdx, 60 * ONE);

		// Single combined lock under GIGAHDX_LOCK_ID covers active + pending.
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 100 * ONE);
		// Spendable strictly zero — no leakage.
		assert_eq!(reducible(ALICE), Balances::free_balance(ALICE) - 100 * ONE);
	});
}

#[test]
fn giga_unstake_should_drain_active_only_when_pot_empty() {
	// Empty pot, stake 100, unstake 100. payout = 100. active drops to 0,
	// no yield transferred. Position = 100.
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 0);
		assert_eq!(s.gigahdx, 0);
		assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 100 * ONE);
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 100 * ONE);
	});
}

#[test]
fn giga_unstake_should_skip_yield_transfer_when_payout_le_active() {
	// Pot 200 → rate 3.0. Stake 100, unstake 10 stHDX → payout 30 ≤ active 100.
	// Active drops 100→70, no pot transfer. Position = 30.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let alice_balance_before = Balances::free_balance(ALICE);
			let pot_before = Balances::free_balance(pot_account());

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE));

			assert_eq!(Stakes::<Test>::get(ALICE).unwrap().hdx_locked, 70 * ONE);
			assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 30 * ONE);
			// Alice's free balance unchanged — no yield transfer (payout came from active).
			assert_eq!(Balances::free_balance(ALICE), alice_balance_before);
			// Pot unchanged.
			assert_eq!(Balances::free_balance(pot_account()), pot_before);
			// Combined lock = 70 + 30 = 100.
			assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 100 * ONE);
		});
}

#[test]
fn giga_unstake_should_extend_lock_when_payout_exceeds_active() {
	// Pot 200 → rate 3.0. Stake 100, unstake 90 stHDX → payout 270 > active 100.
	// Active drops to 0, yield = 170 transferred from pot, lock extends to 270.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let alice_balance_before = Balances::free_balance(ALICE);

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 90 * ONE));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx_locked, 0);
			assert_eq!(s.gigahdx, 10 * ONE);
			assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 270 * ONE);

			// Alice received 170 HDX yield directly into her balance.
			assert_eq!(Balances::free_balance(ALICE), alice_balance_before + 170 * ONE);
			// Pot reduced by 170.
			assert_eq!(Balances::free_balance(pot_account()), 30 * ONE);
			// Combined lock = 0 + 270 = 270 (covers all of Alice's HDX in account).
			assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 270 * ONE);
			// Spendable strictly zero — yield is locked under cooldown.
			assert_eq!(reducible(ALICE), Balances::free_balance(ALICE) - 270 * ONE);
		});
}

#[test]
fn giga_unstake_should_fail_when_pending_position_exists() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE),
			Error::<Test>::PendingUnstakeAlreadyExists
		);
	});
}

#[test]
fn unlock_should_fail_when_cooldown_not_elapsed() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		System::set_block_number(GigaHdxCooldownPeriod::get()); // 1 block early
		assert_noop!(
			GigaHdx::unlock(RawOrigin::Signed(ALICE).into()),
			Error::<Test>::CooldownNotElapsed
		);
	});
}

#[test]
fn unlock_should_release_lock_when_cooldown_elapsed() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into()));

		assert!(PendingUnstakes::<Test>::get(ALICE).is_none());
		// Stakes was {0, 0} after full unstake — should now be cleaned up.
		assert!(Stakes::<Test>::get(ALICE).is_none());
		// Lock fully removed.
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 0);
	});
}

#[test]
fn unlock_should_keep_active_lock_when_partial_unstake() {
	// Stake 100, unstake 40, unlock. Active stake (60) keeps its lock.
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into()));

		assert!(PendingUnstakes::<Test>::get(ALICE).is_none());
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 60 * ONE);
		assert_eq!(s.gigahdx, 60 * ONE);
		// Lock is now just the active stake (40 HDX freed).
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 60 * ONE);
	});
}

#[test]
fn unlock_should_fail_when_no_pending_position() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::unlock(RawOrigin::Signed(ALICE).into()),
			Error::<Test>::NoPendingUnstake
		);
	});
}

#[test]
fn giga_unstake_should_succeed_when_called_after_unlock() {
	// Slot frees up after unlock — caller can unstake again.
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into()));

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 20 * ONE));
		assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 20 * ONE);
	});
}

#[test]
fn giga_unstake_should_handle_remaining_atokens_when_active_drained_by_yield() {
	// Pot 200 → rate 3.0. Stake 100. Unstake 90 → active = 0, gigahdx = 10.
	// Then unstake remaining 10 — case 2 again (active = 0), full payout 30 from pot.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 90 * ONE));
			System::set_block_number(1 + GigaHdxCooldownPeriod::get());
			assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into()));

			// Active stake is gone, but Alice still owns 10 stHDX with zero cost basis.
			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx_locked, 0);
			assert_eq!(s.gigahdx, 10 * ONE);

			// Unstake the remainder.
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE));
			assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 30 * ONE);
			// Pot drained completely.
			assert_eq!(Balances::free_balance(pot_account()), 0);
		});
}
