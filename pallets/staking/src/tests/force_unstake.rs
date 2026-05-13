use super::*;
use crate::types::{Conviction, Vote};
use frame_support::StorageDoubleMap;
use mock::Staking;
use pretty_assertions::assert_eq;

// In the unstake tests with the BOB position staked at block 1_452_987 and an
// unstake at 1_470_000 (within `UnclaimablePeriods`), the regular `unstake`
// pays 0 and slashes the full `10_671_709_925_655_406` back to the pot. For
// the same scenario `force_unstake` must pay that exact total to the user.
const HAPPY_PATH_TOTAL_REWARDS: u128 = 10_671_709_925_655_406_u128;
// For the same scenario at block 1_700_000 (past `UnclaimablePeriods`), the
// sigmoid `PayablePercentage` is only ~3.14% so a regular `unstake` would pay
// `334_912_244_857_841` and slash `10_336_797_680_797_565`. The two sum to
// the same total `force_unstake` must pay in full.
const SIGMOID_PATH_PAID_REWARDS: u128 = 334_912_244_857_841_u128;
const SIGMOID_PATH_SLASHED_REWARDS: u128 = 10_336_797_680_797_565_u128;

fn default_accounts() -> Vec<(u64, u32, Balance)> {
	vec![
		(ALICE, HDX, 150_000 * ONE),
		(BOB, HDX, 250_000 * ONE),
		(CHARLIE, HDX, 10_000 * ONE),
		(DAVE, HDX, 100_000 * ONE),
	]
}

fn default_stakes() -> Vec<(u64, Balance, u64, Balance)> {
	vec![
		(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
		(BOB, 120_000 * ONE, 1_452_987, 0),
		(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
		(DAVE, 10 * ONE, 1_465_000, 1),
	]
}

#[test]
fn force_unstake_should_pay_full_rewards_when_within_unclaimable_period() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(default_stakes())
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_470_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			let unlocked = Staking::force_unstake(&BOB).unwrap();

			//Assert
			assert_eq!(unlocked, 120_000 * ONE + HAPPY_PATH_TOTAL_REWARDS);
			assert_eq!(
				Tokens::free_balance(HDX, &BOB),
				250_000 * ONE + HAPPY_PATH_TOTAL_REWARDS
			);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);
		});
}

#[test]
fn force_unstake_should_pay_full_rewards_when_sigmoid_would_slash() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(default_stakes())
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();
			let expected_total = SIGMOID_PATH_PAID_REWARDS + SIGMOID_PATH_SLASHED_REWARDS;

			//Act
			let unlocked = Staking::force_unstake(&BOB).unwrap();

			//Assert: migration path ignores the sigmoid and pays the full total.
			assert_eq!(unlocked, 120_000 * ONE + expected_total);
			assert_eq!(Tokens::free_balance(HDX, &BOB), 250_000 * ONE + expected_total);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);
		});
}

#[test]
fn force_unstake_should_update_total_stake_and_pot_reserved() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(default_stakes())
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_470_000);
			let total_stake_before = Staking::staking().total_stake;

			//Act
			let unlocked = Staking::force_unstake(&BOB).unwrap();

			//Assert
			let staking = Staking::staking();
			assert_eq!(staking.total_stake, total_stake_before - 120_000 * ONE);

			// The user took all rewards out of the pot — the excess of pot balance
			// over the reserved balance must therefore be exactly zero.
			let pot_balance = Tokens::free_balance(HDX, &Staking::pot_account_id());
			assert_eq!(pot_balance, staking.pot_reserved_balance);

			// The returned unlocked total still matches stake + paid_rewards
			// (no locked_rewards for a fresh position).
			assert_eq!(unlocked, 120_000 * ONE + HAPPY_PATH_TOTAL_REWARDS);
		});
}

#[test]
fn force_unstake_should_emit_force_unstaked_event() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(default_stakes())
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_470_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::force_unstake(&BOB));

			//Assert
			assert_last_event!(Event::<Test>::ForceUnstaked {
				who: BOB,
				position_id: bob_position_id,
				stake: 120_000 * ONE,
				locked_rewards: 0,
				paid_rewards: HAPPY_PATH_TOTAL_REWARDS,
			}
			.into());
		});
}

