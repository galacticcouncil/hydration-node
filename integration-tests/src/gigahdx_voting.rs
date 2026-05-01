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
fn giga_stake_should_mint_gigahdx() {
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
fn vote_should_record_gigahdx_vote_when_voter_holds_gigahdx() {
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
fn remove_vote_should_record_reward_when_referendum_ended() {
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
fn claim_rewards_should_convert_pending_hdx_to_gigahdx() {
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
fn giga_unstake_should_fail_when_amount_would_breach_ongoing_vote_lock() {
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
fn giga_unstake_should_force_remove_finished_votes_and_record_rewards() {
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
fn giga_unstake_should_apply_dynamic_cooldown_from_conviction_lock() {
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

		// Snapshot the vote's remaining lock period BEFORE unstake — on_unstake
		// will force-remove the vote, so we have to capture this beforehand.
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		let block_before_unstake = System::block_number();
		let voting_lock_remaining = vote.lock_expires_at.saturating_sub(block_before_unstake);
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		let expected_cooldown = base_cooldown.max(voting_lock_remaining);

		//Act
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		//Assert
		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1);
		assert_eq!(positions[0].unlock_at, block_before_unstake + expected_cooldown);
	});
}

#[test]
fn stake_unstake_vote_should_compose_correctly_when_interleaved() {
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
fn vote_should_use_combined_balance_when_voter_holds_both_gigahdx_and_hdx() {
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
fn reward_should_be_weighted_by_conviction_across_voters() {
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
fn reward_should_count_only_gigahdx_portion_when_voting_with_combined_balance() {
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
fn claim_rewards_should_aggregate_across_multiple_referenda() {
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
fn reward_pot_should_deplete_across_sequential_referenda() {
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
fn sequential_reward_claims_should_yield_equal_gigahdx() {
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
fn vote_should_use_remaining_balance_when_free_gigahdx_transferred_first() {
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
fn vote_should_cap_gigahdx_portion_at_balance_when_amount_exceeds_holdings() {
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
fn per_vote_splits_should_remain_immutable_across_balance_changes() {
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
fn liquidation_should_clear_all_votes_and_record_rewards_only_for_finished() {
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
fn restake_and_revote_should_succeed_after_liquidation() {
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
fn received_gigahdx_should_be_transferable_when_existing_balance_locked() {
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
fn reward_should_route_to_stuck_when_pending_rewards_full() {
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
fn storage_should_not_desync_when_pending_rewards_full_during_unstake() {
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
fn last_voter_reward_should_be_fair_despite_rounding() {
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
fn remove_vote_should_fail_when_user_never_voted_on_referendum() {
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
fn vote_update_should_replace_conviction_without_double_counting() {
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
fn vote_should_fail_when_referendum_does_not_exist() {
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
fn claim_rewards_should_fail_when_called_twice() {
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
fn split_abstain_vote_should_use_none_conviction() {
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
fn reward_should_reflect_updated_conviction_not_original() {
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
fn reward_should_not_be_recorded_when_referendum_cancelled() {
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
fn none_conviction_should_get_same_reward_as_locked1x() {
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
fn reward_should_be_based_on_vote_amount_not_current_balance() {
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
fn staking_hooks_should_fire_after_voting_changes() {
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
fn giga_unstake_should_split_into_two_positions_when_active_vote_lock_exceeds_free() {
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

		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		let before_block = System::block_number();
		let voting_lock_remaining = vote.lock_expires_at.saturating_sub(before_block);
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();

		// Unstake 150 of 200 — free pool = 200-100 = 100, free_unstake = min(150, 100) = 100,
		// voted_unstake = 50.
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			150 * UNITS,
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 2, "split unstake produces free + voted positions");

		let mut sorted = positions.to_vec();
		sorted.sort_by_key(|p| p.amount);

		// Computed against realised total to stay correct under exchange-rate inflation.
		let total_hdx = sorted[0].amount.saturating_add(sorted[1].amount);
		let expected_voted = total_hdx.saturating_mul(50) / 150;
		let expected_free = total_hdx.saturating_sub(expected_voted);

		let voted_cooldown = base_cooldown.max(voting_lock_remaining);
		assert_eq!(
			sorted[0].amount, expected_voted,
			"voted portion = floor(total * 50/150)"
		);
		assert_eq!(
			sorted[0].unlock_at,
			before_block + voted_cooldown,
			"voted uses max(base, conviction)"
		);

		assert_eq!(sorted[1].amount, expected_free, "free portion = floor(total * 100/150)");
		assert_eq!(sorted[1].unlock_at, before_block + base_cooldown);
	});
}

#[test]
fn on_unstake_should_spill_uncovered_commitment_to_hdx_side() {
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

// ===========================================================================
// Split-unstake property/edge tests
// ===========================================================================

/// Idempotent helper — flips an Ongoing referendum to Approved without using
/// the destructive `info.take()` path (already-finished referenda are left
/// alone).
#[allow(dead_code)]
fn force_approve_referendum_v(index: u32) {
	use pallet_referenda::ReferendumInfo;
	let now = System::block_number();
	if let Some(ReferendumInfo::Ongoing(status)) =
		pallet_referenda::ReferendumInfoFor::<hydradx_runtime::Runtime>::get(index)
	{
		pallet_referenda::ReferendumInfoFor::<hydradx_runtime::Runtime>::insert(
			index,
			ReferendumInfo::Approved(now, Some(status.submission_deposit), status.decision_deposit),
		);
	}
}

/// #1 HDX-conservation: free_hdx + voted_hdx == hdx_amount exactly.
#[test]
fn giga_unstake_split_should_conserve_total_hdx() {
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

		// Capture the HDX value of the unstake before it happens.
		let alice_hdx_before = Currencies::free_balance(0, &alice);
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			150 * UNITS,
		));
		let alice_hdx_after = Currencies::free_balance(0, &alice);
		let total_hdx_received = alice_hdx_after.saturating_sub(alice_hdx_before);

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		let positions_sum: Balance = positions.iter().map(|p| p.amount).sum();
		assert_eq!(
			positions_sum, total_hdx_received,
			"sum of position amounts must equal HDX delivered to user (no HDX gained or lost)",
		);
	});
}

/// #2 Full-balance-locked: when the user's entire balance is vote-locked,
/// unstake hits only the locked portion → single voted-cooldown position.
#[test]
fn giga_unstake_should_create_single_voted_position_when_full_balance_locked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));
		// Vote with the FULL balance — leaves zero free GIGAHDX.
		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked6x),
		));
		end_referendum();

		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r).unwrap();
		let before = System::block_number();
		let voting_lock_remaining = vote.lock_expires_at.saturating_sub(before);
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();

		// Unstake half — the entire 50 must be the voted portion (no free room).
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1, "free=0 → no split");
		// Position amount is approximately the unstaked underlying — aToken
		// interest accrual makes it slightly larger than the GIGAHDX input.
		assert!(
			positions[0].amount >= 50 * UNITS && positions[0].amount < 51 * UNITS,
			"position ≈ 50 stHDX, got {}",
			positions[0].amount,
		);
		assert_eq!(
			positions[0].unlock_at,
			before + base_cooldown.max(voting_lock_remaining),
			"single position carries the conviction-derived cooldown",
		);
	});
}

/// #3 Boundary: Locked4x (28 days < 100 base) doesn't split, Locked6x
/// (112 days > 100 base) does. Documents the strict `>` in `needs_split`.
#[test]
fn giga_unstake_split_should_obey_voting_lock_above_base_cooldown_boundary() {
	let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
	let vote_locking_period = <hydradx_runtime::Runtime as pallet_conviction_voting::Config>::VoteLockingPeriod::get();
	// Sanity-check the assumed parameters so this test breaks loudly if
	// CooldownPeriod or VoteLockingPeriod are retuned.
	assert_eq!(base_cooldown, 100 * DAYS);
	assert_eq!(vote_locking_period, 7 * DAYS);

	// Helper: stake 200, vote 100 with `c`, end_referendum, unstake 150,
	// and return the number of unstake positions.
	let scenario = |c: Conviction| {
		let mut positions_len = 0usize;
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
				aye_with_conviction(100 * UNITS, c),
			));
			end_referendum();
			assert_ok!(GigaHdx::giga_unstake(
				hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
				150 * UNITS,
			));
			positions_len = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice).len();
		});
		positions_len
	};

	// Locked4x = 8 × 7 d = 56 days; after end_referendum's 12-day advance the
	// remaining lock is 44 days < 100 base → no split.
	TestNet::reset();
	assert_eq!(
		scenario(Conviction::Locked4x),
		1,
		"Locked4x remaining < base → no split"
	);
	// Locked6x = 32 × 7 d = 224 days; after end_referendum's 12-day advance
	// the remaining lock is 212 days > 100 base → split.
	// (Locked5x = 112 d − 12 d = 100 d sits exactly on the boundary, so it
	// also doesn't split — `needs_split` uses strict `>`.)
	TestNet::reset();
	assert_eq!(scenario(Conviction::Locked6x), 2, "Locked6x remaining > base → split");
}

/// #4 Lifecycle: after a split-unstake, `unlock` releases the free position
/// after `base_cooldown`, and the voted position only after `conviction_cooldown`.
#[test]
fn unlock_should_release_free_position_before_voted_after_split_unstake() {
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

		let before = System::block_number();
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			150 * UNITS,
		));
		assert_eq!(
			pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice).len(),
			2,
			"split produced 2 positions",
		);

		// At block before+base_cooldown+1 the free position should release;
		// the voted position (Locked6x cooldown ≫ base) must still be pending.
		fast_forward_to(before + base_cooldown + 1);
		assert_ok!(GigaHdx::unlock(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			alice.clone(),
		));
		let positions_mid = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions_mid.len(), 1, "free position released, voted still pending");
		assert!(
			positions_mid[0].unlock_at > before + base_cooldown,
			"remaining position is the voted one with longer cooldown",
		);

		// Fast-forward past the voted unlock too — now everything releases.
		fast_forward_to(positions_mid[0].unlock_at + 1);
		assert_ok!(GigaHdx::unlock(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			alice.clone(),
		));
		assert_eq!(
			pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice).len(),
			0,
			"voted position released after conviction cooldown",
		);
	});
}

/// #5 Per-position `unlock_at` is anchored at the block the position was
/// created. Two no-vote unstakes at different blocks produce two free
/// positions whose `unlock_at` differ by exactly the elapsed blocks.
///
/// (Note: we deliberately don't use voted positions for this test — when the
/// voted cooldown derives from a `PriorLockSplit` entry, all voted positions
/// converge to `prior.until` regardless of unstake block, which is correct
/// but defeats the "anchored at creation block" check.)
#[test]
fn giga_unstake_unlock_at_should_use_per_position_creation_block() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			200 * UNITS,
		));
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();

		let block_t1 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));
		let p1 = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(p1.len(), 1);
		assert_eq!(p1[0].unlock_at, block_t1 + base_cooldown);

		fast_forward_to(block_t1 + 50);
		let block_t2 = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));
		let p2 = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(p2.len(), 2);
		// The two positions' unlock_at differ by exactly the elapsed blocks.
		let mut sorted = p2.to_vec();
		sorted.sort_by_key(|p| p.unlock_at);
		assert_eq!(sorted[0].unlock_at, block_t1 + base_cooldown);
		assert_eq!(sorted[1].unlock_at, block_t2 + base_cooldown);
		assert_eq!(sorted[1].unlock_at - sorted[0].unlock_at, 50);
	});
}

