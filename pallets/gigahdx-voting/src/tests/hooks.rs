use super::mock::*;
use crate::hooks::GigaHdxVotingHooks;
use crate::types::Conviction;
use pallet_conviction_voting::{AccountVote, Status, Vote, VotingHooks};

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
fn on_before_vote_records_gigahdx_vote() {
	ExtBuilder::default().build().execute_with(|| {
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked3x, 400 * ONE);

		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		let recorded = crate::GigaHdxVotes::<Test>::get(&ALICE, 0).expect("vote should exist");
		// ALICE has 500 GIGAHDX, voted with 400, so amount = 400.
		assert_eq!(recorded.amount, 400 * ONE);
		assert_eq!(recorded.conviction, Conviction::Locked3x);

		// Weighted: 400 * 3 = 1200.
		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 1_200 * ONE);
	});
}

#[test]
fn on_before_vote_caps_at_gigahdx_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE has 500 GIGAHDX but votes with 800 total.
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 800 * ONE);

		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		let recorded = crate::GigaHdxVotes::<Test>::get(&ALICE, 0).expect("vote should exist");
		assert_eq!(recorded.amount, 500 * ONE); // Capped at GIGAHDX balance.
	});
}

#[test]
fn on_before_vote_hdx_only_voter_noop() {
	ExtBuilder::default()
		.with_endowed(vec![
			(CHARLIE, HDX, 1_000 * ONE),
			// No GIGAHDX
		])
		.build()
		.execute_with(|| {
			let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 500 * ONE);
			assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&CHARLIE, 0, vote));

			assert!(crate::GigaHdxVotes::<Test>::get(&CHARLIE, 0).is_none());
			assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 0);
		});
}

#[test]
fn on_before_vote_split_uses_none_conviction() {
	ExtBuilder::default().build().execute_with(|| {
		let vote = AccountVote::Split {
			aye: 200 * ONE,
			nay: 100 * ONE,
		};
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		let recorded = crate::GigaHdxVotes::<Test>::get(&ALICE, 0).expect("vote should exist");
		assert_eq!(recorded.conviction, Conviction::None);
		// Balance is 200 + 100 = 300, capped at GIGAHDX balance 500 → 300.
		assert_eq!(recorded.amount, 300 * ONE);
		// Weighted: 300 * 1 / REWARD_MULTIPLIER_SCALE(10) = 30. None conviction has a 0.1x reward weight.
		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 30 * ONE);
	});
}

#[test]
fn on_before_vote_update_replaces_old() {
	ExtBuilder::default().build().execute_with(|| {
		let vote1 = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote1));
		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 200 * ONE);

		// Update with higher conviction.
		let vote2 = standard_vote(true, pallet_conviction_voting::Conviction::Locked3x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote2));

		// Old weighted (200*1=200) subtracted, new (200*3=600) added.
		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 600 * ONE);
		let recorded = crate::GigaHdxVotes::<Test>::get(&ALICE, 0).unwrap();
		assert_eq!(recorded.conviction, Conviction::Locked3x);
	});
}

#[test]
fn on_remove_vote_clears_storage() {
	ExtBuilder::default().build().execute_with(|| {
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked2x, 300 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 600 * ONE);

		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::None);

		assert!(crate::GigaHdxVotes::<Test>::get(&ALICE, 0).is_none());
		assert_eq!(crate::ReferendaTotalWeightedVotes::<Test>::get(0), 0);
	});
}

#[test]
fn on_remove_vote_nonexistent_noop() {
	ExtBuilder::default().build().execute_with(|| {
		// Should not panic.
		GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 99, Status::Completed);
	});
}

#[test]
fn lock_balance_on_unsuccessful_vote_returns_amount() {
	ExtBuilder::default().build().execute_with(|| {
		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 400 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		let locked = GigaHdxVotingHooks::<Test>::lock_balance_on_unsuccessful_vote(&ALICE, 0);
		assert_eq!(locked, Some(400 * ONE));
	});
}

#[test]
fn lock_balance_on_unsuccessful_vote_none_if_no_vote() {
	ExtBuilder::default().build().execute_with(|| {
		let locked = GigaHdxVotingHooks::<Test>::lock_balance_on_unsuccessful_vote(&ALICE, 0);
		assert_eq!(locked, None);
	});
}

#[test]
fn lock_expires_at_calculated_correctly() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked4x, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		let recorded = crate::GigaHdxVotes::<Test>::get(&ALICE, 0).unwrap();
		assert_eq!(recorded.voted_at, 10);
		// Locked4x = 8 periods, VoteLockingPeriod = 10 blocks → 80 blocks lock.
		assert_eq!(recorded.lock_expires_at, 10 + 80);
	});
}

#[test]
fn conviction_none_lock_expires_immediately() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(5);

		let vote = standard_vote(true, pallet_conviction_voting::Conviction::None, 200 * ONE);
		assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));

		let recorded = crate::GigaHdxVotes::<Test>::get(&ALICE, 0).unwrap();
		// None = 0 periods → lock_expires_at = current block.
		assert_eq!(recorded.lock_expires_at, 5);
	});
}
