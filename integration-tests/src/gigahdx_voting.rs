#![cfg(test)]

use crate::gigahdx::PATH_TO_SNAPSHOT;
use crate::polkadot_test_net::*;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchResult,
	traits::{schedule::DispatchTime, Bounded, OnInitialize, StorePreimage},
};
use frame_system::RawOrigin;
use hydradx_runtime::{
	Balances, BlockNumber, ConvictionVoting, Currencies, Democracy, GigaHdx, GigaHdxVoting, Preimage, Referenda,
	Scheduler, System, Tokens,
};
use orml_traits::currency::MultiCurrency;
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use pallet_referenda::ReferendumIndex;
use primitives::constants::time::DAYS;
use primitives::AccountId;
use sp_runtime::FixedPointNumber;
use xcm_emulator::TestExt;

type CallOf<T> = <T as frame_system::Config>::RuntimeCall;
type BoundedCallOf<T> = Bounded<CallOf<T>, <T as frame_system::Config>::Hashing>;

fn set_balance_proposal(who: AccountId, value: u128) -> BoundedCallOf<hydradx_runtime::Runtime> {
	let inner = pallet_balances::Call::force_set_balance { who, new_free: value };
	let outer = hydradx_runtime::RuntimeCall::Balances(inner);
	Preimage::bound(outer).unwrap()
}

fn propose_set_balance(who: AccountId, dest: AccountId, value: u128, dispatch_time: BlockNumber) -> DispatchResult {
	Referenda::submit(
		hydradx_runtime::RuntimeOrigin::signed(who),
		Box::new(RawOrigin::Root.into()),
		set_balance_proposal(dest, value),
		DispatchTime::At(dispatch_time),
	)
}

fn begin_referendum() -> ReferendumIndex {
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<hydradx_runtime::Runtime>::get();
	let now = System::block_number();

	assert_ok!(propose_set_balance(ALICE.into(), CHARLIE.into(), 2, now + 10 * DAYS));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		2_000_000_000 * UNITS,
	));

	assert_ok!(Referenda::place_decision_deposit(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		referendum_index
	));

	fast_forward_to(now + 5 * DAYS);

	referendum_index
}

fn end_referendum() {
	let now = System::block_number();
	fast_forward_to(now + 12 * DAYS);
}

fn begin_referendum_by_bob() -> ReferendumIndex {
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<hydradx_runtime::Runtime>::get();
	let now = System::block_number();

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		BOB.into(),
		1_000_000 * UNITS
	));
	let proposal = {
		let inner = pallet_balances::Call::force_set_balance {
			who: CHARLIE.into(),
			new_free: 2,
		};
		let outer = hydradx_runtime::RuntimeCall::Balances(inner);
		Preimage::bound(outer).unwrap()
	};
	assert_ok!(Referenda::submit(
		hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
		Box::new(RawOrigin::Root.into()),
		proposal,
		DispatchTime::At(now + 10 * DAYS),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		2_000_000_000 * UNITS
	));
	assert_ok!(Referenda::place_decision_deposit(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		referendum_index,
	));

	fast_forward_to(now + 5 * DAYS);

	referendum_index
}

fn fast_forward_to(n: u32) {
	while System::block_number() < n {
		next_block();
	}
}

fn next_block() {
	System::set_block_number(System::block_number() + 1);
	Scheduler::on_initialize(System::block_number());
	Democracy::on_initialize(System::block_number());
}

fn aye(amount: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote {
			aye: true,
			conviction: Conviction::None,
		},
		balance: amount,
	}
}

fn aye_with_conviction(amount: u128, conviction: Conviction) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: true, conviction },
		balance: amount,
	}
}