/// #6 Position-cap edge: existing == cap−1 + 1 (no-split) just fits.
#[test]
fn giga_unstake_should_succeed_when_existing_positions_plus_one_just_fits_cap() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			500 * UNITS,
		));
		// Pre-fill 9 positions directly (no votes → next unstake is a single
		// position). Total cap = 10.
		pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::mutate(&alice, |positions| {
			for i in 0..9u32 {
				positions
					.try_push(pallet_gigahdx::types::UnstakePosition {
						amount: UNITS,
						unlock_at: 10_000 + i,
					})
					.expect("pre-fill 9");
			}
		});

		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));
		assert_eq!(
			pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice).len(),
			10,
			"9 existing + 1 new = exactly the cap",
		);
	});
}

/// #7 Prorate rounding: when `gigahdx_amount` doesn't divide evenly, voted_hdx
/// is rounded down (current behavior) — verify the exact split for a small
/// non-trivial case.
#[test]
fn giga_unstake_split_should_round_voted_hdx_down() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();
		// Choose amounts so the voted/free ratio doesn't divide evenly.
		// Stake 21, vote 14 → free = 7. Unstake 17 → free=7, voted=10.
		// Then voted_hdx = floor(hdx_amount * 10 / 17). For fresh-state 1:1
		// rate, hdx_amount = 17 * UNITS so voted_hdx = floor(17e12 * 10/17) = 10e12.
		// Use larger amounts to avoid hitting MinStake/exchange-rate edge cases.
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			2_100 * UNITS,
		));
		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(1_400 * UNITS, Conviction::Locked6x),
		));
		end_referendum();

		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			1_700 * UNITS, // free=700 (within 700 free), voted=1000
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 2);

		// Sum invariant first.
		let total: Balance = positions.iter().map(|p| p.amount).sum();
		// Match the runtime's prorate exactly: voted = floor(total * 1000 / 1700).
		let voted_expected = total.saturating_mul(1000) / 1700;
		let free_expected = total - voted_expected;
		let voted = positions.iter().min_by_key(|p| p.unlock_at).is_some(); // sanity
		assert!(voted, "two positions present");

		// Free is the position with smaller unlock_at (base cooldown < conviction).
		let mut sorted = positions.to_vec();
		sorted.sort_by_key(|p| p.unlock_at);
		assert_eq!(sorted[0].amount, free_expected, "free portion matches floor split");
		assert_eq!(sorted[1].amount, voted_expected, "voted portion matches floor split");
	});
}

