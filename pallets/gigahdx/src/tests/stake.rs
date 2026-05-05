// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes, TotalLocked, TotalStHdx};
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

#[test]
fn stake_locks_correct_amount_and_records_actual_minted() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 100 * ONE);
		assert_eq!(s.st_minted, 100 * ONE); // bootstrap 1:1, no rounding
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
		assert_eq!(TotalStHdx::<Test>::get(), 100 * ONE);

		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 100 * ONE);
	});
}

#[test]
fn stake_below_min_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE / 2),
			Error::<Test>::BelowMinStake
		);
	});
}

#[test]
fn stake_above_free_balance_fails() {
	ExtBuilder::default().build().execute_with(|| {
		// Alice has 1_000 * ONE
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 10_000 * ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn stake_increases_lock_not_replaces() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 150 * ONE);
		assert_eq!(s.st_minted, 150 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 150 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 150 * ONE);
		assert_eq!(TotalStHdx::<Test>::get(), 150 * ONE);
	});
}

#[test]
fn stake_zero_total_uses_one_to_one_rate() {
	ExtBuilder::default().build().execute_with(|| {
		// Empty pot, no prior stakers -> 1:1
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().st_minted, 100 * ONE);
	});
}

#[test]
fn stake_with_pot_uses_correct_rate() {
	// Pre-fund pot with 30 HDX, Alice already staked 100, then Bob stakes 100.
	ExtBuilder::default()
		.with_pot_balance(30 * ONE)
		.build()
		.execute_with(|| {
			// Alice's stake at bootstrap (pot exists but no stHDX yet -> bootstrap 1:1).
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			assert_eq!(Stakes::<Test>::get(ALICE).unwrap().st_minted, 100 * ONE);

			// Now S=100, T = TotalLocked(100) + pot(30) = 130. Bob's 100 HDX -> 100*100/130 = 76.
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(BOB).into(), 100 * ONE));
			let bob_st = Stakes::<Test>::get(BOB).unwrap().st_minted;
			// floor(100e12 * 100e12 / 130e12) = 76923076923076 (~76.92 stHDX)
			assert_eq!(bob_st, 76_923_076_923_076);
		});
}

#[test]
fn stake_stores_returned_atoken_not_input() {
	ExtBuilder::default().build().execute_with(|| {
		// Configure MM to round: returns 90% of input.
		TestMoneyMarket::set_supply_rounding(9, 10);
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx_locked, 100 * ONE); // input
		assert_eq!(s.st_minted, 90 * ONE); // returned by MM, not input
		assert_eq!(TotalStHdx::<Test>::get(), 90 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 90 * ONE);
	});
}

#[test]
fn stake_cannot_use_funds_already_locked_under_cooldown() {
	// Alice has 1000 HDX. She stakes 1000 and then unstakes 1000 → cooldown lock = 1000.
	// The free balance is still 1000 (locks don't subtract), but staking 1 more
	// must be rejected because that 1 would have to be drawn from cooldown HDX.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));
		assert_eq!(Balances::free_balance(ALICE), 1_000 * ONE);

		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn stake_cannot_extend_lock_past_actual_balance() {
	// Alice has 1000 HDX. After staking 1000, an existing-lock-aware check must
	// reject another 1 — there is no unlocked HDX left to back it.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn stake_after_unlock_succeeds() {
	// After the cooldown elapses and the user unlocks, the lock is gone
	// and their balance is fully available for a new stake.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));

		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into()));

		// Lock is gone, no active stake — fresh 500 HDX stake works.
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 500 * ONE));
	});
}

#[test]
fn stake_partial_remaining_balance_works_with_active_cooldown() {
	// Alice stakes 100, unstakes 100 (cooldown = 100). She still has 900 free for staking.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		// 1000 - cooldown(100) - prev_stake(0) = 900 stakeable.
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 900 * ONE));
		// One more HDX is impossible.
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn stake_mm_supply_failure_reverts_no_storage_mutation() {
	ExtBuilder::default().build().execute_with(|| {
		TestMoneyMarket::fail_supply();
		let pre_free = Balances::free_balance(ALICE);
		let pre_sthdx = Tokens::balance(ST_HDX, &ALICE);

		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::MoneyMarketSupplyFailed
		);

		// No pallet-gigahdx state mutation.
		assert!(Stakes::<Test>::get(ALICE).is_none());
		assert_eq!(TotalLocked::<Test>::get(), 0);
		assert_eq!(TotalStHdx::<Test>::get(), 0);
		assert_eq!(locked_under_ghdx(ALICE), 0);
		// stHDX rolled back by with_transaction.
		assert_eq!(Tokens::balance(ST_HDX, &ALICE), pre_sthdx);
		assert_eq!(Balances::free_balance(ALICE), pre_free);
		// MM was never credited (it errored).
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 0);
	});
}
