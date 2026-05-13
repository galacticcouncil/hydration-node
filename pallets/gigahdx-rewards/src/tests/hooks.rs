// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::pallet::{
	Event, PendingRewards, ReferendaRewardPool, ReferendaTotalWeightedVotes, ReferendumTracks, UserVoteRecords,
};
use crate::types::REWARD_MULTIPLIER_SCALE;
use crate::voting_hooks::VotingHooksImpl;

use frame_support::assert_ok;
use frame_system::RawOrigin;
use pallet_conviction_voting::{AccountVote, Conviction, Status, Vote, VotingHooks};

const REF_A: u32 = 7;
const REF_B: u32 = 9;

fn standard_vote(aye: bool, conviction: Conviction, balance: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye, conviction },
		balance,
	}
}

fn stake(who: AccountId, amount: u128) {
	assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(who).into(), amount));
}

#[test]
fn on_before_vote_should_skip_when_user_has_no_stake() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE has not staked → no record should be created.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 100 * ONE),
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_none());
		assert!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).is_none());
		assert!(ReferendumTracks::<Test>::get(REF_A).is_none());
	});
}

#[test]
fn on_before_vote_should_skip_when_vote_is_split() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			AccountVote::Split {
				aye: 40 * ONE,
				nay: 60 * ONE,
			},
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_none());
		assert!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).is_none());
	});
}

#[test]
fn on_before_vote_should_skip_when_vote_is_split_abstain() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			AccountVote::SplitAbstain {
				aye: 10 * ONE,
				nay: 10 * ONE,
				abstain: 80 * ONE,
			},
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_none());
		assert!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).is_none());
	});
}

#[test]
fn on_before_vote_should_clean_up_prior_record_when_downgrading_to_split() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);

		// First cast a tracked Standard vote — record, freeze, and tally
		// row all written.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked6x, 50 * ONE),
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_some());
		assert_eq!(stake_record(&ALICE).frozen, 50 * ONE);
		let tally = ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap();
		assert_eq!(tally.voters_count, 1);
		assert!(tally.total_weighted > 0);

		// Downgrade to Split — the prior record, the freeze, and the tally
		// row must all unwind so the user is not credited a reward share they
		// no longer hold.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			AccountVote::Split {
				aye: 20 * ONE,
				nay: 30 * ONE,
			},
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_none());
		assert_eq!(stake_record(&ALICE).frozen, 0);
		assert!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).is_none());
	});
}

#[test]
fn on_before_vote_should_clean_up_prior_record_when_downgrading_to_split_abstain() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked3x, 40 * ONE),
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_some());
		assert_eq!(stake_record(&ALICE).frozen, 40 * ONE);

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			AccountVote::SplitAbstain {
				aye: 10 * ONE,
				nay: 10 * ONE,
				abstain: 20 * ONE,
			},
		));
		assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_none());
		assert_eq!(stake_record(&ALICE).frozen, 0);
		assert!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).is_none());
	});
}

#[test]
fn on_before_vote_should_cap_weighted_at_min_of_vote_and_stake() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 50 * ONE);
		// vote.balance > stake.hdx ⇒ should be capped to 50 * ONE.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 1_000 * ONE),
		));
		let rec = UserVoteRecords::<Test>::get(ALICE, REF_A).unwrap();
		assert_eq!(rec.staked_vote_amount, 50 * ONE);
		// Locked1x = 10 / 10 = 1× → weighted = 50 * ONE
		assert_eq!(rec.weighted, 50 * ONE);
		assert_eq!(rec.weighted, 50 * ONE * 10 / REWARD_MULTIPLIER_SCALE);
	});
}

#[test]
fn on_before_vote_should_replace_record_when_vote_is_edited() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);

		// First vote: 30 HDX, 1x conviction → weighted = 30 * ONE.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 30 * ONE),
		));
		let tally_1 = ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap();
		assert_eq!(tally_1.voters_count, 1);
		assert_eq!(tally_1.total_weighted, 30 * ONE);

		// Edit: 80 HDX, 2x → weighted = 160 * ONE.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked2x, 80 * ONE),
		));
		let rec = UserVoteRecords::<Test>::get(ALICE, REF_A).unwrap();
		assert_eq!(rec.staked_vote_amount, 80 * ONE);
		assert_eq!(rec.weighted, 160 * ONE);

		let tally_2 = ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap();
		assert_eq!(tally_2.voters_count, 1, "voter count unchanged on edit");
		assert_eq!(tally_2.total_weighted, 160 * ONE, "total recomputed");
	});
}

#[test]
fn on_before_vote_should_increment_voter_count_only_for_new_records() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		stake(BOB, 100 * ONE);

		// First vote by alice — inc 1.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 10 * ONE),
		));
		assert_eq!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap().voters_count, 1);

		// Edit by alice — inc 0.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 20 * ONE),
		));
		assert_eq!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap().voters_count, 1);

		// New voter (bob) — inc 1.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&BOB,
			REF_A,
			standard_vote(false, Conviction::Locked1x, 15 * ONE),
		));
		assert_eq!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap().voters_count, 2);
	});
}

