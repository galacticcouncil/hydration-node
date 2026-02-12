use super::mock::*;
use crate::hooks::GigaHdxVotingHooks;
use hydradx_traits::gigahdx::{GigaHdxHooks, ReferendumOutcome};
use pallet_conviction_voting::{AccountVote, Vote, VotingHooks};

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
fn can_unstake_true_when_no_votes() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(crate::Pallet::<Test>::can_unstake(&ALICE));
	});
}

#[test]
fn can_unstake_false_with_ongoing_referendum() {
	ExtBuilder::default().build().execute_with(|| {
		set_referendum_outcome(0, ReferendumOutcome::Ongoing);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		assert!(!crate::Pallet::<Test>::can_unstake(&ALICE));
	});
}

#[test]
fn can_unstake_true_when_all_finished() {
	ExtBuilder::default().build().execute_with(|| {
		set_referendum_outcome(0, ReferendumOutcome::Approved);
		set_referendum_outcome(1, ReferendumOutcome::Rejected);

		let vote0 = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote0));

		let vote1 = standard_vote(false, pallet_conviction_voting::Conviction::Locked2x, 100 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 1, vote1));

		assert!(crate::Pallet::<Test>::can_unstake(&ALICE));
	});
}

#[test]
fn on_unstake_force_removes_finished_votes() {
	ExtBuilder::default().build().execute_with(|| {
		set_referendum_outcome(0, ReferendumOutcome::Approved);
		set_track_id(0, 0);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		clear_force_remove_calls();

		assert_ok!(crate::Pallet::<Test>::on_unstake(&ALICE, 100 * ONE));

		let calls = get_force_remove_calls();
		assert_eq!(calls.len(), 1);
		assert_eq!(calls[0], (ALICE, Some(0), 0)); // (who, track_id, ref_index)
	});
}

#[test]
fn additional_unstake_lock_returns_max_remaining() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		// Vote on ref 0 with Locked2x (2 periods * 10 blocks = 20 blocks lock).
		let vote0 = standard_vote(true, pallet_conviction_voting::Conviction::Locked2x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote0));

		// Vote on ref 1 with Locked4x (8 periods * 10 blocks = 80 blocks lock).
		let vote1 = standard_vote(true, pallet_conviction_voting::Conviction::Locked4x, 100 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 1, vote1));

		// At block 10: lock_expires_at for ref 0 = 30, ref 1 = 90.
		// Remaining: ref 0 = 20, ref 1 = 80. Max = 80.
		let lock = crate::Pallet::<Test>::additional_unstake_lock(&ALICE);
		assert_eq!(lock, 80);
	});
}

#[test]
fn additional_unstake_lock_zero_when_expired() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(5);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		// Lock expires at 5 + 10 = 15. Advance to block 20.
		run_to_block(20);

		let lock = crate::Pallet::<Test>::additional_unstake_lock(&ALICE);
		assert_eq!(lock, 0);
	});
}

#[test]
fn additional_unstake_lock_zero_no_votes() {
	ExtBuilder::default().build().execute_with(|| {
		let lock = crate::Pallet::<Test>::additional_unstake_lock(&ALICE);
		assert_eq!(lock, 0);
	});
}
