// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes};
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
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));

		let entry = only_pending(ALICE);
		assert_eq!(entry.id, 1);
		assert_eq!(entry.amount, 40 * ONE);
		assert_eq!(entry.expires_at, 1 + GigaHdxCooldownPeriod::get());

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 60 * ONE);
		assert_eq!(s.gigahdx, 60 * ONE);

		// Single combined lock covers active + pending; spendable must be zero.
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 100 * ONE);
		assert_eq!(reducible(ALICE), Balances::free_balance(ALICE) - 100 * ONE);
	});
}

#[test]
fn giga_unstake_should_drain_active_only_when_pot_empty() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 0);
		assert_eq!(s.gigahdx, 0);
		assert_eq!(only_pending(ALICE).amount, 100 * ONE);
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 100 * ONE);
	});
}

#[test]
fn giga_unstake_should_skip_yield_transfer_when_payout_le_active() {
	// pot 200 → rate 3.0; unstake 10 → payout 30 ≤ active 100, no yield needed.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let alice_balance_before = Balances::free_balance(ALICE);
			let pot_before = Balances::free_balance(pot_account());

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE));

			assert_eq!(Stakes::<Test>::get(ALICE).unwrap().hdx, 70 * ONE);
			assert_eq!(only_pending(ALICE).amount, 30 * ONE);
			assert_eq!(Balances::free_balance(ALICE), alice_balance_before);
			assert_eq!(Balances::free_balance(pot_account()), pot_before);
			assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 100 * ONE);
		});
}

#[test]
fn giga_unstake_should_extend_lock_when_payout_exceeds_active() {
	// pot 200 → rate 3.0; unstake 90 → payout 270 > active 100:
	// active drained, yield 170 from pot, lock extends to 270.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let alice_balance_before = Balances::free_balance(ALICE);

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 90 * ONE));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 0);
			assert_eq!(s.gigahdx, 10 * ONE);
			assert_eq!(only_pending(ALICE).amount, 270 * ONE);

			assert_eq!(Balances::free_balance(ALICE), alice_balance_before + 170 * ONE);
			assert_eq!(Balances::free_balance(pot_account()), 30 * ONE);
			assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 270 * ONE);
			// Yield is locked under cooldown — spendable must stay zero.
			assert_eq!(reducible(ALICE), Balances::free_balance(ALICE) - 270 * ONE);
		});
}

#[test]
fn unlock_should_fail_when_cooldown_not_elapsed() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		System::set_block_number(GigaHdxCooldownPeriod::get());
		assert_noop!(
			GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1),
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
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1));

		assert_eq!(pending_count(ALICE), 0);
		// Stakes was {0, 0} after full unstake → cleaned up by unlock.
		assert!(Stakes::<Test>::get(ALICE).is_none());
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 0);
	});
}

#[test]
fn unlock_should_keep_active_lock_when_partial_unstake() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1));

		assert_eq!(pending_count(ALICE), 0);
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 60 * ONE);
		assert_eq!(s.gigahdx, 60 * ONE);
		// Active stake keeps its share of the lock; 40 HDX freed.
		assert_eq!(lock_amount(ALICE, GIGAHDX_LOCK_ID), 60 * ONE);
	});
}

#[test]
fn unlock_should_fail_when_no_pending_position() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1),
			Error::<Test>::PendingUnstakeNotFound
		);
	});
}

#[test]
fn giga_unstake_should_succeed_when_called_after_unlock() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		let unlock_block = 1 + GigaHdxCooldownPeriod::get();
		System::set_block_number(unlock_block);
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1));

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 20 * ONE));
		// Second unstake's id = current block at time of unstake.
		let entry = only_pending(ALICE);
		assert_eq!(entry.id, unlock_block);
		assert_eq!(entry.amount, 20 * ONE);
	});
}

#[test]
fn giga_unstake_should_handle_remaining_atokens_when_active_drained_by_yield() {
	// pot 200 → rate 3.0. First unstake 90 zeroes active and leaves 10 stHDX
	// with zero cost basis; the remaining 10 still unstakes — payout comes
	// entirely from the pot as yield against an empty active stake.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 90 * ONE));
			let unlock_block = 1 + GigaHdxCooldownPeriod::get();
			System::set_block_number(unlock_block);
			assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 0);
			assert_eq!(s.gigahdx, 10 * ONE);

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE));
			let entry = only_pending(ALICE);
			assert_eq!(entry.id, unlock_block);
			assert_eq!(entry.amount, 30 * ONE);
			assert_eq!(Balances::free_balance(pot_account()), 0);
		});
}
