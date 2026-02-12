use super::mock::*;
use crate::hooks::GigaHdxVotingHooks;
use frame_support::traits::fungibles::Mutate;
use hydradx_traits::gigahdx::ReferendumOutcome;
use pallet_conviction_voting::{AccountVote, Status, Vote, VotingHooks};
use pallet_currencies::fungibles::FungibleCurrencies;

fn standard_vote(
	aye: bool,
	conviction: pallet_conviction_voting::Conviction,
	balance: Balance,
) -> AccountVote<Balance> {
	AccountVote::Standard {
		vote: Vote { aye, conviction },
		balance,
	}
}

#[test]
fn reward_pool_allocated_on_first_remove() {
	ExtBuilder::default().build().execute_with(|| {
		// Fund the GigaReward pot.
		let pot = crate::Pallet::<Test>::giga_reward_pot_account();
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 10_000 * ONE).unwrap();

		// Setup: referendum 0 on track 0 (10% reward).
		set_track_id(0, 0);
		set_referendum_outcome(0, ReferendumOutcome::Approved);

		// ALICE votes.
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked2x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		// Remove vote with Completed status → triggers reward allocation.
		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::Completed);

		// Should have allocated 10% of 10_000 = 1_000 HDX.
		assert!(crate::RewardAllocated::<Test>::get(0));
		let pool = crate::ReferendaRewardPool::<Test>::get(0).expect("pool should exist");
		assert_eq!(pool.total_reward, 1_000 * ONE);
		assert_eq!(pool.track_id, 0);
	});
}

#[test]
fn reward_not_allocated_for_non_completed() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = crate::Pallet::<Test>::giga_reward_pot_account();
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 10_000 * ONE).unwrap();

		set_track_id(0, 0);
		set_referendum_outcome(0, ReferendumOutcome::Ongoing);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		// Remove with None status (cancelled) → no reward.
		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::None);

		assert!(!crate::RewardAllocated::<Test>::get(0));
		assert!(crate::ReferendaRewardPool::<Test>::get(0).is_none());
		assert!(crate::PendingRewards::<Test>::get(&ALICE).is_empty());
	});
}

#[test]
fn conviction_weighted_reward_distribution() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = crate::Pallet::<Test>::giga_reward_pot_account();
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 10_000 * ONE).unwrap();

		set_track_id(0, 0);
		set_referendum_outcome(0, ReferendumOutcome::Approved);

		// ALICE votes with Locked3x conviction (multiplier 3), 300 GIGAHDX.
		let vote_a = standard_vote(true, pallet_conviction_voting::Conviction::Locked3x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote_a));

		// BOB votes with Locked1x conviction (multiplier 1), 300 GIGAHDX.
		let vote_b = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&BOB, 0, vote_b));

		// Total weighted: ALICE = 300*3=900, BOB = 300*1=300 → total = 1200.
		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 1_200 * ONE);

		// ALICE removes → triggers allocation.
		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::Completed);

		let alice_rewards = crate::PendingRewards::<Test>::get(&ALICE);
		assert_eq!(alice_rewards.len(), 1);
		// ALICE's share: 900/1200 * 1000 = 750 HDX.
		assert_eq!(alice_rewards[0].reward_amount, 750 * ONE);

		// BOB removes.
		GigaHdxVotingHooks::<Test>::on_remove_vote(&BOB, 0, Status::Completed);

		let bob_rewards = crate::PendingRewards::<Test>::get(&BOB);
		assert_eq!(bob_rewards.len(), 1);
		// BOB's share: 300/1200 * 1000 = 250 HDX.
		assert_eq!(bob_rewards[0].reward_amount, 250 * ONE);
	});
}

#[test]
fn remaining_reward_tracks_correctly() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = crate::Pallet::<Test>::giga_reward_pot_account();
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 10_000 * ONE).unwrap();

		set_track_id(0, 0);
		set_referendum_outcome(0, ReferendumOutcome::Approved);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::Completed);

		let pool = crate::ReferendaRewardPool::<Test>::get(0).unwrap();
		// Single voter gets all: 1000 - 1000 = 0 remaining.
		assert_eq!(pool.remaining_reward, 0);
	});
}

#[test]
fn empty_reward_pot_allocates_zero() {
	ExtBuilder::default().build().execute_with(|| {
		// No funds in reward pot.
		set_track_id(0, 0);
		set_referendum_outcome(0, ReferendumOutcome::Approved);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::Completed);

		// RewardAllocated should be set even with zero pot.
		assert!(crate::RewardAllocated::<Test>::get(0));
		// No pending rewards since allocation was zero.
		assert!(crate::PendingRewards::<Test>::get(&ALICE).is_empty());
	});
}