/// Set up the GIGAHDX system: fund holding accounts, gigapot, reward pot.
fn init_gigahdx() {
	let gigapot = pallet_gigahdx::Pallet::<hydradx_runtime::Runtime>::gigapot_account_id();
	let reward_pot = pallet_gigahdx_voting::Pallet::<hydradx_runtime::Runtime>::giga_reward_pot_account();

	// Fund holding accounts with ED so they exist.
	assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), gigapot, UNITS));
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		reward_pot,
		100_000 * UNITS,
	));

	// Give ALICE and BOB plenty of HDX.
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		ALICE.into(),
		1_000_000 * UNITS,
	));
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		BOB.into(),
		1_000_000 * UNITS,
	));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn giga_stake_produces_gigahdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let gigapot = pallet_gigahdx::Pallet::<hydradx_runtime::Runtime>::gigapot_account_id();

		let alice_hdx_before = Currencies::free_balance(HDX, &alice);

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// ALICE should have received GIGAHDX.
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(gigahdx_bal, 100 * UNITS);

		// HDX should have moved to gigapot.
		let gigapot_hdx = Currencies::free_balance(HDX, &gigapot);
		assert!(gigapot_hdx >= 100 * UNITS); // at least the staked amount (plus ED)

		// ALICE's HDX should have decreased.
		let alice_hdx_after = Currencies::free_balance(HDX, &alice);
		assert_eq!(alice_hdx_before - alice_hdx_after, 100 * UNITS);
	});
}

#[test]
fn vote_with_gigahdx_records_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		// Stake to get GIGAHDX.
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		// Vote with an amount within GIGAHDX balance.
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye(50 * UNITS),
		));

		// GigaHdxVotes should be populated.
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r);
		assert!(vote.is_some());
		let vote = vote.unwrap();
		assert_eq!(vote.amount, 50 * UNITS);

		// ReferendumTracks cache should be populated.
		let cached_track = pallet_gigahdx_voting::ReferendumTracks::<hydradx_runtime::Runtime>::get(r);
		assert!(cached_track.is_some());
		assert_eq!(cached_track.unwrap(), 0); // root track
	});
}

#[test]
fn end_referendum_remove_vote_records_reward() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye(50 * UNITS),
		));

		// End the referendum (it should pass).
		end_referendum();

		// Remove vote after referendum completed.
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0), // root track
			r,
		));

		// PendingRewards should have an entry for ALICE.
		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(!pending.is_empty());
		assert_eq!(pending[0].referenda_id, r);
		assert!(pending[0].reward_amount > 0);
	});
}

#[test]
fn claim_rewards_converts_to_gigahdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye(50 * UNITS),
		));

		end_referendum();

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));

		let gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);

		// Claim rewards.
		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			alice.clone()
		),));

		let gigahdx_after = Currencies::free_balance(GIGAHDX, &alice);
		assert!(gigahdx_after > gigahdx_before);

		// PendingRewards should be empty.
		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(pending.is_empty());
	});
}

#[test]
fn giga_unstake_blocked_during_ongoing_when_would_breach_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye(50 * UNITS),
		));

		// Full unstake (100) would dip the balance below the 50-UNITS ongoing
		// vote lock — must be rejected.
		assert_noop!(
			GigaHdx::giga_unstake(hydradx_runtime::RuntimeOrigin::signed(alice.clone()), 100 * UNITS,),
			pallet_gigahdx::Error::<hydradx_runtime::Runtime>::ActiveVotesInOngoingReferenda,
		);

		// Partial unstake within the free portion (50 free, 50 locked) succeeds
		// even with the ongoing vote — only the over-the-lock portion is gated.
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));
	});
}

#[test]
fn giga_unstake_force_removes_finished_votes_and_records_rewards() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		//Act
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		//Assert
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r);
		assert!(vote.is_none(), "Vote should be force-removed by unstake");

		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(!pending.is_empty(), "Rewards should be recorded during force-removal");
		assert!(pending[0].reward_amount > 0);

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1);
		assert!(positions[0].amount > 0);

		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			alice.clone()
		),));

		let pending_after = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(pending_after.is_empty());
	});
}

#[test]
fn giga_unstake_applies_dynamic_cooldown_from_conviction_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked6x),
		));
		end_referendum();

		let block_before_unstake = System::block_number();

		//Act
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		//Assert
		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1);
		assert_eq!(positions[0].unlock_at, block_before_unstake + 222 * DAYS);
	});
}

#[test]
fn interleaved_stake_unstake_vote_operations() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 100 * UNITS);

		//Act - partial unstake
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			40 * UNITS,
		));

		//Assert
		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 60 * UNITS);

		//Act - stake more
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		//Assert
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 159_009_900_990_099);

		//Act - vote with new balance, then unstake after referendum
		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();

		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		//Assert
		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 2);

		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(!pending.is_empty());

		assert!(Currencies::free_balance(GIGAHDX, &alice) > 0);
	});
}

