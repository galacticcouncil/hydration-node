use super::mock::*;
use crate::Error;
use frame_support::{assert_noop, assert_ok};

fn setup_unstake(who: AccountId, stake_amount: Balance, unstake_amount: Balance) {
	assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(who), stake_amount));
	assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(who), unstake_amount));
}

#[test]
fn unlock_should_release_hdx_when_cooldown_elapsed() {
	ExtBuilder::default().build().execute_with(|| {
		setup_unstake(ALICE, 100 * ONE, 100 * ONE);

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 1);
		assert_eq!(positions[0].unlock_at, 101); // block 1 + 100

		// Advance past cooldown
		run_to_block(101);

		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE));

		// Position removed
		assert_eq!(GigaHdx::unstake_positions(&ALICE).len(), 0);
	});
}

#[test]
fn unlock_should_fail_when_cooldown_not_elapsed() {
	ExtBuilder::default().build().execute_with(|| {
		setup_unstake(ALICE, 100 * ONE, 100 * ONE);

		// Still at block 1, cooldown expires at 101
		assert_noop!(
			GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE),
			Error::<Test>::NothingToUnlock
		);
	});
}

#[test]
fn unlock_should_succeed_when_called_by_third_party() {
	ExtBuilder::default().build().execute_with(|| {
		setup_unstake(ALICE, 100 * ONE, 100 * ONE);

		run_to_block(101);

		// BOB unlocks ALICE's positions
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE));

		assert_eq!(GigaHdx::unstake_positions(&ALICE).len(), 0);
	});
}

#[test]
fn unlock_should_fail_when_no_positions() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE has no unstake positions at all
		assert_noop!(
			GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE),
			Error::<Test>::NothingToUnlock
		);
	});
}

#[test]
fn unlock_should_release_only_expired_positions_when_some_pending() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Create two positions at different times
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));

		run_to_block(5);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 2);
		assert_eq!(positions[0].unlock_at, 101); // block 1 + 100
		assert_eq!(positions[1].unlock_at, 105); // block 5 + 100

		// Advance past first cooldown only
		run_to_block(101);

		// Unlocks only the first position
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE));
		assert_eq!(GigaHdx::unstake_positions(&ALICE).len(), 1);

		// Second position still in cooldown — nothing to unlock
		assert_noop!(
			GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE),
			Error::<Test>::NothingToUnlock
		);

		// Advance past second cooldown
		run_to_block(105);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE));
		assert_eq!(GigaHdx::unstake_positions(&ALICE).len(), 0);
	});
}

#[test]
fn unlock_should_release_all_when_all_expired() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Create two positions at the same block
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 20 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));

		assert_eq!(GigaHdx::unstake_positions(&ALICE).len(), 2);

		// Advance past both cooldowns
		run_to_block(101);

		// Single call unlocks both
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(BOB), ALICE));
		assert_eq!(GigaHdx::unstake_positions(&ALICE).len(), 0);
	});
}
