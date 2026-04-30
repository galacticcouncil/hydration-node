use super::mock::*;
use crate::adapter::GigaHdxVotingCurrency;
use crate::hooks::GigaHdxVotingHooks;
use frame_support::traits::{LockIdentifier, LockableCurrency, WithdrawReasons};
use hydradx_traits::gigahdx::ReferendumOutcome;
use pallet_conviction_voting::{AccountVote, Conviction, Vote, VotingHooks};

const PYCONVOT: LockIdentifier = *b"pyconvot";

fn standard_vote(aye: bool, conviction: Conviction, balance: Balance) -> AccountVote<Balance> {
	AccountVote::Standard {
		vote: Vote { aye, conviction },
		balance,
	}
}

/// Setup helper — stakes `stake_amount`, casts a vote with the given conviction,
/// applies the resulting voting lock, and marks the referendum finished so
/// `on_unstake` can force-remove it.
fn setup_voted_unstake(stake_amount: Balance, vote_amount: Balance, conviction: Conviction) {
	assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), stake_amount));

	set_referendum_outcome(0, ReferendumOutcome::Approved);
	set_track_id(0, 0);

	let vote = standard_vote(true, conviction, vote_amount);
	assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));
	<GigaHdxVotingCurrency<Test> as LockableCurrency<AccountId>>::set_lock(
		PYCONVOT,
		&ALICE,
		vote_amount,
		WithdrawReasons::all(),
	);
	assert_eq!(crate::GigaHdxVotingLock::<Test>::get(&ALICE), vote_amount);
}

#[test]
fn giga_unstake_should_not_split_when_voting_lock_within_base_cooldown() {
	// Locked1x → lock = 1 × VoteLockingPeriod(10) = 10 blocks ≤ CooldownPeriod(100).
	// Even though there's a voting commitment, the conviction cooldown collapses
	// into the base cooldown so the unstake yields a single position.
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE), (ALICE, GIGAHDX, 200 * ONE)])
		.build()
		.execute_with(|| {
			setup_voted_unstake(200 * ONE, 100 * ONE, Conviction::Locked1x);

			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 200 * ONE));

			let positions = pallet_gigahdx::UnstakePositions::<Test>::get(&ALICE);
			assert_eq!(positions.len(), 1, "no split when vote_lock ≤ base");
			assert_eq!(positions[0].amount, 200 * ONE);
			assert_eq!(positions[0].unlock_at, 1 + 100, "single position uses base cooldown");
		});
}

#[test]
fn giga_unstake_should_fail_with_too_many_positions_when_split_does_not_fit() {
	// 9 pre-existing + projected split (2) = 11 > MaxUnstakePositions(10).
	// Precheck must reject before on_unstake / MM withdraw / burn / transfer run.
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE), (ALICE, GIGAHDX, 200 * ONE)])
		.build()
		.execute_with(|| {
			setup_voted_unstake(200 * ONE, 100 * ONE, Conviction::Locked6x);

			pallet_gigahdx::UnstakePositions::<Test>::mutate(&ALICE, |positions| {
				for i in 0..9u64 {
					positions
						.try_push(pallet_gigahdx::types::UnstakePosition {
							amount: ONE,
							unlock_at: 1_000 + i,
						})
						.unwrap();
				}
			});

			assert_noop!(
				GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 150 * ONE),
				pallet_gigahdx::Error::<Test>::TooManyUnstakePositions,
			);

			// Failure must be at the precheck — state untouched.
			assert_eq!(pallet_gigahdx::UnstakePositions::<Test>::get(&ALICE).len(), 9);
		});
}

#[test]
fn second_giga_unstake_should_observe_capped_voting_lock_from_first_unstake() {
	// First unstake 250 of 300: free pool = 300-200 = 100 → free=100, voted=150 (split).
	// After first unstake balance is 50, so on_unstake caps GigaHdxVotingLock at 50
	// (was 200) and spills the 150 difference to UnstakeSpillover.
	// Second unstake 50: must see the *capped* lock (50), not the original (200).
	// Free pool = 50-50 = 0 → free=0, voted=50, single conviction-cooldown position.
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE), (ALICE, GIGAHDX, 300 * ONE)])
		.build()
		.execute_with(|| {
			setup_voted_unstake(300 * ONE, 200 * ONE, Conviction::Locked6x);

			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 250 * ONE));
			assert_eq!(
				crate::GigaHdxVotingLock::<Test>::get(&ALICE),
				50 * ONE,
				"on_unstake caps G-side at post-unstake balance",
			);
			assert_eq!(pallet_gigahdx::UnstakePositions::<Test>::get(&ALICE).len(), 2);

			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 50 * ONE));

			let positions = pallet_gigahdx::UnstakePositions::<Test>::get(&ALICE);
			assert_eq!(positions.len(), 3, "second unstake adds one (no free portion)");
			let last = positions.last().unwrap();
			assert_eq!(last.amount, 50 * ONE);
			assert_eq!(
				last.unlock_at,
				1 + 32 * 10,
				"second unstake voted-cooldown driven by the still-active prior",
			);
		});
}

#[test]
fn giga_unstake_should_succeed_when_split_just_fits_position_cap() {
	// 8 pre-existing + 2 split = 10 = cap → boundary success.
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE), (ALICE, GIGAHDX, 200 * ONE)])
		.build()
		.execute_with(|| {
			setup_voted_unstake(200 * ONE, 100 * ONE, Conviction::Locked6x);

			pallet_gigahdx::UnstakePositions::<Test>::mutate(&ALICE, |positions| {
				for i in 0..8u64 {
					positions
						.try_push(pallet_gigahdx::types::UnstakePosition {
							amount: ONE,
							unlock_at: 1_000 + i,
						})
						.unwrap();
				}
			});

			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 150 * ONE));

			let positions = pallet_gigahdx::UnstakePositions::<Test>::get(&ALICE);
			assert_eq!(positions.len(), 10, "8 existing + 2 from split");

			// 200 balance, 100 lock → free pool = 100. Unstake 150 → free=100, voted=50.
			// Free pushed first, voted second.
			let new_positions = &positions[8..];
			assert_eq!(new_positions[0].amount, 100 * ONE, "free portion = 100");
			assert_eq!(new_positions[0].unlock_at, 1 + 100, "free uses base cooldown");
			assert_eq!(new_positions[1].amount, 50 * ONE, "voted portion = 50");
			assert_eq!(
				new_positions[1].unlock_at,
				1 + 32 * 10,
				"voted uses Locked6x cooldown (32 × VoteLockingPeriod)",
			);
		});
}