#[test]
fn combined_voting_power() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		// Stake 50 HDX → get 50 GIGAHDX.
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		let r = begin_referendum();

		// Vote with 200 UNITS — exceeds GIGAHDX balance (50), so 50 GIGAHDX + 150 HDX.
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye(200 * UNITS),
		));

		// GigaHdxVotes records the combined amount (200) and per-side snapshot
		// (gigahdx_lock=50, hdx_lock=150).
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		assert_eq!(vote.amount, 200 * UNITS, "combined committed amount");
		assert_eq!(vote.gigahdx_lock, 50 * UNITS);
		assert_eq!(vote.hdx_lock, 150 * UNITS);

		// Effective lock split mirrors the per-vote snapshot.
		let split = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(split.gigahdx_amount, 50 * UNITS);
		assert_eq!(split.hdx_amount, 150 * UNITS);
	});
}

#[test]
fn conviction_weighted_rewards() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		// Both stake 100 GIGAHDX.
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		// ALICE votes with Conviction::None (multiplier 1).
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::None),
		));

		// BOB votes with Conviction::Locked6x (multiplier 6).
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked6x),
		));

		end_referendum();

		// Remove both votes (triggering reward allocation and recording).
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			Some(0),
			r,
		));

		let alice_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		let bob_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&bob);

		assert!(!alice_pending.is_empty());
		assert!(!bob_pending.is_empty());

		// BOB should get ~6x ALICE's reward (weighted by conviction).
		// Total weighted: 100*1 + 100*6 = 700
		// ALICE share: 100/700, BOB share: 600/700
		let alice_reward = alice_pending[0].reward_amount;
		let bob_reward = bob_pending[0].reward_amount;
		assert!(bob_reward > alice_reward);
		// BOB should get roughly 6x (allow for rounding).
		assert!(bob_reward >= alice_reward * 5);
	});
}

#[test]
fn rewards_only_for_gigahdx_portion_when_voting_with_combined_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let alice_gigahdx = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(alice_gigahdx, 100 * UNITS, "First staker should get 1:1");

		//Act
		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(500 * UNITS, Conviction::Locked1x),
		));

		//Assert
		let alice_vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		assert_eq!(alice_vote.amount, 500 * UNITS, "Vote records combined balance (500)");
		assert_eq!(
			alice_vote.gigahdx_lock,
			100 * UNITS,
			"Only the GIGAHDX portion (100) is the reward-bearing side"
		);
		assert_eq!(alice_vote.hdx_lock, 400 * UNITS, "Remainder (400) goes to the HDX side");

		let split = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(split.gigahdx_amount, 100 * UNITS);
		assert_eq!(split.hdx_amount, 400 * UNITS);

		let total_weighted = pallet_gigahdx_voting::ReferendaTotalWeightedVotes::<hydradx_runtime::Runtime>::get(r);
		assert_eq!(
			total_weighted,
			100 * UNITS,
			"Total weighted votes should be based on GIGAHDX only (100), not total vote (500)"
		);

		//Act
		end_referendum();
		// Triggers reward allocation and recording.
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0), // root track
			r,
		));

		//Assert
		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(!pending.is_empty(), "ALICE should have pending rewards");
		assert!(pending[0].reward_amount > 0);

		let gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			alice.clone()
		),));

		let gigahdx_after = Currencies::free_balance(GIGAHDX, &alice);
		assert!(
			gigahdx_after > gigahdx_before,
			"ALICE should receive GIGAHDX from claiming rewards"
		);

		let pending_after = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(pending_after.is_empty());
	});
}

