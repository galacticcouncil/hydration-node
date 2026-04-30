use super::mock::*;
use crate::adapter::GigaHdxVotingCurrency;
use crate::hooks::GigaHdxVotingHooks;
use frame_support::traits::{fungible::Inspect, LockIdentifier, LockableCurrency, WithdrawReasons};
use hydradx_traits::gigahdx::ReferendumOutcome;
use pallet_conviction_voting::{AccountVote, Vote, VotingHooks};

const VOTING_LOCK: LockIdentifier = *b"pyconvot";

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
fn total_balance_should_sum_gigahdx_and_hdx_balances() {
	ExtBuilder::default().build().execute_with(|| {
		let balance = <GigaHdxVotingCurrency<Test> as Inspect<AccountId>>::total_balance(&ALICE);
		// ALICE has 1_000 HDX + 500 GIGAHDX
		assert_eq!(balance, 1_000 * ONE + 500 * ONE);
	});
}

/// Adapter recomputes per-side max from `GigaHdxVotes`. With one active vote
/// of (300, 0), the GIGAHDX-side cap is 300 and the HDX-side cap is 0.
#[test]
fn set_lock_should_take_gigahdx_split_from_active_votes() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE has 500 GIGAHDX → 300 fits entirely on the G-side.
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			300 * ONE,
			WithdrawReasons::all(),
		);

		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 300 * ONE);
	});
}

/// 700 against (500 G, 1000 H): 500 spills onto G-side, 200 onto H-side.
#[test]
fn set_lock_should_spill_to_hdx_when_amount_exceeds_gigahdx_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 700 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			700 * ONE,
			WithdrawReasons::all(),
		);

		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 500 * ONE);
	});
}

/// remove_lock wipes both sides + clears any prior split.
#[test]
fn remove_lock_should_clear_voting_lock_storage() {
	ExtBuilder::default().build().execute_with(|| {
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 700 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			700 * ONE,
			WithdrawReasons::all(),
		);
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 500 * ONE);

		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::remove_lock(VOTING_LOCK, &ALICE);

		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 0);
	});
}

/// Two votes of different sizes: per-side max is the larger vote's split.
#[test]
fn extend_lock_should_max_aggregate_per_side_across_votes() {
	ExtBuilder::default().build().execute_with(|| {
		let vote_a = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote_a));
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
			VOTING_LOCK,
			&ALICE,
			300 * ONE,
			WithdrawReasons::all(),
		);
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 300 * ONE);

		// Second vote — bigger.
		let vote_b = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 600 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 1, vote_b));
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
			VOTING_LOCK,
			&ALICE,
			600 * ONE,
			WithdrawReasons::all(),
		);
		// 600 against 500 G + 1000 H → 500 G-side, 100 H-side.
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 500 * ONE);
	});
}

/// Live testnet bug: a smaller follow-up vote after a stake must refresh the
/// per-side split. Under the new model every vote records its own split into
/// `GigaHdxVotes`, so the smaller vote still contributes correctly to the
/// per-side max-aggregate. The previous `extend_lock` `amount >= current_total`
/// guard is gone.
#[test]
fn lock_split_should_use_per_vote_snapshot_when_smaller_vote_follows_stake() {
	use frame_support::traits::fungibles::Mutate as FungiblesMutate;

	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE)]) // no GIGAHDX initially
		.build()
		.execute_with(|| {
			// First vote uses HDX only — 500 H locked, 0 G.
			let v1 = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 500 * ONE);
			assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, v1));
			<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
				VOTING_LOCK,
				&ALICE,
				500 * ONE,
				WithdrawReasons::all(),
			);
			assert_eq!(
				crate::GigaHdxVotingLock::<Test>::get(&ALICE),
				0,
				"no GIGAHDX yet → G-side cap is 0"
			);

			// Now ALICE stakes — gains 200 GIGAHDX.
			<<Test as pallet_gigahdx::Config>::Currency>::mint_into(GIGAHDX, &ALICE, 200 * ONE).unwrap();

			// A smaller follow-up vote (200) lands on a different referendum.
			// In the old design `extend_lock`'s amount-guard would skip the recompute.
			// Under the new design every vote records its own split and the per-side max
			// reflects this vote's GIGAHDX contribution.
			let v2 = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
			assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 1, v2));
			<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
				VOTING_LOCK,
				&ALICE,
				500 * ONE,
				WithdrawReasons::all(),
			);

			assert_eq!(
				crate::GigaHdxVotingLock::<Test>::get(&ALICE),
				200 * ONE,
				"smaller follow-up vote contributes its 200 GIGAHDX to G-side max"
			);
		});
}

/// Per-vote subtraction: removing the larger vote releases its specific
/// contribution. Per-side max collapses to the smaller vote's split.
#[test]
fn lock_split_should_collapse_to_smaller_when_larger_vote_removed() {
	ExtBuilder::default().build().execute_with(|| {
		set_referendum_outcome(0, ReferendumOutcome::Ongoing);
		set_referendum_outcome(1, ReferendumOutcome::Ongoing);

		let small = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 100 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, small));
		let big = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 400 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 1, big));
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::extend_lock(
			VOTING_LOCK,
			&ALICE,
			400 * ONE,
			WithdrawReasons::all(),
		);
		assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), 400 * ONE);

		// Remove the bigger vote (ongoing). Hook deletes the entry; the lock itself
		// is not yet touched (mirrors upstream).
		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 1, pallet_conviction_voting::Status::Ongoing);
		assert_eq!(
			crate::GigaHdxVotingLock::<Test>::get(&ALICE),
			400 * ONE,
			"remove_vote alone does not shrink the lock — mirrors upstream"
		);

		// Triggering the recompute (e.g. via `unlock` → `set_lock`) collapses to small vote's split.
		<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
			VOTING_LOCK,
			&ALICE,
			100 * ONE,
			WithdrawReasons::all(),
		);
		assert_eq!(
			crate::GigaHdxVotingLock::<Test>::get(&ALICE),
			100 * ONE,
			"per-side max now reflects only the surviving smaller vote"
		);
	});
}

/// HDX-only voter (no GIGAHDX): G-side stays 0 even when conviction-voting
/// asks for a lock. Voting is fully covered by the HDX-side native lock.
#[test]
fn gigahdx_lock_should_be_zero_when_voter_holds_no_gigahdx() {
	ExtBuilder::default()
		.with_endowed(vec![(CHARLIE, HDX, 1_000 * ONE)])
		.build()
		.execute_with(|| {
			let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 500 * ONE);
			assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&CHARLIE, 0, vote));

			<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
				VOTING_LOCK,
				&CHARLIE,
				500 * ONE,
				WithdrawReasons::all(),
			);

			assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&CHARLIE), 0);
		});
}
