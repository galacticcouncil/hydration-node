// SPDX-License-Identifier: Apache-2.0
//
// End-to-end integration tests for `pallet-gigahdx-rewards` against the
// live mainnet-state snapshot used by `gigahdx.rs`.
//
// These tests exercise the full extrinsic path:
//   `pallet-conviction-voting::vote` → `VotingHooksImpl::on_before_vote` →
//   user record + stake freeze. Then:
//   `pallet-conviction-voting::remove_vote` → `on_remove_vote(Completed)` →
//   pool allocation + per-user pro-rata payout + `claim_rewards` compounding.
//
// They verify that the runtime wiring (`CombinedVotingHooks`,
// `RuntimeReferenda`, the two pot `PalletId`s) plumbs through correctly,
// that the freeze guard in `pallet-gigahdx::giga_unstake` is enforced under
// real dispatch, and that the `RuntimeReferenda` adapter resolves the
// current track for an ongoing referendum.

#![cfg(test)]

use crate::gigahdx::PATH_TO_SNAPSHOT;
use crate::polkadot_test_net::{hydra_live_ext, TestNet, ALICE, BOB, CHARLIE, DAVE, UNITS};
use frame_support::traits::{schedule::DispatchTime, Bounded, OnInitialize, StorePreimage};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::{
	pallet_custom_origins::Origin as CustomOrigin, Balances, BlockNumber, ConvictionVoting, Democracy, EVMAccounts,
	GigaHdx, GigaHdxRewards, OriginCaller, Preimage, Referenda, Runtime, RuntimeOrigin, Scheduler, System,
};
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use pallet_referenda::ReferendumIndex;
use primitives::constants::time::DAYS;
use primitives::AccountId;
use xcm_emulator::Network;

// ---------------------------------------------------------------------------
// Helpers — ported from the legacy `gigahdx_voting.rs` integration tests and
// adapted for the rewards model. Helpers that referenced `pallet-gigahdx-voting`
// (which no longer exists on this branch) have been dropped or replaced with
// the equivalent `pallet-gigahdx-rewards` API.
// ---------------------------------------------------------------------------

type CallOf<T> = <T as frame_system::Config>::RuntimeCall;
type BoundedCallOf<T> = Bounded<CallOf<T>, <T as frame_system::Config>::Hashing>;

fn set_balance_proposal(who: AccountId, value: u128) -> BoundedCallOf<Runtime> {
	let inner = pallet_balances::Call::force_set_balance { who, new_free: value };
	let outer = hydradx_runtime::RuntimeCall::Balances(inner);
	Preimage::bound(outer).unwrap()
}

fn propose_set_balance(
	who: AccountId,
	dest: AccountId,
	value: u128,
	dispatch_time: BlockNumber,
) -> frame_support::dispatch::DispatchResult {
	Referenda::submit(
		RuntimeOrigin::signed(who),
		Box::new(RawOrigin::Root.into()),
		set_balance_proposal(dest, value),
		DispatchTime::At(dispatch_time),
	)
}

/// Submit a referendum (Alice), deposit (Dave), fast-forward into the
/// deciding period. Returns the referendum index.
fn begin_referendum() -> ReferendumIndex {
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<Runtime>::get();
	let now = System::block_number();

	assert_ok!(propose_set_balance(ALICE.into(), CHARLIE.into(), 2, now + 10 * DAYS));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		2_000_000_000 * UNITS,
	));

	assert_ok!(Referenda::place_decision_deposit(
		RuntimeOrigin::signed(DAVE.into()),
		referendum_index,
	));

	fast_forward_to(now + 5 * DAYS);

	referendum_index
}

/// Fast-forward past the decision + confirmation window so the referendum
/// transitions to a `Completed` status.
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

fn nay_with_conviction(amount: u128, conviction: Conviction) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: false, conviction },
		balance: amount,
	}
}

/// Submit a referendum on the given proposal origin (track is resolved from it).
fn begin_referendum_on_track(proposal_origin: OriginCaller) -> ReferendumIndex {
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<Runtime>::get();
	let now = System::block_number();

	assert_ok!(Referenda::submit(
		RuntimeOrigin::signed(ALICE.into()),
		Box::new(proposal_origin),
		set_balance_proposal(CHARLIE.into(), 2),
		DispatchTime::At(now + 10 * DAYS),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		2_000_000_000 * UNITS,
	));
	assert_ok!(Referenda::place_decision_deposit(
		RuntimeOrigin::signed(DAVE.into()),
		referendum_index,
	));

	fast_forward_to(now + 5 * DAYS);
	referendum_index
}

fn init_rewards() {
	init_rewards_with_pot(100_000 * UNITS);
}

