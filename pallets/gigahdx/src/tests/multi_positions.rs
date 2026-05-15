// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, PendingUnstakes, Stakes, TotalLocked};
use frame_support::sp_runtime::traits::AccountIdConversion;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use primitives::Balance;

fn pot() -> AccountId {
	GigaHdxPalletId::get().into_account_truncating()
}

fn locked_under_ghdx(account: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

fn ids_of(who: AccountId) -> Vec<u64> {
	let mut v: Vec<u64> = PendingUnstakes::<Test>::iter_prefix(who).map(|(id, _)| id).collect();
	v.sort();
	v
}

fn pending_sum(who: AccountId) -> Balance {
	PendingUnstakes::<Test>::iter_prefix(who).map(|(_, p)| p.amount).sum()
}

fn next_block() {
	System::set_block_number(System::block_number() + 1);
}

// ---------------------------------------------------------------------------
// Same-block compounding
// ---------------------------------------------------------------------------

#[test]
fn giga_unstake_should_compound_into_one_position_when_called_twice_in_same_block() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let block = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 20 * ONE));

		assert_eq!(ids_of(ALICE), vec![block]);
		assert_eq!(pending_count(ALICE), 1);
		assert_eq!(PendingUnstakes::<Test>::get(ALICE, block).unwrap().amount, 50 * ONE);
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().unstaking, 50 * ONE);
	});
}

#[test]
fn giga_unstake_should_compound_correctly_when_rate_changes_between_same_block_calls() {
	// pot=50, stake=100 → rate=1.5. First unstake 40g → payout 60 (principal).
	// Second unstake 60g → payout 90 with 50 yield (active drained).
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let pot_before = Balances::free_balance(pot());

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 60 * ONE));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 0);
			assert_eq!(s.gigahdx, 0);
			assert_eq!(s.unstaking, 150 * ONE);
			assert_eq!(s.unstaking_count, 1);
			assert_eq!(Balances::free_balance(pot()), pot_before - 50 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), 150 * ONE);
		});
}

#[test]
fn same_block_compounding_should_not_bump_unstaking_count() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		for _ in 0..5 {
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 5 * ONE));
		}
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().unstaking_count, 1);
	});
}

#[test]
fn cancel_should_handle_compounded_position_with_yield_correctly() {
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let block = System::block_number();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 60 * ONE));
			assert_eq!(PendingUnstakes::<Test>::get(ALICE, block).unwrap().amount, 150 * ONE);
			assert_eq!(Balances::free_balance(pot()), 0);

			assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), block));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 150 * ONE);
			assert_eq!(s.gigahdx, 150 * ONE);
			assert_eq!(s.unstaking, 0);
			assert_eq!(s.unstaking_count, 0);
			assert_eq!(TotalLocked::<Test>::get(), 150 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), 150 * ONE);
		});
}

#[test]
fn cancel_compounded_position_should_preserve_system_total() {
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let baseline = system_total();
			let block = System::block_number();

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 60 * ONE));
			assert_eq!(system_total(), baseline);

			assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), block));
			assert_eq!(system_total(), baseline);
		});
}

#[test]
fn cancel_compounded_should_decrement_count_by_one() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE));
		next_block();
		let b1 = System::block_number();
		for _ in 0..3 {
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 10 * ONE));
		}
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().unstaking_count, 2);

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), b1));

		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().unstaking_count, 1);
		assert_eq!(ids_of(ALICE), vec![b0]);
	});
}

#[test]
fn unlock_compounded_should_release_full_compounded_amount() {
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let block = System::block_number();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 60 * ONE));
			let pre_free = Balances::free_balance(ALICE);
			assert_eq!(locked_under_ghdx(ALICE), 150 * ONE);

			System::set_block_number(block + GigaHdxCooldownPeriod::get());
			assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), block));

			assert_eq!(pending_count(ALICE), 0);
			assert_eq!(Balances::free_balance(ALICE), pre_free);
			assert_eq!(locked_under_ghdx(ALICE), 0);
		});
}

