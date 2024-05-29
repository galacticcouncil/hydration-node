use super::*;

fn the_lock(amount: u64) -> BalanceLock<u64> {
	BalanceLock {
		id: DEMOCRACY_ID,
		amount,
		reasons: pallet_balances::Reasons::All,
	}
}

fn aye(x: u8, balance: u64) -> AccountVote<u64> {
	AccountVote::Standard {
		vote: Vote {
			aye: true,
			conviction: Conviction::try_from(x).unwrap(),
		},
		balance,
	}
}

fn nay(x: u8, balance: u64) -> AccountVote<u64> {
	AccountVote::Standard {
		vote: Vote {
			aye: false,
			conviction: Conviction::try_from(x).unwrap(),
		},
		balance,
	}
}

#[test]
fn vote_should_call_on_vote_hook() {
	new_test_ext().execute_with(|| {
		let r = Democracy::inject_referendum(2, set_balance_proposal(2), VoteThreshold::SuperMajorityApprove, 0);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(1), r, big_aye(1)));
		let expected_data = OnVoteData {
			who: 1,
			ref_index: 0,
			vote: big_aye(1),
		};
		assert_eq!(HooksHandler::last_on_vote_data(), expected_data);
	});
}

#[test]
fn remove_vote_should_call_on_remove_vote_hook() {
	new_test_ext().execute_with(|| {
		let r = Democracy::inject_referendum(2, set_balance_proposal(2), VoteThreshold::SuperMajorityApprove, 0);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(1), r, big_aye(1)));
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(1), r));
		let expected_data = OnRemoveVoteData { who: 1, ref_index: 0 };
		assert_eq!(HooksHandler::last_on_remove_vote_data(), expected_data);
	});
}

#[test]
fn remove_vote_should_not_extend_lock_when_voted_not_in_favor_and_hook_returns_false() {
	new_test_ext().execute_with(|| {
		let r = Democracy::inject_referendum(2, set_balance_proposal(2), VoteThreshold::SuperMajorityApprove, 0);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(1), r, nay(5, 10)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(2), r, aye(4, 20)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(3), r, aye(3, 30)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(4), r, aye(2, 40)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(5), r, nay(1, 50)));
		assert_eq!(
			tally(r),
			Tally {
				ayes: 250,
				nays: 100,
				turnout: 150
			}
		);
		fast_forward_to(3);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(1), r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(1), 1));
		assert_eq!(Balances::locks(1), vec![]);
		assert_eq!(Balances::usable_balance(1), 10);
	});
}

#[test]
fn remove_vote_should_extend_lock_when_voted_not_in_favor_hook_returns_true() {
	new_test_ext().execute_with(|| {
		HooksHandler::with_remove_vote_locked_amount(1, 10);
		let r = Democracy::inject_referendum(2, set_balance_proposal(2), VoteThreshold::SuperMajorityApprove, 0);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(1), r, nay(5, 10)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(2), r, aye(4, 20)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(3), r, aye(3, 30)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(4), r, aye(2, 40)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(5), r, nay(1, 50)));
		assert_eq!(
			tally(r),
			Tally {
				ayes: 250,
				nays: 100,
				turnout: 150
			}
		);
		fast_forward_to(3);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(1), r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(1), 1));
		assert_eq!(Balances::locks(1), vec![the_lock(10)]);
		assert_eq!(Balances::usable_balance(1), 0);
	});
}

#[test]
fn remove_vote_should_extend_lock_only_for_given_amount_when_voted_not_in_favor_hook_returns_true() {
	new_test_ext().execute_with(|| {
		HooksHandler::with_remove_vote_locked_amount(1, 5);
		let r = Democracy::inject_referendum(2, set_balance_proposal(2), VoteThreshold::SuperMajorityApprove, 0);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(1), r, nay(5, 10)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(2), r, aye(4, 20)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(3), r, aye(3, 30)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(4), r, aye(2, 40)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(5), r, nay(1, 50)));
		assert_eq!(
			tally(r),
			Tally {
				ayes: 250,
				nays: 100,
				turnout: 150
			}
		);
		fast_forward_to(3);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(1), r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(1), 1));
		assert_eq!(Balances::locks(1), vec![the_lock(5)]);
		assert_eq!(Balances::usable_balance(1), 5);
	});
}