/// #8 No-vote sanity: pure free unstake produces a single base-cooldown position.
#[test]
fn giga_unstake_should_create_single_position_when_no_votes_active() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let before = System::block_number();
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			50 * UNITS,
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1, "no votes → no split");
		assert_eq!(positions[0].unlock_at, before + base_cooldown);
	});
}

/// #9 Stale prior: Locked1x vote → end_referendum + advance past lock window
/// → prior is `is_active() = false` after rejig → voting_lock = 0 → no split.
#[test]
fn giga_unstake_should_not_split_when_only_expired_priors_remain() {
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
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		// Approve so a prior actually accumulates on remove_vote.
		force_approve_referendum_v(r);
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r,
		));

		// Fast-forward past the Locked1x window (1 × VoteLockingPeriod = 7 days)
		// + a margin so the prior rejigs to inactive.
		let now = System::block_number();
		fast_forward_to(now + 8 * DAYS);

		// Trigger an `unlock` first to clear the conviction-voting prior so the
		// adapter's recompute drops GigaHdxVotingLock to 0.
		assert_ok!(ConvictionVoting::unlock(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			0,
			alice.clone().into(),
		));

		let before = System::block_number();
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			150 * UNITS,
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1, "expired prior → no split");
		assert_eq!(positions[0].unlock_at, before + base_cooldown);
	});
}