#[test]
fn same_block_compounding_should_not_trigger_admission_cap() {
	ExtBuilder::default().build().execute_with(|| {
		let max = GigaHdxMaxPendingUnstakes::get();
		assert_ok!(GigaHdx::giga_stake(
			RawOrigin::Signed(ALICE).into(),
			(max as Balance) * 10 * ONE,
		));
		for _ in 0..max + 2 {
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), ONE));
		}
		assert_eq!(pending_count(ALICE), 1);
	});
}

// ---------------------------------------------------------------------------
// Admission cap across distinct blocks
// ---------------------------------------------------------------------------

#[test]
fn giga_unstake_should_create_distinct_positions_across_blocks() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 300 * ONE));
		let mut expected_ids = vec![];
		for _ in 0..3 {
			expected_ids.push(System::block_number());
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
			next_block();
		}
		assert_eq!(ids_of(ALICE), expected_ids);
		assert_eq!(pending_count(ALICE), 3);
	});
}

#[test]
fn giga_unstake_should_fail_when_max_pending_positions_reached_across_blocks() {
	ExtBuilder::default().build().execute_with(|| {
		let max = GigaHdxMaxPendingUnstakes::get();
		assert_ok!(GigaHdx::giga_stake(
			RawOrigin::Signed(ALICE).into(),
			(max as Balance) * 10 * ONE,
		));
		for _ in 0..max {
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 5 * ONE));
			next_block();
		}
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 5 * ONE),
			Error::<Test>::TooManyPendingUnstakes,
		);
	});
}

#[test]
fn giga_unstake_should_succeed_after_unlock_freed_a_slot() {
	ExtBuilder::default().build().execute_with(|| {
		let max = GigaHdxMaxPendingUnstakes::get();
		assert_ok!(GigaHdx::giga_stake(
			RawOrigin::Signed(ALICE).into(),
			(max as Balance) * 10 * ONE,
		));
		let first_id = System::block_number();
		for _ in 0..max {
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 5 * ONE));
			next_block();
		}
		System::set_block_number(first_id + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), first_id));

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 5 * ONE));
		assert_eq!(pending_count(ALICE) as u32, max);
	});
}

// ---------------------------------------------------------------------------
// unlock(position_id) — block-keyed
// ---------------------------------------------------------------------------

#[test]
fn unlock_should_release_only_targeted_position_when_called() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 300 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		next_block();
		let b1 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		next_block();
		let b2 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

		System::set_block_number(b1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b1));

		assert_eq!(ids_of(ALICE), vec![b0, b2]);
		assert_eq!(locked_under_ghdx(ALICE), 260 * ONE);
	});
}

#[test]
fn unlock_should_fail_when_position_id_not_found() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		System::set_block_number(1 + GigaHdxCooldownPeriod::get());
		assert_noop!(
			GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 999_999),
			Error::<Test>::PendingUnstakeNotFound,
		);
	});
}

#[test]
fn unlock_should_fail_when_target_cooldown_not_elapsed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		System::set_block_number(b0 + GigaHdxCooldownPeriod::get() / 2);
		let b1 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		System::set_block_number(b0 + GigaHdxCooldownPeriod::get());

		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b0));
		assert_noop!(
			GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b1),
			Error::<Test>::CooldownNotElapsed,
		);
	});
}

#[test]
fn unlock_should_clean_up_stake_record_when_all_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		System::set_block_number(b0 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b0));

		assert!(Stakes::<Test>::get(ALICE).is_none());
		assert_eq!(pending_count(ALICE), 0);
		assert_eq!(locked_under_ghdx(ALICE), 0);
	});
}

#[test]
fn unlock_should_keep_stake_record_when_other_positions_remain() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 60 * ONE));
		System::set_block_number(b0 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b0));

		assert!(Stakes::<Test>::get(ALICE).is_some());
		assert_eq!(pending_count(ALICE), 1);
	});
}

// ---------------------------------------------------------------------------
// cancel_unstake — block-keyed
// ---------------------------------------------------------------------------