#[test]
fn multiple_referenda_rewards_claimed_at_once() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 100 * UNITS);

		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(50 * UNITS, Conviction::Locked6x),
		));

		let vote_a = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_a).unwrap();
		let vote_b = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_b).unwrap();
		assert_eq!(vote_a.amount, 50 * UNITS);
		assert_eq!(vote_b.amount, 50 * UNITS);

		let weighted_a = pallet_gigahdx_voting::ReferendaTotalWeightedVotes::<hydradx_runtime::Runtime>::get(r_a);
		let weighted_b = pallet_gigahdx_voting::ReferendaTotalWeightedVotes::<hydradx_runtime::Runtime>::get(r_b);
		assert_eq!(weighted_a, 50 * UNITS);
		assert_eq!(weighted_b, 300 * UNITS);

		//Act
		end_referendum();

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r_a,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r_b,
		));

		//Assert
		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(
			pending.len(),
			2,
			"Should have 2 pending reward entries, got {}",
			pending.len()
		);

		let reward_a = pending
			.iter()
			.find(|e| e.referenda_id == r_a)
			.expect("Should have reward for referendum A");
		let reward_b = pending
			.iter()
			.find(|e| e.referenda_id == r_b)
			.expect("Should have reward for referendum B");
		assert!(reward_a.reward_amount > 0);
		assert!(reward_b.reward_amount > 0);
		let total_reward_hdx = reward_a.reward_amount + reward_b.reward_amount;

		let rate_before = GigaHdx::exchange_rate();
		let gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			alice.clone()
		),));

		let gigahdx_gained = Currencies::free_balance(GIGAHDX, &alice) - gigahdx_before;
		let expected_gigahdx = rate_before.reciprocal().unwrap().saturating_mul_int(total_reward_hdx);
		assert_eq!(
			gigahdx_gained, expected_gigahdx,
			"GIGAHDX gained ({}) should match expected ({}) from total HDX reward ({}) at rate ({})",
			gigahdx_gained, expected_gigahdx, total_reward_hdx, rate_before
		);

		let pending_after = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(pending_after.is_empty());
	});
}

#[test]
fn reward_pot_depletes_across_sequential_referenda() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let reward_pot = pallet_gigahdx_voting::Pallet::<hydradx_runtime::Runtime>::giga_reward_pot_account();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// Skip referendum index 0 because into_sub_account_truncating(0)
		// collides with into_account_truncating() (same AccountId).
		// On mainnet this is not an issue since referenda index > 0 already.
		let _ = begin_referendum();
		end_referendum();

		let pot_balance_initial = Currencies::free_balance(HDX, &reward_pot);
		assert_eq!(pot_balance_initial, 100_000 * UNITS);

		//Act
		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r_a,
		));

		//Assert
		let pool_a = pallet_gigahdx_voting::ReferendaRewardPool::<hydradx_runtime::Runtime>::get(r_a).unwrap();
		assert_eq!(pool_a.total_reward, 10_000 * UNITS);
		assert_eq!(Currencies::free_balance(HDX, &reward_pot), 90_000 * UNITS);

		//Act
		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r_b,
		));

		//Assert
		let pool_b = pallet_gigahdx_voting::ReferendaRewardPool::<hydradx_runtime::Runtime>::get(r_b).unwrap();
		assert_eq!(pool_b.total_reward, 9_000 * UNITS);
		assert_eq!(Currencies::free_balance(HDX, &reward_pot), 81_000 * UNITS);
		assert!(pool_b.total_reward < pool_a.total_reward);
	});
}

#[test]
fn sequential_reward_claims_give_equal_gigahdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			Some(0),
			r,
		));

		let alice_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		let bob_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&bob);
		let alice_reward_hdx = alice_pending[0].reward_amount;
		let bob_reward_hdx = bob_pending[0].reward_amount;

		//Act - ALICE claims first
		let rate_before_alice = GigaHdx::exchange_rate();
		let alice_gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);
		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			alice.clone()
		),));
		let alice_gigahdx_gained = Currencies::free_balance(GIGAHDX, &alice) - alice_gigahdx_before;

		//Act - BOB claims second
		let rate_before_bob = GigaHdx::exchange_rate();
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);
		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			bob.clone()
		),));
		let bob_gigahdx_gained = Currencies::free_balance(GIGAHDX, &bob) - bob_gigahdx_before;

		//Assert
		let alice_conversion_rate = alice_gigahdx_gained * UNITS / alice_reward_hdx;
		let bob_conversion_rate = bob_gigahdx_gained * UNITS / bob_reward_hdx;
		assert_eq!(alice_conversion_rate, 990_099_009_900);
		assert_eq!(bob_conversion_rate, 990_099_009_900);
		assert_eq!(alice_conversion_rate, bob_conversion_rate)
	});
}

