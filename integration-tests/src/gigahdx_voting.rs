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
fn giga_unstake_blocked_during_ongoing() {
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

		// Unstake should be blocked while referendum is ongoing.
		assert_noop!(
			GigaHdx::giga_unstake(hydradx_runtime::RuntimeOrigin::signed(alice.clone()), 50 * UNITS,),
			pallet_gigahdx::Error::<hydradx_runtime::Runtime>::ActiveVotesInOngoingReferenda,
		);
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

		assert_ok!(GigaHdxVoting::claim_rewards(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
		));

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

		// GigaHdxVotes should record only the GIGAHDX portion (50).
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, r);
		assert!(vote.is_some());
		assert_eq!(vote.unwrap().amount, 50 * UNITS);

		// Lock split should reflect the split.
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
		assert_eq!(
			alice_vote.amount,
			100 * UNITS,
			"Vote should record only GIGAHDX portion (100), not total vote (500)"
		);

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

		assert_ok!(GigaHdxVoting::claim_rewards(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
		));

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
		assert_eq!(pending.len(), 2, "Should have 2 pending reward entries, got {}", pending.len());

		let reward_a = pending.iter().find(|e| e.referenda_id == r_a).expect("Should have reward for referendum A");
		let reward_b = pending.iter().find(|e| e.referenda_id == r_b).expect("Should have reward for referendum B");
		assert!(reward_a.reward_amount > 0);
		assert!(reward_b.reward_amount > 0);
		let total_reward_hdx = reward_a.reward_amount + reward_b.reward_amount;

		let rate_before = GigaHdx::exchange_rate();
		let gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdxVoting::claim_rewards(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
		));

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