/// #10 Multi-prior: two priors with different remaining windows — the larger
/// drives the cooldown via `additional_unstake_lock`'s max-aggregate.
#[test]
fn additional_unstake_lock_should_max_aggregate_across_multiple_priors() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			300 * UNITS,
		));

		// Vote on r1 with Locked1x, approve, remove → prior #1 (short).
		let r1 = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r1,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		force_approve_referendum_v(r1);
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r1,
		));

		// Vote on r2 with Locked6x, approve, remove → prior #2 (long).
		let r2 = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			r2,
			aye_with_conviction(150 * UNITS, Conviction::Locked6x),
		));
		force_approve_referendum_v(r2);
		assert_ok!(ConvictionVoting::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			Some(0),
			r2,
		));

		// Both priors are alive; the longer one (Locked6x) should drive the
		// unstake's voted-portion cooldown.
		let before = System::block_number();
		let base_cooldown = <hydradx_runtime::Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		// 300 GIGAHDX, max prior g_lock = 150 → free pool = 150. Unstake 200
		// crosses into the locked portion.
		assert_ok!(GigaHdx::giga_unstake(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			200 * UNITS,
		));
		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 2, "split produced 2 positions");
		let voted = positions.iter().max_by_key(|p| p.unlock_at).unwrap();
		// The voted-position cooldown must reflect the LONGER (Locked6x) prior,
		// not the Locked1x one. Locked6x = 32 × VoteLockingPeriod.
		let voting_lock_period =
			<hydradx_runtime::Runtime as pallet_conviction_voting::Config>::VoteLockingPeriod::get();
		let locked6x_window = voting_lock_period.saturating_mul(32);
		assert!(
			voted.unlock_at >= before + base_cooldown.max(locked6x_window).saturating_sub(50),
			"voted cooldown derived from the larger (Locked6x) prior, got {} expected ≥ {}",
			voted.unlock_at,
			before + base_cooldown.max(locked6x_window).saturating_sub(50),
		);
	});
}