#[test]
fn force_unstake_should_clear_votes_and_votes_rewarded() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(vec![(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE)])
		.with_votings(vec![(
			0,
			// DummyReferendumStatus: even indices are finished.
			vec![
				(
					2_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::Locked4x,
					},
				),
				(
					4_u32,
					Vote {
						amount: 5_000 * ONE,
						conviction: Conviction::Locked2x,
					},
				),
			],
		)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_470_000);
			let alice_position_id = 0;
			VotesRewarded::<Test>::insert(
				ALICE,
				100_u32,
				Vote {
					amount: 1_000 * ONE,
					conviction: Conviction::Locked1x,
				},
			);
			VotesRewarded::<Test>::insert(
				ALICE,
				200_u32,
				Vote {
					amount: 2_000 * ONE,
					conviction: Conviction::Locked2x,
				},
			);
			assert!(crate::Votes::<Test>::contains_key(alice_position_id));
			assert!(VotesRewarded::<Test>::contains_prefix(ALICE));

			//Act
			assert_ok!(Staking::force_unstake(&ALICE));

			//Assert
			assert!(Votes::<Test>::get(alice_position_id).votes.is_empty());
			assert!(!VotesRewarded::<Test>::contains_prefix(ALICE));
		});
}

#[test]
fn force_unstake_should_fail_when_active_ongoing_vote_present() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(vec![(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE)])
		.with_votings(vec![(
			0,
			// Odd index = ongoing per DummyReferendumStatus.
			vec![(
				1_u32,
				Vote {
					amount: 10_000 * ONE,
					conviction: Conviction::Locked4x,
				},
			)],
		)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_470_000);
			let alice_position_id = 0;
			let total_stake_before = Staking::staking().total_stake;

			//Act & assert
			assert_noop!(Staking::force_unstake(&ALICE), Error::<Test>::ActiveVotesOngoing);

			// Storage must be untouched — atomic rollback.
			assert!(Staking::positions(alice_position_id).is_some());
			assert!(Staking::get_user_position_id(&ALICE).unwrap().is_some());
			assert_eq!(Staking::staking().total_stake, total_stake_before);
		});
}

#[test]
fn force_unstake_should_fail_when_mixed_ongoing_and_finished_votes() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(vec![(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE)])
		.with_votings(vec![(
			0,
			vec![
				(
					2_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::Locked4x,
					},
				),
				// At least one ongoing entry blocks the whole call.
				(
					3_u32,
					Vote {
						amount: 5_000 * ONE,
						conviction: Conviction::Locked1x,
					},
				),
			],
		)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_470_000);
			let alice_position_id = 0;

			//Act & assert
			assert_noop!(Staking::force_unstake(&ALICE), Error::<Test>::ActiveVotesOngoing);
			assert!(Staking::positions(alice_position_id).is_some());
		});
}

#[test]
fn force_unstake_should_succeed_when_only_finished_votes_present() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(vec![(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE)])
		.with_votings(vec![(
			0,
			vec![(
				2_u32,
				Vote {
					amount: 10_000 * ONE,
					conviction: Conviction::Locked4x,
				},
			)],
		)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_470_000);
			let alice_position_id = 0;

			//Act
			assert_ok!(Staking::force_unstake(&ALICE));

			//Assert
			assert!(Staking::positions(alice_position_id).is_none());
			assert!(Votes::<Test>::get(alice_position_id).votes.is_empty());
		});
}

#[test]
fn force_unstake_should_fail_when_no_position() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_470_000);

			//Act & assert
			assert_noop!(
				Staking::force_unstake(&BOB),
				Error::<Test>::InconsistentState(InconsistentStateError::PositionNotFound)
			);
		});
}

#[test]
fn force_unstake_should_fail_when_staking_not_initialized() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_470_000);

			//Act & assert
			assert_noop!(Staking::force_unstake(&BOB), Error::<Test>::NotInitialized);
		});
}

#[test]
fn force_unstake_should_return_paid_unpaid_rewards_when_only_locked_after_increase_stake() {
	ExtBuilder::default()
		.with_endowed_accounts(default_accounts())
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.with_stakes(default_stakes())
		.build()
		.execute_with(|| {
			//Arrange — push BOB past unclaimable, then immediately increase_stake to
			// "fold" accumulated rewards into the position. After that, `new_rewards`
			// is zero and any payout must come from `accumulated_unpaid_rewards`.
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				10 * ONE
			));

			let position = Staking::positions(bob_position_id).unwrap();
			let unpaid_before = position.accumulated_unpaid_rewards;
			let locked_before = position.accumulated_locked_rewards;
			let bob_balance_before = Tokens::free_balance(HDX, &BOB);

			//Act
			let unlocked = Staking::force_unstake(&BOB).unwrap();

			//Assert: no new pending rewards introduced since the increase_stake, so the
			// only payable amount is whatever was carried in `accumulated_unpaid_rewards`.
			assert_eq!(Tokens::free_balance(HDX, &BOB) - bob_balance_before, unpaid_before);
			assert_eq!(unlocked, position.stake + locked_before + unpaid_before);
			assert!(Staking::positions(bob_position_id).is_none());
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
		});
}