fn init_rewards_with_pot(pot_amount: u128) {
	let accumulator = GigaHdxRewards::reward_accumulator_pot();
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		accumulator,
		pot_amount,
	));

	// `giga_stake` mints aToken through AAVE; users must have a bound EVM address.
	for who in [ALICE, BOB, CHARLIE] {
		let account: AccountId = who.into();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			account.clone(),
			1_000_000 * UNITS,
		));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(account));
	}
}

/// Root track id on this runtime. Matches `pallet-referenda`'s convention
/// (track 0 = root). Used as the explicit class arg to `remove_vote` — when
/// `class = None` conviction-voting can fail with `ClassNeeded` depending on
/// the user's voting state.
const ROOT_TRACK_CLASS: u16 = 0;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn rewards_should_skip_non_stakers_when_voting() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();

		// Alice never stakes — `pallet_gigahdx::Stakes[alice]` is None, so
		// `VotingHooksImpl::on_before_vote` returns early on the first guard
		// without creating a `UserVoteRecord`.
		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye(50 * UNITS),
		));

		assert!(pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).is_none());
		assert!(pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).is_none());

		end_referendum();

		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let accumulator_after = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_eq!(
			accumulator_after, accumulator_before,
			"non-staker remove_vote must not drain the accumulator"
		);
		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), 0);
	});
}

#[test]
fn rewards_should_credit_pro_rata_when_two_stakers_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 200 * UNITS));

		let r = begin_referendum();

		// Alice 100 HDX × 3x → weighted = 300 * UNITS.
		// Bob 200 HDX × 1x → weighted = 200 * UNITS.
		// Total weighted = 500 * UNITS.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked3x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(200 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		// Root-track allocation = 10% of accumulator at first remove_vote.
		let expected_allocation = accumulator_before / 10;

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(bob.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));

		let alice_reward = pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice);
		let bob_reward = pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&bob);

		assert!(alice_reward > 0, "alice must receive a share");
		assert!(bob_reward > 0, "bob must receive a share");
		assert_eq!(
			alice_reward + bob_reward,
			expected_allocation,
			"the entire allocation must be distributed across the two voters",
		);

		// Pool is deleted after the last claim drains it to zero.
		assert!(
			pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none(),
			"pool must be cleaned up after last voter",
		);
	});
}

#[test]
fn last_voter_should_scoop_remaining_pool() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		// First claimant: allocation gets snapshotted; alice's pro-rata share
		// is computed against the frozen denominator.
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let pool_after_alice = pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r)
			.expect("pool exists between first and last voter");
		let bob_expected = pool_after_alice.remaining_reward;
		assert!(bob_expected > 0);

		// Second claimant scoops *exactly* the remaining_reward — no `floor`,
		// no leftover dust. Pool storage must be deleted afterwards.
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(bob.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let bob_reward = pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&bob);
		assert_eq!(bob_reward, bob_expected, "last voter scoops remaining_reward exactly");
		assert!(pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none());
	});
}

#[test]
fn giga_unstake_should_fail_when_stake_is_frozen_by_active_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		// Stake.frozen now equals 100 HDX (the staked-vote-capped balance) —
		// any unstake that would bring `hdx < frozen` must fail with `StakeFrozen`.
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("alice has stake");
		assert_eq!(stake.frozen, 100 * UNITS);

		// `giga_unstake` operates on gigahdx (atokens) but the frozen check is
		// against the post-payout HDX side. Burning all 100 atokens would set
		// hdx → 0, well below frozen=100 → StakeFrozen.
		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS),
			pallet_gigahdx::Error::<Runtime>::StakeFrozen,
		);

		// After remove_vote (referendum still ongoing → Status::Ongoing path),
		// the freeze is released and the unstake succeeds.
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("alice still has stake");
		assert_eq!(stake.frozen, 0, "remove_vote must unfreeze the stake");

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
	});
}

#[test]
fn claim_rewards_should_compound_into_gigahdx_position() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		end_referendum();

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));

		let pending = pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice);
		assert!(pending > 0, "alice must have a pending reward after solo vote");

		let stake_before = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("alice staked");
		let hdx_before = stake_before.hdx;
		let gigahdx_before = stake_before.gigahdx;

		assert_ok!(GigaHdxRewards::claim_rewards(RuntimeOrigin::signed(alice.clone())));

		let stake_after = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("alice still staked");
		assert_eq!(
			stake_after.hdx,
			hdx_before + pending,
			"claimed HDX must be compounded into the active stake",
		);
		assert!(
			stake_after.gigahdx > gigahdx_before,
			"GIGAHDX position must grow after claim",
		);
		assert_eq!(
			pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice),
			0,
			"pending rewards must be cleared on claim",
		);
	});
}

