// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::pallet::{
	Event, PendingRewards, ReferendaRewardPool, ReferendaTotalWeightedVotes, ReferendumTracks, UserVoteRecords,
};
use crate::types::REWARD_MULTIPLIER_SCALE;
use crate::voting_hooks::VotingHooksImpl;
use pallet_gigahdx::traits::VotingCommitmentInspect;

use frame_support::assert_ok;
use frame_system::RawOrigin;
use pallet_conviction_voting::{AccountVote, Conviction, Status, Vote, VotingHooks};

const REF_A: u32 = 7;
const REF_B: u32 = 9;
const REF_C: u32 = 11;

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
		assert_eq!(GigaHdxRewards::committed(&ALICE), 50 * ONE);
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
		assert_eq!(GigaHdxRewards::committed(&ALICE), 0);
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
		assert_eq!(GigaHdxRewards::committed(&ALICE), 40 * ONE);

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
		assert_eq!(GigaHdxRewards::committed(&ALICE), 0);
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
		// Locked1x = 25 / 100 = 0.25× → weighted = 12.5 * ONE
		assert_eq!(rec.weighted, 50 * ONE * 25 / REWARD_MULTIPLIER_SCALE);
	});
}

#[test]
fn on_before_vote_should_replace_record_when_vote_is_edited() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);

		// First vote: 30 HDX, Locked1x (0.25×) → weighted = 7.5 * ONE.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 30 * ONE),
		));
		let tally_1 = ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap();
		assert_eq!(tally_1.voters_count, 1);
		assert_eq!(tally_1.total_weighted, 30 * ONE * 25 / REWARD_MULTIPLIER_SCALE);

		// Edit: 80 HDX, Locked2x (0.5×) → weighted = 40 * ONE.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked2x, 80 * ONE),
		));
		let rec = UserVoteRecords::<Test>::get(ALICE, REF_A).unwrap();
		assert_eq!(rec.staked_vote_amount, 80 * ONE);
		assert_eq!(rec.weighted, 80 * ONE * 50 / REWARD_MULTIPLIER_SCALE);

		let tally_2 = ReferendaTotalWeightedVotes::<Test>::get(REF_A).unwrap();
		assert_eq!(tally_2.voters_count, 1, "voter count unchanged on edit");
		assert_eq!(tally_2.total_weighted, 80 * ONE * 50 / REWARD_MULTIPLIER_SCALE);
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
		assert_eq!(GigaHdxRewards::committed(&ALICE), 0);

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 60 * ONE),
		));
		assert_eq!(GigaHdxRewards::committed(&ALICE), 60 * ONE);

		// Edit lowers vote → frozen recomputed (unfreeze old, freeze new).
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, 20 * ONE),
		));
		assert_eq!(GigaHdxRewards::committed(&ALICE), 20 * ONE);
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
		assert_eq!(GigaHdxRewards::committed(&ALICE), 70 * ONE);

		VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Ongoing);
		assert_eq!(GigaHdxRewards::committed(&ALICE), 0);
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
fn on_remove_vote_should_record_counted_voter_when_referendum_pruned_after_allocation() {
	ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
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
				standard_vote(true, Conviction::Locked1x, 10 * ONE),
			));

			// Alice finalizes the referendum: pool allocated, alice recorded.
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			let pool = ReferendaRewardPool::<Test>::get(REF_A).expect("pool created");
			assert_eq!(pool.voters_remaining, 1, "bob still outstanding");
			assert_eq!(PendingRewards::<Test>::get(ALICE), 50 * ONE);

			let acc_before = account_balance(&accumulator_pot());
			let alloc_before = account_balance(&allocated_pot());

			// Bob removes his vote only after the referendum info was pruned, so
			// the hook sees `Status::None`. He is still a counted voter and must
			// be accounted against the allocated pool — otherwise the pool entry
			// and his pro-rata share leak permanently.
			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::None);

			assert_eq!(PendingRewards::<Test>::get(BOB), 50 * ONE, "bob credited his share");
			assert!(
				ReferendaRewardPool::<Test>::get(REF_A).is_none(),
				"pool reaped once the last counted voter is accounted",
			);
			// Even split (no rounding remainder) → no recycle and no re-allocation.
			assert_eq!(account_balance(&allocated_pot()), alloc_before, "no spurious transfer");
			assert_eq!(account_balance(&accumulator_pot()), acc_before, "no re-allocation");
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

			// Locked3x (1× weight) with 1-wei vote → weighted=1 each, total=3.
			// 10% of 1_000 ONE allocated = 100 ONE; floor(100*ONE/3) per voter
			// leaves 1 wei dust after the last claimant.
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked3x, 1),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::Locked3x, 1),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&CHARLIE,
				REF_A,
				standard_vote(true, Conviction::Locked3x, 1),
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

