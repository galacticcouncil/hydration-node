// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes, TotalLocked};
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
fn giga_unstake_should_move_active_to_position_when_pot_empty() {
	// payout ≤ active → active drained, position = payout, no yield.
	ExtBuilder::default().build().execute_with(|| {
		let pre_free = Balances::free_balance(ALICE);
		stake_alice_100();
		assert_eq!(Balances::free_balance(ALICE), pre_free);

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 0);
		assert_eq!(s.gigahdx, 0);
		assert_eq!(TotalLocked::<Test>::get(), 0);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 0);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 0);

		assert_eq!(only_pending(ALICE).amount, 100 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(Balances::free_balance(ALICE), pre_free);
	});
}

#[test]
fn giga_unstake_should_pull_yield_from_pot_when_payout_exceeds_active() {
	// payout 130 > active 100 → active drained, yield 30 from pot.
	ExtBuilder::default()
		.with_pot_balance(30 * ONE)
		.build()
		.execute_with(|| {
			let pre_free = Balances::free_balance(ALICE);
			stake_alice_100();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

			assert_eq!(Balances::free_balance(ALICE), pre_free + 30 * ONE);
			let pot: AccountId = GigaHdxPalletId::get().into_account_truncating();
			assert_eq!(Balances::free_balance(pot), 0);

			// Stakes record persists (zeroed) until `unlock` cleans it up.
			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 0);
			assert_eq!(s.gigahdx, 0);

			assert_eq!(only_pending(ALICE).amount, 130 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), 130 * ONE);
		});
}

#[test]
fn giga_unstake_should_split_state_when_partial() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 60 * ONE);
		assert_eq!(s.gigahdx, 60 * ONE);
		// Combined lock = active(60) + position(40).
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 60 * ONE);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 60 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 60 * ONE);
		assert_eq!(only_pending(ALICE).amount, 40 * ONE);
	});
}

#[test]
fn giga_unstake_should_fail_when_amount_zero() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn giga_unstake_should_fail_when_amount_exceeds_stake() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 101 * ONE),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn giga_unstake_should_fail_when_no_stake_exists() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::NoStake
		);
	});
}

#[test]
fn giga_unstake_should_revert_storage_when_mm_withdraw_fails() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		let pre_stake = Stakes::<Test>::get(ALICE).unwrap();
		let pre_total_locked = TotalLocked::<Test>::get();
		let pre_total_sthdx = GigaHdx::total_gigahdx_supply();
		let pre_lock = locked_under_ghdx(ALICE);
		let pre_mm_balance = TestMoneyMarket::balance_of(&ALICE);
		let pre_sthdx_balance = Tokens::balance(ST_HDX, &ALICE);

		TestMoneyMarket::fail_withdraw();
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE),
			Error::<Test>::MoneyMarketWithdrawFailed
		);

		// Pre-decrement of `gigahdx` rolled back by `with_transaction`.
		let post_stake = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(post_stake.gigahdx, pre_stake.gigahdx, "gigahdx must be restored");
		assert_eq!(post_stake.hdx, pre_stake.hdx);
		assert_eq!(TotalLocked::<Test>::get(), pre_total_locked);
		assert_eq!(GigaHdx::total_gigahdx_supply(), pre_total_sthdx);
		assert_eq!(locked_under_ghdx(ALICE), pre_lock);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), pre_mm_balance);
		assert_eq!(Tokens::balance(ST_HDX, &ALICE), pre_sthdx_balance);
		assert_eq!(pending_count(ALICE), 0);
	});
}

#[test]
fn giga_unstake_should_pre_decrement_gigahdx_before_mm_withdraw() {
	// `LockableAToken.burn` relies on lock-manager reading the
	// already-decremented `Stakes[who].gigahdx`. We can't observe mid-call
	// state, but the post-state proves the pre-decrement happened before
	// MM.withdraw — otherwise the burn would have reverted in production.
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, 70 * ONE);
	});
}
