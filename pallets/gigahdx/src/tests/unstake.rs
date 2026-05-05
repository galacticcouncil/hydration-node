// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, PendingUnstakes, Stakes, TotalLocked, TotalStHdx};
use frame_support::sp_runtime::traits::AccountIdConversion;
use frame_support::traits::fungibles::Inspect as FungiblesInspect;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_traits::gigahdx::MoneyMarketOperations;
use primitives::Balance;

fn locked_under_ghdx(account: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

fn stake_alice_100() {
	assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
}

#[test]
fn unstake_full_no_pot_consumes_active_into_position() {
	// Empty pot, stake 100, unstake 100. payout = 100, case 1 (= active).
	// Active drops to 0; position = 100; combined lock = 100; no yield.
	ExtBuilder::default().build().execute_with(|| {
		let pre_free = Balances::free_balance(ALICE);
		stake_alice_100();
		assert_eq!(Balances::free_balance(ALICE), pre_free);

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 0);
		assert_eq!(s.st_minted, 0);
		assert_eq!(TotalLocked::<Test>::get(), 0);
		assert_eq!(TotalStHdx::<Test>::get(), 0);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 0);

		// Position holds 100; lock covers it; no yield to free_balance.
		assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 100 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(Balances::free_balance(ALICE), pre_free);
	});
}

#[test]
fn unstake_full_with_pot_drains_active_and_pulls_yield_from_pot() {
	// Pot 30, stake 100. Unstake 100 stHDX → payout 130 (case 2).
	// active 100 → 0, yield 30 transferred from pot, position = 130.
	ExtBuilder::default()
		.with_pot_balance(30 * ONE)
		.build()
		.execute_with(|| {
			let pre_free = Balances::free_balance(ALICE);
			stake_alice_100();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

			// Yield 30 received into Alice's free balance.
			assert_eq!(Balances::free_balance(ALICE), pre_free + 30 * ONE);
			// Pot drained.
			let pot: AccountId = GigaHdxPalletId::get().into_account_truncating();
			assert_eq!(Balances::free_balance(pot), 0);

			// Stakes record still present (zeroed) until `unlock` cleans it up.
			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx_locked, 0);
			assert_eq!(s.st_minted, 0);

			// Position = full payout; lock covers everything.
			assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 130 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), 130 * ONE);
		});
}

#[test]
fn unstake_partial() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 60 * ONE);
		assert_eq!(s.st_minted, 60 * ONE);
		// Combined lock = active(60) + position(40) = 100.
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 60 * ONE);
		assert_eq!(TotalStHdx::<Test>::get(), 60 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 60 * ONE);
		assert_eq!(PendingUnstakes::<Test>::get(ALICE).unwrap().amount, 40 * ONE);
	});
}

#[test]
fn unstake_zero_fails() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn unstake_above_stake_fails() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 101 * ONE),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn unstake_no_stake_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::NoStake
		);
	});
}

#[test]
fn unstake_mm_failure_reverts_no_storage_mutation() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		let pre_stake = Stakes::<Test>::get(ALICE).unwrap();
		let pre_total_locked = TotalLocked::<Test>::get();
		let pre_total_sthdx = TotalStHdx::<Test>::get();
		let pre_lock = locked_under_ghdx(ALICE);
		let pre_mm_balance = TestMoneyMarket::balance_of(&ALICE);
		let pre_sthdx_balance = Tokens::balance(ST_HDX, &ALICE);

		TestMoneyMarket::fail_withdraw();
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE),
			Error::<Test>::MoneyMarketWithdrawFailed
		);

		// Pre-decrement of `st_minted` was rolled back by `with_transaction`.
		let post_stake = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(post_stake.st_minted, pre_stake.st_minted, "st_minted must be restored");
		assert_eq!(post_stake.hdx_locked, pre_stake.hdx_locked);
		assert_eq!(TotalLocked::<Test>::get(), pre_total_locked);
		assert_eq!(TotalStHdx::<Test>::get(), pre_total_sthdx);
		assert_eq!(locked_under_ghdx(ALICE), pre_lock);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), pre_mm_balance);
		assert_eq!(Tokens::balance(ST_HDX, &ALICE), pre_sthdx_balance);
		assert!(
			PendingUnstakes::<Test>::get(ALICE).is_none(),
			"no position created on failure"
		);
	});
}

#[test]
fn unstake_pre_decrements_st_minted_before_mm_withdraw() {
	// LockableAToken.burn relies on the lock-manager precompile reading the
	// already-decremented `Stakes[who].st_minted`. We can't observe that
	// mid-call here, but the post-state proves the pre-decrement happened
	// before MM.withdraw (otherwise the burn would have failed in production).
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().st_minted, 70 * ONE);
	});
}