#[test]
fn vote_after_transferring_free_gigahdx_uses_correct_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			1_000 * UNITS,
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 1_000 * UNITS);

		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(500 * UNITS, Conviction::Locked1x),
		));

		//Act
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			bob.clone(),
			GIGAHDX,
			300 * UNITS,
		));

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 700 * UNITS);

		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(600 * UNITS, Conviction::Locked1x),
		));

		//Assert
		let vote_b = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_b).unwrap();
		assert_eq!(
			vote_b.amount,
			600 * UNITS,
			"GIGAHDX portion should be capped at current balance (700), vote is 600 so all from GIGAHDX"
		);

		let split = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(split.gigahdx_amount, 600 * UNITS);
		assert_eq!(split.hdx_amount, 0);
	});
}

#[test]
fn vote_with_more_than_balance_is_capped_at_gigahdx_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let gigapot = pallet_gigahdx::Pallet::<hydradx_runtime::Runtime>::gigapot_account_id();

		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), gigapot, UNITS));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000 * UNITS
		));

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			1_000 * UNITS,
		));

		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			bob.clone(),
			GIGAHDX,
			300 * UNITS,
		));

		assert_eq!(Currencies::free_balance(HDX, &alice), 0);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 700 * UNITS);

		let r = begin_referendum_by_bob();

		//Act & Assert
		assert_noop!(
			ConvictionVoting::vote(
				hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
				r,
				aye_with_conviction(701 * UNITS, Conviction::Locked1x),
			),
			pallet_conviction_voting::Error::<hydradx_runtime::Runtime>::InsufficientFunds
		);

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(700 * UNITS, Conviction::Locked1x),
		));

		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		assert_eq!(vote.amount, 700 * UNITS);
	});
}

/// Per-vote splits are immutable snapshots. A balance increase between votes does
/// NOT reshape an earlier vote's split — the effective lock is the per-side
/// max-aggregate across all active votes.
///
/// 1. ALICE stakes 500 HDX -> 500 GIGAHDX. Votes 800 on r_a -> snapshot (500 G, 300 H).
/// 2. ALICE stakes 200 more -> 700 GIGAHDX. Vote A's snapshot is unchanged.
/// 3. Votes 800 on r_b -> snapshot (700 G, 100 H) using the new balance.
/// 4. Effective LockSplit = max-per-side across both votes = (700 G, 300 H).
#[test]
fn per_vote_splits_are_immutable_snapshots_across_balance_changes() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			500 * UNITS,
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 500 * UNITS);

		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));

		// Vote A snapshot at cast time: g=500, h=300.
		let vote_a = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_a).unwrap();
		assert_eq!(vote_a.gigahdx_lock, 500 * UNITS);
		assert_eq!(vote_a.hdx_lock, 300 * UNITS);

		let split_a = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(split_a.gigahdx_amount, 500 * UNITS);
		assert_eq!(split_a.hdx_amount, 300 * UNITS);

		//Act — stake more GIGAHDX. Vote A's snapshot must be unchanged.
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			200 * UNITS,
		));

		let vote_a_after = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_a).unwrap();
		assert_eq!(vote_a_after.gigahdx_lock, 500 * UNITS, "vote A snapshot is immutable");
		assert_eq!(vote_a_after.hdx_lock, 300 * UNITS, "vote A snapshot is immutable");

		// Vote B uses the new balance for its own snapshot.
		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));

		let alice_gigahdx = Currencies::free_balance(GIGAHDX, &alice);
		let vote_b = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_b).unwrap();
		assert_eq!(
			vote_b.gigahdx_lock, alice_gigahdx,
			"vote B snapshot uses balance at its cast time"
		);
		assert_eq!(vote_b.hdx_lock, 800 * UNITS - alice_gigahdx);

		//Assert — effective LockSplit is the per-side max across both vote snapshots.
		let split = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(
			split.gigahdx_amount, vote_b.gigahdx_lock,
			"G-side cap = max(vote A=500, vote B=alice_gigahdx) = vote B"
		);
		assert_eq!(
			split.hdx_amount, vote_a.hdx_lock,
			"H-side cap = max(vote A=300, vote B=800-alice_gigahdx) = vote A"
		);
	});
}

#[test]
fn liquidation_clears_all_votes_and_records_rewards_only_for_finished() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();

		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		assert!(pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_a).is_some());
		assert!(pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_b).is_some());

		//Act
		assert_ok!(GigaHdxVoting::prepare_for_liquidation(&alice));

		//Assert
		assert!(pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_a).is_none());
		assert!(pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_b).is_none());

		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(
			pending.iter().any(|e| e.referenda_id == r_a),
			"Should have reward for finished referendum A"
		);
		assert!(
			!pending.iter().any(|e| e.referenda_id == r_b),
			"Should NOT have reward for ongoing referendum B"
		);
	});
}