#[test]
fn rewards_should_ignore_split_votes() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			AccountVote::Split {
				aye: 40 * UNITS,
				nay: 30 * UNITS,
			},
		));

		// Split votes are dropped silently by `on_before_vote` — no record,
		// no live-tally entry.
		assert!(pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).is_none());
		assert!(pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).is_none());

		end_referendum();
		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let accumulator_after = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_eq!(
			accumulator_after, accumulator_before,
			"split votes must not trigger a pool allocation",
		);
		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), 0);
		assert!(pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none());
	});
}

#[test]
fn rewards_should_use_track_specific_percentage_when_non_root_track() {
	// Treasurer = track 5 → 5% (vs root's 10%).
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum_on_track(OriginCaller::Origins(CustomOrigin::Treasurer));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		let expected_allocation = accumulator_before / 20;

		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(5),
			r,
		));

		let accumulator_after = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_eq!(accumulator_before - accumulator_after, expected_allocation);
		assert_eq!(
			pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice),
			expected_allocation,
		);
	});
}

#[test]
fn rewards_should_replace_weighted_when_vote_is_edited() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 200 * UNITS));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		let tally_after_first = pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).unwrap();
		assert_eq!(tally_after_first.total_weighted, 100 * UNITS);
		assert_eq!(tally_after_first.voters_count, 1);
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			100 * UNITS,
		);

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(200 * UNITS, Conviction::Locked3x),
		));

		let record = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).unwrap();
		assert_eq!(record.staked_vote_amount, 200 * UNITS);
		assert_eq!(record.weighted, 600 * UNITS);
		let tally_after_edit = pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).unwrap();
		assert_eq!(tally_after_edit.total_weighted, 600 * UNITS);
		assert_eq!(tally_after_edit.voters_count, 1, "edit must not increment voters_count");
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			200 * UNITS,
		);

		end_referendum();
		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		let expected_allocation = accumulator_before / 10;

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));

		assert_eq!(
			pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice),
			expected_allocation,
		);
	});
}

#[test]
fn rewards_should_skip_allocation_when_referendum_is_cancelled() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		assert_ok!(Referenda::cancel(RawOrigin::Root.into(), r));

		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let accumulator_after = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());

		assert_eq!(accumulator_after, accumulator_before);
		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), 0);
		assert!(pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none());
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			0,
			"cancelled referendum must still unfreeze the stake",
		);
	});
}

#[test]
fn pending_rewards_should_accumulate_across_multiple_referenda() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r1 = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r1,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		end_referendum();

		let pot_before_r1 = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		let expected_r1 = pot_before_r1 / 10;
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r1,
		));

		let r2 = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r2,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		end_referendum();

		let pot_before_r2 = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		let expected_r2 = pot_before_r2 / 10;
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r2,
		));

		assert_eq!(
			pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice),
			expected_r1 + expected_r2,
		);

		let stake_before = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_ok!(GigaHdxRewards::claim_rewards(RuntimeOrigin::signed(alice.clone())));
		let stake_after = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(stake_after.hdx, stake_before.hdx + expected_r1 + expected_r2);
		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), 0);
	});
}

#[test]
fn rewards_should_ignore_split_abstain_votes() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			AccountVote::SplitAbstain {
				aye: 20 * UNITS,
				nay: 20 * UNITS,
				abstain: 30 * UNITS,
			},
		));

		assert!(pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).is_none());
		assert!(pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).is_none());

		end_referendum();
		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		let accumulator_after = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_eq!(accumulator_after, accumulator_before);
		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), 0);
		assert!(pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none());
	});
}

#[test]
fn rewards_should_credit_nay_voters_same_as_aye() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			nay_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		let record = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).unwrap();
		assert_eq!(record.staked_vote_amount, 100 * UNITS);
		assert_eq!(record.weighted, 100 * UNITS);

		end_referendum();
		let pot_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		let expected = pot_before / 10;

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));

		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), expected);
	});
}

#[test]
fn rewards_should_cleanup_with_zero_payout_when_accumulator_is_empty() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards_with_pot(0);

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		end_referendum();
		let allocated_pot_before = Balances::free_balance(&GigaHdxRewards::allocated_rewards_pot());
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));

		assert_eq!(
			Balances::free_balance(&GigaHdxRewards::allocated_rewards_pot()),
			allocated_pot_before,
		);
		assert_eq!(pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&alice), 0);
		assert!(pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none());
		assert_noop!(
			GigaHdxRewards::claim_rewards(RuntimeOrigin::signed(alice)),
			pallet_gigahdx_rewards::Error::<Runtime>::NoPendingRewards,
		);
	});
}
