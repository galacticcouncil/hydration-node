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

/// Reproduces Bug #5: liquidation must clear GigaHdxVotingLock + LockSplit so that
/// the EVM precompile at 0x0806 reports zero locked GIGAHDX. Without this, AAVE's
/// LockableAToken._transfer reverts during liquidationCall because the stale lock
/// amount exceeds the free balance.
#[test]
fn prepare_for_liquidation_clears_voting_lock_storage() {
	use crate::adapter::GigaHdxVotingCurrency;
	use frame_support::traits::{LockIdentifier, LockableCurrency, WithdrawReasons};

	const VOTING_LOCK: LockIdentifier = *b"pyconvot";

	ExtBuilder::default().build().execute_with(|| {
		set_referendum_outcome(0, ReferendumOutcome::Approved);
		set_track_id(0, 0);

		// Vote to populate GigaHdxVotes + ReferendaTotalWeightedVotes storage.
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		// Simulate conviction-voting's set_lock(300) on our adapter. ALICE has
		// 500 GIGAHDX + 1000 HDX → entire 300 locked in GIGAHDX.
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			300 * ONE,
			WithdrawReasons::all(),
		);

		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 300 * ONE);
		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 300 * ONE);

		// Liquidation entry point.
		assert_ok!(crate::Pallet::<Test>::prepare_for_liquidation(&ALICE));

		// Everything the EVM precompile or native-currency layer can see must be zero.
		assert_eq!(
			crate::GigaHdxVotingLock::<Test>::get(&ALICE),
			0,
			"precompile would still report locked GIGAHDX → AAVE transfer reverts"
		);
		let split_after = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split_after.gigahdx_amount, 0);
		assert_eq!(split_after.hdx_amount, 0);
	});
}

// ---------------------------------------------------------------------------
// on_post_unstake tests
// ---------------------------------------------------------------------------

#[test]
fn on_post_unstake_no_voting_lock_is_noop() {
	ExtBuilder::default().build().execute_with(|| {
		// No lock set. Hook must succeed and leave storage at defaults.
		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));
		assert_eq!(crate::LockSplit::<Test>::get(&ALICE), Default::default());
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 0);
	});
}

#[test]
fn on_post_unstake_unstake_below_free_keeps_tracker() {
	// balance=700 (1000-300 unstaked), tracker=500 (all GIGAHDX). Free portion absorbed unstake.
	ExtBuilder::default().build().execute_with(|| {
		give_gigahdx(&ALICE, 700 * ONE);
		crate::LockSplit::<Test>::insert(
			&ALICE,
			crate::types::VotingLockSplit {
				gigahdx_amount: 500 * ONE,
				hdx_amount: 0,
			},
		);
		crate::GigaHdxVotingLock::<Test>::insert(&ALICE, 500 * ONE);

		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 500 * ONE, "tracker unchanged — free portion absorbed the unstake");
		assert_eq!(split.hdx_amount, 0, "no spillover needed");
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 500 * ONE);
	});
}

#[test]
fn on_post_unstake_unstake_into_locked_spills_to_hdx() {
	// balance=400 (1000-600 unstaked), tracker was 500. New tracker=400, spillover=100.
	ExtBuilder::default().build().execute_with(|| {
		give_gigahdx(&ALICE, 400 * ONE);
		crate::LockSplit::<Test>::insert(
			&ALICE,
			crate::types::VotingLockSplit {
				gigahdx_amount: 500 * ONE,
				hdx_amount: 0,
			},
		);
		crate::GigaHdxVotingLock::<Test>::insert(&ALICE, 500 * ONE);

		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 400 * ONE, "tracker capped at new balance");
		assert_eq!(split.hdx_amount, 100 * ONE, "spillover = old_total - new_tracker");
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 400 * ONE);
	});
}

#[test]
fn on_post_unstake_full_unstake_spills_entire_commitment_to_hdx() {
	// balance=0 (fully unstaked), tracker was 500. New tracker=0, spillover=500.
	ExtBuilder::default().build().execute_with(|| {
		give_gigahdx(&ALICE, 0);
		crate::LockSplit::<Test>::insert(
			&ALICE,
			crate::types::VotingLockSplit {
				gigahdx_amount: 500 * ONE,
				hdx_amount: 0,
			},
		);
		crate::GigaHdxVotingLock::<Test>::insert(&ALICE, 500 * ONE);

		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 500 * ONE, "full commitment spills to HDX");
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 0);
	});
}

#[test]
fn on_post_unstake_with_existing_hdx_spillover_grows_spillover() {
	// Prior split: (500, 200), total=700. balance=400 after unstake 600.
	// Expected new split: gigahdx=400, hdx=300.
	ExtBuilder::default().build().execute_with(|| {
		give_gigahdx(&ALICE, 400 * ONE);
		crate::LockSplit::<Test>::insert(
			&ALICE,
			crate::types::VotingLockSplit {
				gigahdx_amount: 500 * ONE,
				hdx_amount: 200 * ONE,
			},
		);
		crate::GigaHdxVotingLock::<Test>::insert(&ALICE, 500 * ONE);
		// Set the prior HDX-side lock so apply_lock_split replaces rather than stacks it.
		set_hdx_voting_lock(&ALICE, 200 * ONE);

		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));

		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 400 * ONE);
		assert_eq!(split.hdx_amount, 300 * ONE, "spillover grows from 200 to 300");
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 400 * ONE);
	});
}

#[test]
fn on_post_unstake_sequential_partial_unstakes() {
	// Two partial unstakes in sequence.
	ExtBuilder::default().build().execute_with(|| {
		// Start: balance 1000, commitment 500 (all GIGAHDX).
		give_gigahdx(&ALICE, 1_000 * ONE);
		crate::LockSplit::<Test>::insert(
			&ALICE,
			crate::types::VotingLockSplit {
				gigahdx_amount: 500 * ONE,
				hdx_amount: 0,
			},
		);
		crate::GigaHdxVotingLock::<Test>::insert(&ALICE, 500 * ONE);

		// First unstake 200 → balance 800, tracker still 500 (free portion absorbed it).
		give_gigahdx(&ALICE, 800 * ONE);
		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));
		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 500 * ONE);
		assert_eq!(split.hdx_amount, 0);

		// Second unstake 500 → balance 300, tracker caps at 300, spillover 200.
		give_gigahdx(&ALICE, 300 * ONE);
		assert_ok!(crate::Pallet::<Test>::on_post_unstake(&ALICE));
		let split = crate::LockSplit::<Test>::get(&ALICE);
		assert_eq!(split.gigahdx_amount, 300 * ONE);
		assert_eq!(split.hdx_amount, 200 * ONE, "spillover = old_total(500) - new_tracker(300)");
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 300 * ONE);
	});
}