#[test]
fn restake_and_revote_works_after_liquidation() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		assert_ok!(GigaHdxVoting::prepare_for_liquidation(&alice));

		assert!(pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_a).is_none());

		//Act
		end_referendum();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		//Assert
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r_b).unwrap();
		assert_eq!(vote.amount, 50 * UNITS);

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 199_009_900_990_099);
	});
}

#[test]
fn received_gigahdx_is_transferable_while_existing_balance_is_locked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			200 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(lock, 100 * UNITS);

		//Act
		let bob_gigahdx = Currencies::free_balance(GIGAHDX, &bob);
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			alice.clone(),
			GIGAHDX,
			bob_gigahdx,
		));

		//Assert
		let alice_gigahdx = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(alice_gigahdx, 100 * UNITS + bob_gigahdx);

		let lock_after = pallet_gigahdx_voting::GigaHdxVotingLock::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(lock_after, 100 * UNITS);

		let free_gigahdx = alice_gigahdx - lock_after;
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			charlie.clone(),
			GIGAHDX,
			free_gigahdx,
		));

		assert!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			charlie.clone(),
			GIGAHDX,
			1 * UNITS,
		)
		.is_err());
	});
}

//  When PendingRewards is full, removing a vote routes the reward to StuckRewards
/// (dead-letter queue) instead of silently dropping it.
#[test]
fn reward_routed_to_stuck_when_pending_rewards_full() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// Pre-fill PendingRewards to MaxVotes (25) entries.
		pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::mutate(&alice, |entries| {
			for i in 0..25u32 {
				let _ = entries.try_push(pallet_gigahdx_voting::types::PendingRewardEntry {
					referenda_id: 9000 + i,
					reward_amount: 1 * UNITS,
				});
			}
		});
		assert_eq!(
			pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice).len(),
			25
		);

		// Skip referendum index 0 (sub-account collision)
		let _ = begin_referendum();
		end_referendum();

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();

		//Act: first remove_vote attempt — reward slot full, vote must be preserved.
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));

		//Assert — vote cleared, reward routed to StuckRewards (not lost).
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r);
		assert!(vote.is_none(), "Vote should be removed");

		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(pending.len(), 25, "PendingRewards still full");
		assert!(
			!pending.iter().any(|e| e.referenda_id == r),
			"Overflow reward should not be in PendingRewards"
		);

		let stuck = pallet_gigahdx_voting::StuckRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(stuck.len(), 1, "Overflow reward should be in StuckRewards");
		assert_eq!(stuck[0].referenda_id, r, "StuckRewards entry matches the referendum");
	});
}

#[test]
fn pending_rewards_full_during_unstake_does_not_desync() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// Fill PendingRewards near cap (25 entries).
		pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::mutate(&alice, |entries| {
			for i in 0..25u32 {
				let _ = entries.try_push(pallet_gigahdx_voting::types::PendingRewardEntry {
					referenda_id: 9000 + i,
					reward_amount: 1 * UNITS,
				});
			}
		});

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();

		// Unstake — on_unstake force-removes the finished vote. PendingRewards is full,
		// so the reward goes to StuckRewards. Storage must not desync.
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// Invariant: GigaHdxVotes entry is gone (mirrors conviction-voting state).
		assert!(
			pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).is_none(),
			"GigaHdxVotes must be cleared unconditionally"
		);

		// Any stranded rewards are recoverable via drain. The 25 pre-seeded entries
		// are fake (no backing pots), so we can't use `claim_rewards` here — clear
		// `PendingRewards` directly to make room, then verify drain promotes stuck.
		let stuck = pallet_gigahdx_voting::StuckRewards::<hydradx_runtime::Runtime>::get(&alice);
		if !stuck.is_empty() {
			pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::remove(&alice);
			assert_ok!(GigaHdxVoting::drain_stuck_rewards(
				hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
				alice.clone(),
			));
			assert!(
				pallet_gigahdx_voting::StuckRewards::<hydradx_runtime::Runtime>::get(&alice).is_empty(),
				"StuckRewards drained"
			);
		}
	});
}

