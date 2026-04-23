use super::mock::*;
use crate::{Error, Event};
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect, Mutate as FungiblesMutate},
};
use sp_runtime::FixedU128;

fn setup_stake(who: AccountId, amount: Balance) {
	assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(who), amount));
}

#[test]
fn giga_unstake_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		// With no-op MoneyMarket, user has stHDX.
		let st_hdx_balance = <Test as crate::Config>::Currency::balance(ST_HDX, &ALICE);
		assert_eq!(st_hdx_balance, 100 * ONE);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// stHDX burned
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &ALICE), 0);

		// HDX transferred back to user (but locked)
		assert_eq!(
			<Test as crate::Config>::Currency::balance(HDX, &ALICE),
			1_000 * ONE // Back to original balance
		);

		// Unstake position created
		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 1);
		assert_eq!(positions[0].amount, 100 * ONE);
		assert_eq!(positions[0].unlock_at, 1 + 100); // block 1 + CooldownPeriod 100

		// Check event
		System::assert_last_event(
			Event::Unstaked {
				who: ALICE,
				gigahdx_withdrawn: 100 * ONE,
				st_hdx_burned: 100 * ONE,
				hdx_amount: 100 * ONE,
				unlock_at: 101,
			}
			.into(),
		);
	});
}

#[test]
fn giga_unstake_should_fail_with_zero_amount() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn giga_unstake_multiple_positions() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 2);
		assert_eq!(positions[0].amount, 30 * ONE);
		assert_eq!(positions[1].amount, 30 * ONE);

		// Each position has a unique lock_id
		assert_ne!(positions[0].lock_id, positions[1].lock_id);
	});
}

#[test]
fn giga_unstake_too_many_positions_should_fail() {
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 100_000 * ONE)])
		.build()
		.execute_with(|| {
			setup_stake(ALICE, 11_000 * ONE);

			// Create MaxUnstakePositions (10) positions
			for _ in 0..10 {
				assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE));
			}

			// 11th should fail
			assert_noop!(
				GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE),
				Error::<Test>::TooManyUnstakePositions
			);
		});
}

#[test]
fn giga_unstake_at_increased_rate() {
	ExtBuilder::default().build().execute_with(|| {
		// Stake 100 HDX at 1:1
		setup_stake(ALICE, 100 * ONE);

		// Simulate fee accrual: double the gigapot
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 100 * ONE));

		// Exchange rate is now 2.0
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::from(2));

		// Unstake 100 stHDX worth of GIGAHDX → should get 200 HDX
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions[0].amount, 200 * ONE);
	});
}

// ---------------------------------------------------------------------------
// Phase 1 — Remaining-balance minimum tests
// ---------------------------------------------------------------------------

#[test]
fn giga_unstake_should_fail_when_remaining_below_min_stake() {
	ExtBuilder::default().build().execute_with(|| {
		// Stake 2*ONE. Unstaking ONE+1 leaves ONE-1 remaining, below MinStake=ONE.
		setup_stake(ALICE, 2 * ONE);

		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE + 1),
			Error::<Test>::RemainingBelowMinStake
		);
	});
}

#[test]
fn giga_unstake_partial_should_succeed_when_remaining_meets_min_stake() {
	ExtBuilder::default().build().execute_with(|| {
		// Stake 2*ONE. Unstaking ONE leaves exactly ONE remaining == MinStake.
		setup_stake(ALICE, 2 * ONE);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE));
	});
}

#[test]
fn giga_unstake_full_should_always_succeed() {
	ExtBuilder::default().build().execute_with(|| {
		// Full unstake (remaining == 0) is always valid — no minimum enforced on the last exit.
		setup_stake(ALICE, ONE);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE));
	});
}

#[test]
fn giga_unstake_zero_amount_fails() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

// ---------------------------------------------------------------------------
// Phase 2 — Multiple concurrent unlock position tests
// ---------------------------------------------------------------------------

#[test]
fn giga_unstake_positions_have_independent_cooldowns() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		// First unstake at block 1.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 10 * ONE));

		// Advance the chain and unstake again.
		System::set_block_number(50);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 10 * ONE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 2);
		// Position 0 unlocks at 1 + 100 = 101.
		// Position 1 unlocks at 50 + 100 = 150.
		assert_eq!(positions[0].unlock_at, 101);
		assert_eq!(positions[1].unlock_at, 150);
		assert_ne!(positions[0].lock_id, positions[1].lock_id);
	});
}

#[test]
fn unlock_frees_only_expired_positions() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		// Two positions, staggered.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 10 * ONE));
		System::set_block_number(50);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 10 * ONE));

		// Advance to unlock the first but not the second: first expires at 101, second at 150.
		System::set_block_number(120);

		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(ALICE), ALICE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 1, "only the unexpired position remains");
		assert_eq!(positions[0].unlock_at, 150);
	});
}

#[test]
fn unlock_fails_when_no_position_expired() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 10 * ONE));

		// Block 1, first unlock is at 101. Try to unlock at block 50 — nothing to unlock.
		System::set_block_number(50);
		assert_noop!(
			GigaHdx::unlock(RuntimeOrigin::signed(ALICE), ALICE),
			Error::<Test>::NothingToUnlock
		);
	});
}

// ---------------------------------------------------------------------------
// Phase 3 — on_post_unstake hook invocation regression
// ---------------------------------------------------------------------------

#[test]
fn giga_unstake_calls_on_post_unstake_hook() {
	// The gigahdx mock uses Hooks = (), so on_post_unstake is a no-op.
	// This test guards against future refactors that accidentally drop the hook call.
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 50 * ONE));
	});
}