/// Sybil-resistance: splitting the same stake across two accounts must not earn
/// more than a single account voting with the combined stake. Weighted votes
/// are linear in stake and the pool is a fixed pro-rata split, so two half-votes
/// sum to exactly one full vote. An honest co-voter (BOB) is present in both
/// scenarios so the attacker competes for a *shared* pool — a solo voter would
/// trivially scoop 100% either way. Scaled down from the 1M / 500K+500K example;
/// the property is scale-invariant.
#[test]
fn on_remove_vote_should_not_reward_more_when_voter_splits_stake_across_accounts() {
	// Scenario A — one account votes the full stake (ALICE = attacker).
	let single = ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 600 * ONE);
			stake(BOB, 400 * ONE);

			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked6x, 600 * ONE),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::Locked6x, 400 * ONE),
			));
			// weighted (×8): ALICE 4800, BOB 3200 → total 8000 ONE.

			// Attacker removes first (not last → no dust scoop); BOB last.
			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Completed);

			// pool = 10% of 1000 = 100 ONE. ALICE = floor(4800·100/8000) = 60.
			assert_eq!(PendingRewards::<Test>::get(ALICE), 60 * ONE);
			assert_eq!(PendingRewards::<Test>::get(BOB), 40 * ONE);
			PendingRewards::<Test>::get(ALICE)
		});

	// Scenario B — same stake split across two accounts (ALICE + CHARLIE),
	// honest co-voter BOB unchanged.
	let split = ExtBuilder::default()
		.with_accumulator(1_000 * ONE)
		.build()
		.execute_with(|| {
			stake(ALICE, 300 * ONE);
			stake(CHARLIE, 300 * ONE);
			stake(BOB, 400 * ONE);

			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				REF_A,
				standard_vote(true, Conviction::Locked6x, 300 * ONE),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&CHARLIE,
				REF_A,
				standard_vote(true, Conviction::Locked6x, 300 * ONE),
			));
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&BOB,
				REF_A,
				standard_vote(true, Conviction::Locked6x, 400 * ONE),
			));
			// weighted (×8): 2400 + 2400 + 3200 → total 8000 ONE (identical).

			VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_A, Status::Completed);
			VotingHooksImpl::<Test>::on_remove_vote(&CHARLIE, REF_A, Status::Completed);
			VotingHooksImpl::<Test>::on_remove_vote(&BOB, REF_A, Status::Completed);

			// ALICE = CHARLIE = floor(2400·100/8000) = 30 each.
			assert_eq!(PendingRewards::<Test>::get(ALICE), 30 * ONE);
			assert_eq!(PendingRewards::<Test>::get(CHARLIE), 30 * ONE);
			assert_eq!(PendingRewards::<Test>::get(BOB), 40 * ONE);
			PendingRewards::<Test>::get(ALICE) + PendingRewards::<Test>::get(CHARLIE)
		});

	assert!(split <= single, "splitting stake must never increase rewards");
	assert_eq!(split, single, "split {split} must equal single {single}");
}

// ---------------------------------------------------------------------------
// Frozen-overlap spec (currently FAILING — drives the `frozen` redesign).
//
// `frozen` must equal the MAX over the user's active per-referendum
// reservations (the overlapping commitment), not their SUM. The hook today
// stacks `freeze`/`unfreeze` deltas, so concurrent partial votes over-freeze.
// These unit specs are the fast mirror of the integration tests in
// `integration-tests/src/gigahdx_rewards.rs`; they flip green once `frozen`
// becomes a recomputed max.
// ---------------------------------------------------------------------------

/// Three partial votes (X/2 each) over the same stake overlap — only X/2 is
/// ever committed. CURRENT BUG: frozen sums to 3 * X/2.
#[test]
fn frozen_should_equal_overlap_not_sum_when_voting_partial_on_multiple_referenda() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		let half = 50 * ONE;

		for r in [REF_A, REF_B, REF_C] {
			assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
				&ALICE,
				r,
				standard_vote(true, Conviction::Locked3x, half),
			));
		}

		// DESIRED: max(X/2, X/2, X/2) = X/2.  CURRENT BUG: 3 * X/2.
		assert_eq!(
			GigaHdxRewards::committed(&ALICE),
			half,
			"frozen must be the overlap (X/2), not the sum of the votes"
		);
	});
}

/// Removing the single largest vote must recompute `frozen` down to the
/// next-highest reservation. CURRENT BUG: `frozen -= removed` leaves sum-minus-max.
#[test]
fn frozen_should_recompute_to_second_highest_when_largest_vote_removed() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		let quarter = 25 * ONE;

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, quarter),
		));
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_B,
			standard_vote(true, Conviction::Locked1x, quarter),
		));
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_C,
			standard_vote(true, Conviction::Locked1x, 100 * ONE),
		));

		VotingHooksImpl::<Test>::on_remove_vote(&ALICE, REF_C, Status::Ongoing);

		// DESIRED: max(X/4, X/4) = X/4.  CURRENT BUG: 1.5X - X = X/2.
		assert_eq!(
			GigaHdxRewards::committed(&ALICE),
			quarter,
			"removing the largest vote must recompute frozen to the next-highest reservation"
		);
	});
}

/// Editing the largest vote *down* must recompute `frozen` to the true
/// overlap, not just subtract the delta off a summed base.
#[test]
fn frozen_should_lower_when_largest_vote_edited_down() {
	ExtBuilder::default().build().execute_with(|| {
		stake(ALICE, 100 * ONE);
		let quarter = 25 * ONE;

		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_A,
			standard_vote(true, Conviction::Locked1x, quarter),
		));
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_B,
			standard_vote(true, Conviction::Locked1x, 100 * ONE),
		));

		// Edit REF_B down to X/4.
		assert_ok!(VotingHooksImpl::<Test>::on_before_vote(
			&ALICE,
			REF_B,
			standard_vote(true, Conviction::Locked1x, quarter),
		));

		// DESIRED: max(X/4, X/4) = X/4.  CURRENT BUG: X/2.
		assert_eq!(
			GigaHdxRewards::committed(&ALICE),
			quarter,
			"editing the largest vote down must recompute frozen, not subtract a delta off a summed base"
		);
	});
}