#[test]
fn last_voter_reward_is_fair_despite_rounding() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			1_000_000 * UNITS
		));

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			100 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(charlie.clone()),
			100 * UNITS
		));

		// Skip referendum index 0 (sub-account collision)
		let _ = begin_referendum();
		end_referendum();

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(charlie.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		//Act
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			Some(0),
			r
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(charlie.clone()),
			Some(0),
			r
		));

		//Assert
		let alice_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		let bob_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&bob);
		let charlie_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&charlie);

		let alice_reward = alice_pending[0].reward_amount;
		let bob_reward = bob_pending[0].reward_amount;
		let charlie_reward = charlie_pending[0].reward_amount;

		let pool = pallet_gigahdx_voting::ReferendaRewardPool::<hydradx_runtime::Runtime>::get(r).unwrap();
		let total_distributed = alice_reward + bob_reward + charlie_reward;

		assert_eq!(alice_reward, bob_reward);
		assert_eq!(bob_reward, charlie_reward);
		assert!(total_distributed <= pool.total_reward);
		assert_eq!(pool.remaining_reward, pool.total_reward - total_distributed);
		assert_eq!(pool.remaining_reward, 1);
	});
}

#[test]
fn remove_vote_should_fail_for_referendum_user_never_voted_on() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		//Act & Assert
		assert_noop!(
			ConvictionVoting::remove_vote(hydradx_runtime::RuntimeOrigin::signed(alice.clone()), Some(0), r,),
			pallet_conviction_voting::Error::<hydradx_runtime::Runtime>::NotVoter
		);
	});
}

#[test]
fn vote_update_on_same_referendum_replaces_conviction_without_double_counting() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		let vote_1 = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		assert_eq!(vote_1.amount, 50 * UNITS);
		let weighted_1 = pallet_gigahdx_voting::ReferendaTotalWeightedVotes::<hydradx_runtime::Runtime>::get(r);
		assert_eq!(weighted_1, 50 * UNITS);

		//Act
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked6x),
		));

		//Assert
		let vote_2 = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		assert_eq!(vote_2.amount, 50 * UNITS);

		let weighted_2 = pallet_gigahdx_voting::ReferendaTotalWeightedVotes::<hydradx_runtime::Runtime>::get(r);
		assert_eq!(weighted_2, 300 * UNITS);
	});
}

#[test]
fn vote_on_nonexistent_referendum_fails() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		//Act & Assert
		assert!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			99999,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		)
		.is_err());
	});
}

#[test]
fn double_claim_rewards_fails() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));

		assert_ok!(GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(
			alice.clone()
		),));

		//Act & Assert
		assert_noop!(
			GigaHdxVoting::claim_rewards(hydradx_runtime::RuntimeOrigin::signed(alice.clone()),),
			pallet_gigahdx_voting::Error::<hydradx_runtime::Runtime>::NoPendingRewards
		);
	});
}

#[test]
fn split_abstain_vote_uses_none_conviction() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();

		//Act
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			AccountVote::SplitAbstain {
				aye: 30 * UNITS,
				nay: 20 * UNITS,
				abstain: 10 * UNITS,
			},
		));

		//Assert
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		assert_eq!(vote.amount, 60 * UNITS);
		assert_eq!(vote.conviction, pallet_gigahdx_voting::types::Conviction::None);

		let weighted = pallet_gigahdx_voting::ReferendaTotalWeightedVotes::<hydradx_runtime::Runtime>::get(r);
		assert_eq!(weighted, 6 * UNITS);
	});
}

/// ALICE votes Locked1x, then changes to Locked6x on the same referendum.
/// BOB votes Locked1x. After referendum ends, rewards should reflect
/// ALICE's final conviction (6x), not her original (1x).
#[test]
fn reward_reflects_updated_conviction_not_original() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			100 * UNITS,
		));

		// Skip referendum index 0 (sub-account collision)
		let _ = begin_referendum();
		end_referendum();

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		//Act
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked6x),
		));

		end_referendum();

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			Some(0),
			r,
		));

		//Assert
		let alice_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		let bob_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&bob);

		let alice_reward = alice_pending[0].reward_amount;
		let bob_reward = bob_pending[0].reward_amount;

		assert!(
			alice_reward > bob_reward * 5,
			"ALICE (6x conviction) should get ~6x more reward than BOB (1x). Alice: {}, Bob: {}",
			alice_reward,
			bob_reward
		);
	});
}

