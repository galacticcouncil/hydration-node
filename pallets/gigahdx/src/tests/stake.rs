// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes, TotalLocked};
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
fn giga_stake_should_record_correct_state_when_called() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 100 * ONE);

		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 100 * ONE);
	});
}

#[test]
fn giga_stake_should_fail_when_amount_below_min() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE / 2),
			Error::<Test>::BelowMinStake
		);
	});
}

#[test]
fn giga_stake_should_fail_when_amount_above_free_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 10_000 * ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn giga_stake_should_increase_lock_when_already_staked() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 150 * ONE);
		assert_eq!(s.gigahdx, 150 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 150 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 150 * ONE);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 150 * ONE);
	});
}

#[test]
fn giga_stake_should_use_one_to_one_rate_when_supply_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, 100 * ONE);
	});
}

#[test]
fn giga_stake_should_use_correct_rate_when_pot_funded() {
	ExtBuilder::default()
		.with_pot_balance(30 * ONE)
		.build()
		.execute_with(|| {
			// Pot exists but no stHDX yet → bootstrap 1:1.
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, 100 * ONE);

			// S=100, T = TotalLocked(100) + pot(30) = 130 → Bob: floor(100e12 * 100e12 / 130e12).
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(BOB).into(), 100 * ONE));
			let bob_st = Stakes::<Test>::get(BOB).unwrap().gigahdx;
			assert_eq!(bob_st, 76_923_076_923_076);
		});
}

#[test]
fn giga_stake_should_store_returned_atoken_when_mm_rounds() {
	ExtBuilder::default().build().execute_with(|| {
		// MM returns 90% of input; `Stakes.gigahdx` must reflect the
		// returned amount, not the input. stHDX issuance reflects the input.
		TestMoneyMarket::set_supply_rounding(9, 10);
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 90 * ONE);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 100 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 90 * ONE);
	});
}

#[test]
fn giga_stake_should_fail_when_funds_locked_under_cooldown() {
	// Free balance is still 1000 after stake+unstake (locks don't subtract),
	// but staking another 1 must be rejected — it would draw from cooldown HDX.
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
fn giga_stake_should_fail_when_extending_lock_past_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn giga_stake_should_succeed_when_called_after_unlock() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 1_000 * ONE));

		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1));

		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 500 * ONE));
	});
}

#[test]
fn giga_stake_should_use_unlocked_balance_when_cooldown_active() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		// 1000 - cooldown(100) - prev_stake(0) = 900 stakeable.
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 900 * ONE));
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn giga_stake_should_revert_storage_when_mm_supply_fails() {
	ExtBuilder::default().build().execute_with(|| {
		TestMoneyMarket::fail_supply();
		let pre_free = Balances::free_balance(ALICE);
		let pre_sthdx = Tokens::balance(ST_HDX, &ALICE);

		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::MoneyMarketSupplyFailed
		);

		assert!(Stakes::<Test>::get(ALICE).is_none());
		assert_eq!(TotalLocked::<Test>::get(), 0);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 0);
		assert_eq!(locked_under_ghdx(ALICE), 0);
		// stHDX mint rolled back by with_transaction.
		assert_eq!(Tokens::balance(ST_HDX, &ALICE), pre_sthdx);
		assert_eq!(Balances::free_balance(ALICE), pre_free);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 0);
	});
}

#[test]
fn giga_stake_should_subtract_own_existing_stake() {
	// Alice has 1000 ONE total. Two 500 stakes max out her balance; a third
	// stake of 1 must fail because own_claim now equals her whole balance.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 500 * ONE));
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 500 * ONE));
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::InsufficientFreeBalance,
		);
	});
}

#[test]
fn giga_stake_should_fail_when_external_claims_nonzero() {
	// Strict policy: any non-zero external claim blocks admission,
	// regardless of how much free balance the caller has.
	ExtBuilder::default().build().execute_with(|| {
		TestExternalClaims::set(ONE);
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::BlockedByExternalLock,
		);
	});
}

#[test]
fn giga_stake_should_fail_when_external_claim_appears_after_stake() {
	// Existing staker who later acquires another lock (e.g. legacy
	// staking) can't grow their gigahdx position.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 400 * ONE));
		TestExternalClaims::set(ONE);
		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), ONE),
			Error::<Test>::BlockedByExternalLock,
		);
	});
}

#[test]
fn giga_stake_should_treat_unstaking_as_own_claim() {
	// After a full unstake, stake.hdx → 0 and stake.unstaking holds the pending
	// amount. A fresh stake must still see that pending portion as committed.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 600 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 600 * ONE));
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 0);
		assert_eq!(s.unstaking, 600 * ONE);

		assert_noop!(
			GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 500 * ONE),
			Error::<Test>::InsufficientFreeBalance,
		);
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 400 * ONE));
	});
}