#[test]
fn on_before_vote_should_freeze_stake_for_voted_amount() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		assert_eq!(stake_record(&ALICE).frozen, 0);

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 60 * ONE),
		));
		assert_eq!(stake_record(&ALICE).frozen, 60 * ONE);

		// Edit lowers vote → frozen recomputed (unfreeze old, freeze new).
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 20 * ONE),
		));
		assert_eq!(stake_record(&ALICE).frozen, 20 * ONE);
	});
}

#[test]
fn on_before_vote_should_cache_track_id_when_first_voter_arrives() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		assert!(ReferendumTracks::<Test>::get(REF_A).is_none());
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 10 * ONE),
		));
		assert_eq!(ReferendumTracks::<Test>::get(REF_A), Some(0u16));
	});
}

#[test]
fn on_remove_vote_should_drop_record_and_skip_reward_when_status_is_ongoing() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 100 * ONE);
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 50 * ONE),
			));
			assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_some());

			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Ongoing);

			assert!(UserVoteRecords::<Test>::get(ALICE, REF_A).is_none());
			assert!(ReferendaRewardPool::<Test>::get(REF_A).is_none());
			assert_eq!(PendingRewards::<Test>::get(ALICE), 0);
		});
}

#[test]
fn on_remove_vote_should_unfreeze_stake() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 70 * ONE),
		));
		assert_eq!(stake_record(&ALICE).frozen, 70 * ONE);

		VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Ongoing);
		assert_eq!(stake_record(&ALICE).frozen, 0);
	});
}

#[test]
fn on_remove_vote_should_decrement_voters_count_pre_allocation() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		stake(BOB, 100 * ONE);

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 10 * ONE),
		));
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&BOB,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 20 * ONE),
		));
		assert_eq!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap().voters_count, 2);

		VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Ongoing);
		// One left.
		assert_eq!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap().voters_count, 1);

		VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Ongoing);
		// Entry pruned when count reaches zero.
		assert!(ReferendaTotalWeightedVotes::<Test>::get(REF_A).is_none());
	});
}

#[test]
fn on_remove_vote_should_allocate_pool_once_per_referendum() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 100 * ONE);
			stake(BOB, 100 * ONE);

			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 40 * ONE),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 60 * ONE),
			));

			let acc_before = account_balance(&accumulator_pot());
			let alloc_before = account_balance(&allocated_pot());

			// First Completed remove: transfer 10% of accumulator → allocated.
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			let pool = ReferendaRewardPool::<Test>::get(REF_A).expect("pool created");
			assert_eq!(pool.total_reward, 100 * ONE); // 10% of 1_000 * ONE
			assert_eq!(pool.track_id, 0u16);
			// Voter count snapshot: alice already deducted from live tally but
			// re-added when populating the pool → 2.
			assert_eq!(pool.voters_remaining, 1); // alice immediately decremented
			let acc_after_first = account_balance(&accumulator_pot());
			let alloc_after_first = account_balance(&allocated_pot());
			assert_eq!(acc_before - acc_after_first, 100 * ONE);
			assert_eq!(alloc_after_first - alloc_before, 100 * ONE);

			// Second Completed remove: no re-allocation; transfers should not happen.
			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Completed);
			let acc_after_second = account_balance(&accumulator_pot());
			let alloc_after_second = account_balance(&allocated_pot());
			assert_eq!(acc_after_first, acc_after_second, "no further accumulator drain");
			// Allocated pot only shrinks via claim_rewards, not on_remove_vote.
			assert_eq!(alloc_after_first, alloc_after_second);

			// Pool removed once last voter claimed.
			assert!(ReferendaRewardPool::<Test>::get(REF_A).is_none());
		});
}

#[test]
fn on_remove_vote_should_record_user_reward_pro_rata() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			// alice and bob with equal weighted; charlie 2x.
			stake(ALICE, 100 * ONE);
			stake(BOB, 100 * ONE);
			stake(CHARLIE, 100 * ONE);

			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&CHARLIE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 20 * ONE),
			));
			// Total weighted = 10 + 10 + 20 = 40 * ONE.

			// total_reward = 10% of 1_000 * ONE = 100 * ONE.
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			// alice share = floor(10 * 100 / 40) = 25 * ONE.
			assert_eq!(PendingRewards::<Test>::get(ALICE), 25 * ONE);

			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Completed);
			assert_eq!(PendingRewards::<Test>::get(BOB), 25 * ONE);

			// Charlie is the last → scoops remainder = 100 - 25 - 25 = 50 * ONE.
			VotingHooksImpl::<Test>::on_remove_vote(&CHARLIE, REF_A, Status::Completed);
			assert_eq!(PendingRewards::<Test>::get(CHARLIE), 50 * ONE);
		});
}