#[test]
fn no_reward_for_cancelled_referendum() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		//Act
		assert_ok!(Referenda::cancel(RawOrigin::Root.into(), r));

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));

		//Assert
		let pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		assert!(pending.is_empty(), "Cancelled referenda should not generate rewards");
	});
}

/// BUG: Spec says Conviction::None should have 0.1x reward multiplier,
/// but code uses 1x (same as Locked1x). This test documents the discrepancy.
#[test]
fn none_conviction_gets_same_reward_as_locked1x() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			100 * UNITS,
		));

		// Skip referendum index 0
		let _ = begin_referendum();
		end_referendum();

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::None),
		));
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			Some(0),
			r,
		));

		//Assert
		let alice_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		let bob_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&bob);
		let alice_reward = alice_pending[0].reward_amount;
		let bob_reward = bob_pending[0].reward_amount;

		// Per spec: None = 0.1x, Locked1x = 1x. Total weighted = 50*0.1 + 50*1 = 55.
		// Alice should get 5/55 of pool, Bob should get 50/55.
		// Currently both get 5_000_000_000_000_000 (equal) because code uses None = 1x.
		assert_eq!(alice_reward, 909_090_909_090_909);
		assert_eq!(bob_reward, 9_090_909_090_909_090);
	});
}

/// User stakes more GIGAHDX AFTER voting but BEFORE removing vote.
/// The reward should be based on the vote amount at time of voting, not current balance.
#[test]
fn reward_based_on_vote_amount_not_current_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			100 * UNITS,
		));

		// Skip referendum index 0
		let _ = begin_referendum();
		end_referendum();

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(50 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		//Act - ALICE stakes MORE before removing vote
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			500 * UNITS,
		));

		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(bob.clone()),
			Some(0),
			r,
		));

		//Assert
		let alice_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&alice);
		let bob_pending = pallet_gigahdx_voting::PendingRewards::<hydradx_runtime::Runtime>::get(&bob);
		let alice_reward = alice_pending[0].reward_amount;
		let bob_reward = bob_pending[0].reward_amount;

		assert_eq!(
			alice_reward, bob_reward,
			"Rewards should be equal - ALICE's extra stake after voting should not affect reward"
		);
	});
}

#[test]
fn staking_hooks_still_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		// Initialize old staking.
		assert_ok!(hydradx_runtime::Staking::initialize_staking(RawOrigin::Root.into()));
		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			(10_000 * UNITS) as i128,
		));

		let alice: AccountId = ALICE.into();

		// Old-style HDX stake.
		assert_ok!(hydradx_runtime::Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			1_000 * UNITS,
		));

		let r = begin_referendum();

		// Vote — should trigger both StakingConvictionVoting and GigaHdxVotingHooks.
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye(500 * UNITS),
		));

		// Verify old staking hooks recorded the vote.
		let position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap()
		.unwrap();
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(position_id);
		assert!(!stake_voting.votes.is_empty());
	});
}

#[test]
fn unstake_with_voting_lock_creates_one_position_with_max_cooldown() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			200 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked6x),
		));
		end_referendum();

		let before_block = System::block_number();
		let base_cooldown = 222 * DAYS;
		let locked6x_period =
			<hydradx_runtime::Runtime as pallet_conviction_voting::Config>::VoteLockingPeriod::get().saturating_mul(6);
		let expected_cooldown = base_cooldown.max(locked6x_period);

		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			150 * UNITS,
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1, "design: single position with max cooldown");
		assert_eq!(positions[0].unlock_at, before_block + expected_cooldown);
	});
}

#[test]
fn on_post_unstake_sees_final_hdx_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			200 * UNITS,
		));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(150 * UNITS, Conviction::Locked6x),
		));
		end_referendum();

		// Partial unstake — after this, remaining GIGAHDX = 50, voting commitment = 150,
		// so the spillover to HDX must be 100.
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			150 * UNITS,
		));

		let split = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(split.gigahdx_amount, 50 * UNITS, "tracker capped at remaining GIGAHDX");
		assert_eq!(split.hdx_amount, 100 * UNITS, "spillover = commitment - remaining");
	});
}