#[test]
fn cancel_unstake_should_leave_other_positions_intact() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 300 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		next_block();
		let b1 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		next_block();
		let b2 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), b1));

		assert_eq!(ids_of(ALICE), vec![b0, b2]);
		assert_eq!(PendingUnstakes::<Test>::get(ALICE, b0).unwrap().amount, 30 * ONE);
		assert_eq!(PendingUnstakes::<Test>::get(ALICE, b2).unwrap().amount, 50 * ONE);
	});
}

#[test]
fn cancel_unstake_should_fail_when_position_id_not_found() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		assert_noop!(
			GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 999_999),
			Error::<Test>::PendingUnstakeNotFound,
		);
	});
}

// ---------------------------------------------------------------------------
// Cached accounting invariants
// ---------------------------------------------------------------------------

#[test]
fn cached_unstaking_should_match_sum_of_positions() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 300 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 75 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 25 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.unstaking, pending_sum(ALICE));
		assert_eq!(s.unstaking, 150 * ONE);
		assert_eq!(s.unstaking_count, 3);
	});
}

#[test]
fn cached_unstaking_should_decrease_when_position_unlocked() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		System::set_block_number(b0 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b0));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.unstaking, 30 * ONE);
		assert_eq!(s.unstaking_count, 1);
		assert_eq!(s.unstaking, pending_sum(ALICE));
	});
}

#[test]
fn cached_unstaking_should_decrease_when_position_cancelled() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), b0));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.unstaking, 30 * ONE);
		assert_eq!(s.unstaking_count, 1);
		assert_eq!(s.unstaking, pending_sum(ALICE));
	});
}

// ---------------------------------------------------------------------------
// Lock invariants
// ---------------------------------------------------------------------------

#[test]
fn lock_should_equal_active_plus_sum_of_pending_when_multiple_positions() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 300 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 75 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 25 * ONE));

		let active = Stakes::<Test>::get(ALICE).unwrap().hdx;
		assert_eq!(locked_under_ghdx(ALICE), active + pending_sum(ALICE));
		assert_eq!(active, 150 * ONE);
		assert_eq!(pending_sum(ALICE), 150 * ONE);
	});
}

#[test]
fn lock_should_decrease_by_unlocked_amount_when_one_unlocks() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		let pre_lock = locked_under_ghdx(ALICE);

		System::set_block_number(b0 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b0));

		assert_eq!(locked_under_ghdx(ALICE), pre_lock - 50 * ONE);
	});
}

#[test]
fn lock_should_remain_when_one_position_is_cancelled() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 200 * ONE));
		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		next_block();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
		let pre_lock = locked_under_ghdx(ALICE);

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), b0));

		assert_eq!(locked_under_ghdx(ALICE), pre_lock);
	});
}

fn system_total() -> Balance {
	let pending_sum: Balance = PendingUnstakes::<Test>::iter().map(|(_, _, p)| p.amount).sum();
	TotalLocked::<Test>::get() + pending_sum + Balances::free_balance(pot())
}

#[test]
fn multiple_pending_positions_should_conserve_system_total() {
	ExtBuilder::default()
		.with_pot_balance(60 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let baseline = system_total();

			let b0 = System::block_number();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			assert_eq!(system_total(), baseline);

			assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), b0));
			assert_eq!(system_total(), baseline);

			next_block();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
			next_block();
			let b_for_30 = System::block_number();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 30 * ONE));
			assert_eq!(system_total(), baseline);

			System::set_block_number(b_for_30 + GigaHdxCooldownPeriod::get());
			assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b_for_30));
			assert_eq!(system_total(), baseline - 30 * ONE);
		});
}

#[test]
fn frozen_should_remain_invariant_across_multi_position_operations() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 300 * ONE));
		GigaHdx::freeze(&ALICE, 50 * ONE);

		let b0 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		next_block();
		let b1 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), b0));
		System::set_block_number(b1 + GigaHdxCooldownPeriod::get());
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), b1));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.frozen, 50 * ONE);
		assert!(s.frozen <= s.hdx);
	});
}