#[test]
fn on_remove_vote_should_accumulate_pending_rewards_across_referenda() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 200 * ONE);

			// ref A: alice solo voter → scoops 100 * ONE.
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			assert_eq!(PendingRewards::<Test>::get(ALICE), 100 * ONE);

			// Accumulator after first allocation: 1000 - 100 = 900 * ONE.
			// ref B: alice solo voter → 10% of 900 = 90 * ONE.
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_B,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_B, Status::Completed);
			assert_eq!(PendingRewards::<Test>::get(ALICE), 100 * ONE + 90 * ONE);
		});
}

#[test]
fn on_remove_vote_should_recycle_rounding_dust_to_accumulator() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			// 3 equal voters, weight=1 each, total_weighted=3. Allocation is
			// 10% of 1_000 ONE = 100 ONE, so each share is floor(100 * ONE / 3)
			// which leaves a 1-wei remainder after the last claimant.
			stake(ALICE, 100 * ONE);
			stake(BOB, 100 * ONE);
			stake(CHARLIE, 100 * ONE);

			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 1),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 1),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&CHARLIE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 1),
			));

			// First `on_remove_vote(Completed)` triggers the allocation pull
			// from the accumulator pot into the allocated pot.
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			let accumulator_after_alloc = account_balance(&accumulator_pot());
			assert_eq!(account_balance(&allocated_pot()), 100 * ONE);

			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Completed);
			VotingHooksImpl::<Test>::on_remove_vote(&CHARLIE, REF_A, Status::Completed);

			// All three voters get the same pro-rata share — no scoop bonus.
			let alice = PendingRewards::<Test>::get(ALICE);
			let bob = PendingRewards::<Test>::get(BOB);
			let charlie = PendingRewards::<Test>::get(CHARLIE);
			assert_eq!(alice, bob);
			assert_eq!(bob, charlie);

			let sum = alice + bob + charlie;
			let dust = (100 * ONE) - sum;
			assert!(dust > 0 && dust < 3, "expected 1-wei dust, got {dust}");

			// Dust transferred from allocated pot back to accumulator pot
			// rather than awarded to the last claimant.
			assert_eq!(account_balance(&allocated_pot()), sum);
			assert_eq!(
				account_balance(&accumulator_pot()),
				accumulator_after_alloc + dust,
				"dust returned to accumulator pot"
			);
		});
}

#[test]
fn on_remove_vote_should_credit_zero_to_zero_weighted_voter() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 100 * ONE);
			stake(BOB, 100 * ONE);

			// ALICE: real weight (Locked6x, 10 ONE → weighted = 60 ONE).
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked6x, 10 * ONE),
			));
			// BOB: vote_balance × multiplier / scale floors to 0 (smallest
			// possible vote × None-conviction). With our reward multiplier
			// scale this corresponds to a sub-scale vote balance.
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::None, 1),
			));
			let bob_rec = UserVoteRecords::<Test>::get(BOB, REF_A).unwrap();
			assert_eq!(bob_rec.weighted, 0, "must be a zero-weighted vote");

			// Allocate + claim. Even if BOB is the last claimant, the audit
			// exploit is to scoop the full pool — with the fix he should get
			// exactly zero.
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Completed);

			assert_eq!(PendingRewards::<Test>::get(BOB), 0, "zero-weighted voter gets zero");
			assert!(PendingRewards::<Test>::get(ALICE) > 0);
		});
}

#[test]
fn on_remove_vote_should_delete_pool_when_last_voter_claims() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 100 * ONE);
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			assert!(ReferendaRewardPool::<Test>::get(REF_A).is_none());
		});
}

#[test]
fn on_remove_vote_should_still_cleanup_when_allocation_was_zero() {
	// Empty accumulator: pool inserted with total_reward = 0; last voter triggers cleanup.
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 10 * ONE),
		));
		assert_eq!(account_balance(&accumulator_pot()), 0);

		VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);

		// Pool cleaned up; no pending reward issued.
		assert!(ReferendaRewardPool::<Test>::get(REF_A).is_none());
		assert_eq!(PendingRewards::<Test>::get(ALICE), 0);
		// Allocated pot received nothing.
		assert_eq!(account_balance(&allocated_pot()), 0);
	});
}

#[test]
fn on_remove_vote_should_delete_track_cache_at_allocation() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 100 * ONE);
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			assert_eq!(ReferendumTracks::<Test>::get(REF_A), Some(0u16));

			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			assert!(ReferendumTracks::<Test>::get(REF_A).is_none());
		});
}

/// Sanity: events should include `RewardPoolAllocated` on first completed
/// remove. Kept lightweight — used during debugging only.
#[test]
fn on_remove_vote_should_emit_pool_allocated_event() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 100 * ONE);
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));
			let _ = last_events(0); // touch helper
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			let recent = last_events(10);
			assert!(recent.iter().any(|e| matches!(
				e,
				RuntimeEvent::GigaHdxRewards(Event::RewardPoolAllocated { ref_index, .. }) if *ref_index == REF_A
			)));
		});
}
