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
use codec::Encode;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::{schedule::DispatchTime, Bounded, LockIdentifier, OnInitialize, StorePreimage};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::{
	pallet_custom_origins::Origin as CustomOrigin, Balances, BlockNumber, ConvictionVoting, Democracy, EVMAccounts,
	GigaHdx, GigaHdxRewards, OriginCaller, Preimage, Referenda, Runtime, RuntimeOrigin, Scheduler, System,
};
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use pallet_gigahdx::traits::VotingCommitmentInspect;
use pallet_referenda::ReferendumIndex;
use pallet_transaction_payment::ChargeTransactionPayment;
use primitives::constants::time::DAYS;
use primitives::AccountId;
use sp_runtime::traits::{DispatchTransaction, Dispatchable, TransactionExtension};
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

/// Step blocks until the referendum reaches a decided state, then stop
/// immediately. Unlike `end_referendum` (which overshoots by a fixed 12 days,
/// past the `end + VoteLockingPeriod` window), this lands `now ≈ end` so the
/// conviction lock applied by `remove_vote` is still active and observable.
fn fast_forward_until_completed(r: ReferendumIndex) {
	let mut guard = 0u32;
	loop {
		match pallet_referenda::ReferendumInfoFor::<Runtime>::get(r) {
			Some(pallet_referenda::ReferendumInfo::Approved(..))
			| Some(pallet_referenda::ReferendumInfo::Rejected(..)) => break,
			_ => {}
		}
		next_block();
		guard = guard.saturating_add(1);
		assert!(guard < 20 * DAYS, "referendum did not reach a decided state in time");
	}
}

/// Deterministically drive a referendum to a decided state. A real `Approved`
/// outcome is unreachable on the mainnet-state snapshot (root-track *support* is
/// turnout ÷ active issuance, far below threshold at test scale), so force the
/// poll status directly. `remove_vote` only reads the moment + variant, so this
/// faithfully exercises the completed-referendum reward/lock path for either
/// outcome. `end = now`, so the conviction-lock window (`end + VoteLockingPeriod`)
/// is open at the immediately-following `remove_vote`.
fn force_referendum_completed(r: ReferendumIndex, approved: bool) {
	let now = System::block_number();
	if approved {
		pallet_referenda::ReferendumInfoFor::<Runtime>::insert(
			r,
			pallet_referenda::ReferendumInfo::Approved(now, None, None),
		);
	} else {
		pallet_referenda::ReferendumInfoFor::<Runtime>::insert(
			r,
			pallet_referenda::ReferendumInfo::Rejected(now, None, None),
		);
	}
}

const CONVICTION_VOTING_ID: LockIdentifier = *b"pyconvot";
const GIGAHDX_LOCK_ID: LockIdentifier = *b"ghdxlock";

/// Amount currently locked under `id` on `who`'s HDX, or 0.
fn lock_for(who: &AccountId, id: LockIdentifier) -> u128 {
	Balances::locks(who)
		.iter()
		.find(|l| l.id == id)
		.map(|l| l.amount)
		.unwrap_or(0)
}

/// Conviction-voting (`pyconvot`) lock currently held on `who`'s HDX, or 0.
fn conviction_lock(who: &AccountId) -> u128 {
	lock_for(who, CONVICTION_VOTING_ID)
}

/// Clear any expired conviction (`pyconvot`) lock for `who` on the root track.
fn conviction_unlock(who: &AccountId) {
	assert_ok!(ConvictionVoting::unlock(
		RuntimeOrigin::signed(who.clone()),
		ROOT_TRACK_CLASS,
		who.clone(),
	));
}

/// gigahdx cooldown (blocks between `giga_unstake` and `unlock`).
fn gigahdx_cooldown() -> BlockNumber {
	<Runtime as pallet_gigahdx::Config>::CooldownPeriod::get()
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

		// Alice 100 HDX × Locked3x (1× base) → weighted = 100 * UNITS.
		// Bob   100 HDX × Locked1x (0.25×)  → weighted =  25 * UNITS.
		// Total weighted = 125 * UNITS — divides 10% of the 100_000-UNIT pot
		// (= 10^16) cleanly into 8/5 and 1/5 shares with no rounding dust.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked3x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
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
		// Verify the pro-rata split: alice (weighted 100) gets 4× bob (weighted 25).
		assert_eq!(alice_reward, 4 * bob_reward);

		// Pool is deleted after the last claim drains it to zero.
		assert!(
			pallet_gigahdx_rewards::ReferendaRewardPool::<Runtime>::get(r).is_none(),
			"pool must be cleaned up after last voter",
		);
	});
}

// Voting rewards are paid for *participation*, weighted by conviction — never by
// vote direction or referendum outcome (`record_user_reward` splits the pool
// pro-rata by `weighted`, with no aye/nay or pass/reject branch anywhere). This
// test pins that property explicitly: a referendum that ends `Rejected` still
// pays both the (losing) AYE voter and the NAY voter their full pro-rata share.
#[test]
fn rewards_should_credit_voters_when_referendum_is_rejected() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into(); // AYE — ends up on the losing side
		let bob: AccountId = BOB.into(); // NAY — outweighs AYE so the referendum is rejected

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 300 * UNITS));

		let r = begin_referendum();

		// Tally weight (conviction-voting's 1× for Locked1x): AYE 100 vs NAY 300 ⇒ approval
		// 25% < the root track's end-of-period threshold, so the referendum ends `Rejected`
		// and Alice's AYE is on the losing side.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			nay_with_conviction(300 * UNITS, Conviction::Locked1x),
		));

		fast_forward_until_completed(r);

		// Precondition for the test's premise: the AYE side actually lost.
		assert!(
			matches!(
				pallet_referenda::ReferendumInfoFor::<Runtime>::get(r),
				Some(pallet_referenda::ReferendumInfo::Rejected(..))
			),
			"referendum must end Rejected so the AYE voter is on the losing side",
		);

		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		// Root-track allocation = 10% of the accumulator at the first remove_vote.
		let expected_allocation = accumulator_before / 10;

		// Alice removes first (takes her exact pro-rata share); Bob (last) scoops the rest.
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

		// Reward weight (gigahdx's 0.25× for Locked1x): Alice 25, Bob 75 ⇒ 1:3 split of the
		// whole pool — paid despite the rejection, with the losing AYE voter included.
		assert!(alice_reward > 0, "losing AYE voter must still receive a reward");
		assert_eq!(
			bob_reward,
			3 * alice_reward,
			"rewards follow weighted votes (25:75), not the outcome"
		);
		assert_eq!(
			alice_reward.saturating_add(bob_reward),
			expected_allocation,
			"the entire allocation must be distributed across both voters regardless of outcome",
		);

		// Lock-symmetry: rewards are outcome-independent, but the conviction
		// *lock* must be too. The winning NAY voter is locked by conviction-voting
		// for the vote balance; the losing AYE voter must be locked for the same
		// period on the staked amount her reward was computed on. Before the
		// `lock_balance_on_unsuccessful_vote` fix the loser had no `pyconvot` lock
		// and could farm the conviction-weighted reward, then exit on only the
		// 28-day unstake cooldown — never paying the lock the multiplier prices in.
		assert_eq!(
			conviction_lock(&bob),
			300 * UNITS,
			"winning voter is conviction-locked for the vote balance",
		);
		assert_eq!(
			conviction_lock(&alice),
			100 * UNITS,
			"losing voter must be conviction-locked symmetrically (regression guard)",
		);
	});
}

// Mirror of `rewards_should_credit_voters_when_referendum_is_rejected`: identical
// stakes/convictions (Alice 100, Bob 300, both Locked1x), so identical reward
// weights (25 : 75) and therefore identical rewards — but the AYE/NAY directions
// are swapped so the referendum ends `Approved` instead of `Rejected`. The two
// tests together prove the payout is a pure function of weighted participation,
// independent of the outcome: both voters get the exact same share either way.
#[test]
fn rewards_should_credit_voters_when_referendum_is_approved() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into(); // NAY — ends up on the losing side
		let bob: AccountId = BOB.into(); // AYE — outweighs NAY so the referendum is approved

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 300 * UNITS));

		let r = begin_referendum();

		// Same amounts/convictions as the rejected test, only the directions are swapped:
		// tally weight AYE 300 vs NAY 100 ⇒ 75% approval, so the referendum ends `Approved`.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			nay_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			aye_with_conviction(300 * UNITS, Conviction::Locked1x),
		));

		// A real `Approved` is unreachable at test scale (support threshold), so
		// force it — `remove_vote` reads only the outcome, which is what we test.
		force_referendum_completed(r, true);

		// Premise: this referendum approves (opposite outcome to the rejected mirror).
		assert!(
			matches!(
				pallet_referenda::ReferendumInfoFor::<Runtime>::get(r),
				Some(pallet_referenda::ReferendumInfo::Approved(..))
			),
			"referendum must end Approved",
		);

		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		let expected_allocation = accumulator_before / 10;

		// Alice removes first (exact pro-rata share); Bob (last) scoops the rest.
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

		// Exactly the same split as the rejected mirror (25 : 75), proving the reward is
		// outcome-independent: the losing NAY voter and the winning AYE voter are both paid.
		assert!(alice_reward > 0, "losing NAY voter must still receive a reward");
		assert_eq!(
			bob_reward,
			3 * alice_reward,
			"rewards follow weighted votes (25:75), not the outcome"
		);
		assert_eq!(
			alice_reward.saturating_add(bob_reward),
			expected_allocation,
			"the entire allocation must be distributed across both voters regardless of outcome",
		);

		// Lock-symmetry, approved mirror: the winning AYE voter is locked by
		// conviction-voting; the losing NAY voter must be locked the same way on
		// her staked amount. Same property as the rejected test, opposite outcome.
		assert_eq!(
			conviction_lock(&bob),
			300 * UNITS,
			"winning voter is conviction-locked for the vote balance",
		);
		assert_eq!(
			conviction_lock(&alice),
			100 * UNITS,
			"losing voter must be conviction-locked symmetrically (regression guard)",
		);
	});
}

// Removing a vote while the referendum is still *ongoing* forfeits the vote: no
// reward is allocated (the referendum never completed) and conviction-voting
// applies no `pyconvot` lock (the lock only attaches to a *completed* vote). The
// gigahdx unstake-freeze is released either way. Identical for AYE and NAY.
#[test]
fn remove_vote_during_referendum_should_not_lock_or_reward_for_both_directions() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into(); // AYE
		let bob: AccountId = BOB.into(); // NAY

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));

		let r = begin_referendum();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked3x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			nay_with_conviction(100 * UNITS, Conviction::Locked3x),
		));

		// Active votes freeze the stake (unstake guard) but place no `pyconvot`
		// lock yet — the referendum is still ongoing.
		assert_eq!(GigaHdxRewards::committed(&alice), 100 * UNITS);
		assert_eq!(GigaHdxRewards::committed(&bob), 100 * UNITS);

		// Remove while ongoing (no fast-forward).
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

		for who in [alice.clone(), bob.clone()] {
			assert_eq!(
				pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&who),
				0,
				"no reward for an ongoing (never-completed) referendum"
			);
			// No conviction *commitment* was incurred: a vote removed while ongoing
			// accrues no lock period, so `unlock` clears the `pyconvot` lock
			// *immediately* — no fast-forward — unlike a vote removed after
			// completion (which stays locked for the full conviction period).
			conviction_unlock(&who);
			assert_eq!(
				conviction_lock(&who),
				0,
				"ongoing-removed vote incurs no conviction-period lock"
			);
			// Freeze lifted → unstake proceeds immediately (only the cooldown applies).
			assert_eq!(GigaHdxRewards::committed(&who), 0);
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(who.clone()), 100 * UNITS));
		}
	});
}

// Parity with the AYE freeze test (`giga_unstake_should_fail_when_stake_is_frozen_by_active_vote`):
// a NAY vote freezes the gigahdx stake exactly the same way, and `remove_vote`
// releases it.
#[test]
fn giga_unstake_should_fail_when_stake_is_frozen_by_active_nay_vote() {
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

		assert_eq!(GigaHdxRewards::committed(&alice), 100 * UNITS);
		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS),
			pallet_gigahdx::Error::<Runtime>::StakeFrozen,
		);

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		assert_eq!(
			GigaHdxRewards::committed(&alice),
			0,
			"remove_vote must unfreeze the stake"
		);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
	});
}

// Full post-completion lifecycle proven identical for AYE (winning side of the
// forced approval) and NAY (losing side): both are rewarded, both are
// conviction-locked for the staked amount, both can `giga_unstake` (the
// conviction lock relabels in place — it never blocks the unstake), and after
// the cooldown + conviction period both the `ghdxlock` and the `pyconvot` lock
// release cleanly. This is the "everything works the same as AYE — locking,
// unlocking, unstaking" guarantee.
#[test]
fn conviction_vote_full_lifecycle_should_match_for_aye_and_nay() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into(); // AYE — winning side of the forced approval
		let bob: AccountId = BOB.into(); // NAY — losing side

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
			nay_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		force_referendum_completed(r, true);
		let completed_at = System::block_number();

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

		// Post-completion state is identical for the winning AYE and losing NAY voter.
		for who in [alice.clone(), bob.clone()] {
			assert!(
				pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&who) > 0,
				"both voters rewarded regardless of side"
			);
			assert_eq!(
				conviction_lock(&who),
				100 * UNITS,
				"both voters conviction-locked for the staked amount"
			);
			// Vote record gone → unstake guard released.
			assert_eq!(GigaHdxRewards::committed(&who), 0);
			// Unlock before the conviction period elapses is a no-op — the
			// commitment holds (contrast: a vote removed while ongoing clears at once).
			conviction_unlock(&who);
			assert_eq!(
				conviction_lock(&who),
				100 * UNITS,
				"conviction lock persists until the period elapses"
			);
			// Conviction lock does not block the unstake (it relabels in place).
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(who.clone()), 100 * UNITS));
			// Still conviction-locked: the unstake does not shorten the commitment.
			assert_eq!(
				conviction_lock(&who),
				100 * UNITS,
				"conviction lock persists through the gigahdx unstake"
			);
		}

		// Past the cooldown (28 DAYS) — which exceeds the conviction period (7 DAYS) —
		// both the gigahdx `unlock` and conviction-voting `unlock` release cleanly.
		System::set_block_number(completed_at + gigahdx_cooldown() + 1);
		for who in [alice.clone(), bob.clone()] {
			assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(who.clone()), completed_at));
			conviction_unlock(&who);
			assert_eq!(
				lock_for(&who, GIGAHDX_LOCK_ID),
				0,
				"ghdxlock released after gigahdx unlock"
			);
			assert_eq!(conviction_lock(&who), 0, "conviction lock released after the period");
		}
	});
}

// Partial vote: stake X, vote with X/2. The reward, the unstake freeze, and the
// conviction lock must all track the *voted* amount (X/2) — not the full stake
// and not zero — identically for the winning AYE and losing NAY voter. The
// unvoted half carries no conviction commitment.
#[test]
fn partial_vote_should_lock_and_reward_only_the_voted_amount_for_both_directions() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into(); // AYE — winning side of the forced approval
		let bob: AccountId = BOB.into(); // NAY — losing side

		// Stake X = 200, vote with X/2 = 100.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 200 * UNITS));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 200 * UNITS));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(100 * UNITS, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			nay_with_conviction(100 * UNITS, Conviction::Locked1x),
		));

		// Freeze (unstake guard) is the voted half only — the other X/2 stays
		// unstakeable while the vote is active.
		assert_eq!(GigaHdxRewards::committed(&alice), 100 * UNITS);
		assert_eq!(GigaHdxRewards::committed(&bob), 100 * UNITS);

		force_referendum_completed(r, true);

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

		for who in [alice.clone(), bob.clone()] {
			assert!(
				pallet_gigahdx_rewards::PendingRewards::<Runtime>::get(&who) > 0,
				"voted stake earns a reward"
			);
			// The crux: lock equals the voted amount (X/2 = 100), not the full
			// stake (X = 200) and not zero — same for the winning AYE and losing NAY.
			assert_eq!(
				conviction_lock(&who),
				100 * UNITS,
				"conviction lock equals the voted amount (X/2), not the full stake"
			);
			assert_eq!(GigaHdxRewards::committed(&who), 0, "freeze released after remove");
		}
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
		assert_eq!(GigaHdxRewards::committed(&alice), 100 * UNITS);

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
		assert_eq!(
			GigaHdxRewards::committed(&alice),
			0,
			"remove_vote must unfreeze the stake"
		);

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

		// Split votes ARE recorded (with weighted=0) so liquidation's
		// clearance adapter can find them — but they earn no rewards.
		let rec = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).unwrap();
		assert_eq!(rec.weighted, 0);
		let tally = pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).unwrap();
		assert_eq!(tally.total_weighted, 0);
		assert_eq!(tally.voters_count, 1);

		end_referendum();
		let accumulator_before = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r,
		));
		// Allocation briefly fires (pot moves out), but every voter has
		// weighted=0 so all of `remaining_reward` is refunded back to the
		// accumulator → net change is zero.
		let accumulator_after = Balances::free_balance(&GigaHdxRewards::reward_accumulator_pot());
		assert_eq!(accumulator_after, accumulator_before);
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
		// Locked1x = 0.25× → 100 * 0.25 = 25 UNITS weighted.
		assert_eq!(tally_after_first.total_weighted, 25 * UNITS);
		assert_eq!(tally_after_first.voters_count, 1);
		assert_eq!(GigaHdxRewards::committed(&alice), 100 * UNITS,);

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(200 * UNITS, Conviction::Locked3x),
		));

		let record = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).unwrap();
		assert_eq!(record.staked_vote_amount, 200 * UNITS);
		// Locked3x = 1× (base) → 200 * 1 = 200 UNITS weighted.
		assert_eq!(record.weighted, 200 * UNITS);
		let tally_after_edit = pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).unwrap();
		assert_eq!(tally_after_edit.total_weighted, 200 * UNITS);
		assert_eq!(tally_after_edit.voters_count, 1, "edit must not increment voters_count");
		assert_eq!(GigaHdxRewards::committed(&alice), 200 * UNITS,);

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
			GigaHdxRewards::committed(&alice),
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

		let rec = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, r).unwrap();
		assert_eq!(rec.weighted, 0);
		let tally = pallet_gigahdx_rewards::ReferendaTotalWeightedVotes::<Runtime>::get(r).unwrap();
		assert_eq!(tally.total_weighted, 0);
		assert_eq!(tally.voters_count, 1);

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
		// Locked1x = 0.25× → 100 * 0.25 = 25 UNITS weighted (nay/aye treated symmetrically).
		assert_eq!(record.weighted, 25 * UNITS);

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

// ---------------------------------------------------------------------------
// Voting commitment = the *overlap* of a staker's votes (the max single
// reservation), not their sum: conviction voting locks the same balance for
// every concurrent vote, so N partial votes of `amount` commit `amount`, not
// `N * amount`. `giga_unstake` pulls it lazily via `GigaHdxRewards::committed`.
//
// Scenario: Alice stakes X, votes X/2 on three live referenda. The other X/2
// is never used for voting and stays unstakeable.
// ---------------------------------------------------------------------------
#[test]
fn giga_unstake_should_release_unused_half_when_voting_partial_amount_on_multiple_referenda() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();

		// 1. Alice stakes X HDX -> receives gigahdx.
		let x = 100 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), x));

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake should exist");
		assert_eq!(stake.hdx, x);
		assert_eq!(GigaHdxRewards::committed(&alice), 0);
		let gigahdx_total = stake.gigahdx;

		// 2. Three referenda; Alice votes X/2 on each. Cast each vote right after
		//    opening its referendum (votes persist regardless of later status).
		let half = x / 2;
		for _ in 0..3 {
			let r = begin_referendum();
			assert_ok!(ConvictionVoting::vote(
				RuntimeOrigin::signed(alice.clone()),
				r,
				aye_with_conviction(half, Conviction::Locked3x),
			));
		}

		// 3. Only X/2 is ever committed — the same half backs all three votes.
		assert_eq!(GigaHdxRewards::committed(&alice), half);

		// 4. Alice unstakes the unused half — never used for voting, so it
		//    succeeds (post-unstake hdx = X/2 ≥ committed X/2).
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_total / 2,
		));
	});
}

// ---------------------------------------------------------------------------
// Voting-commitment guard coverage.
//
// `GigaHdxRewards::committed(who)` is the MAX over the user's active
// per-referendum reservations (`UserVoteRecords[who, *].staked_vote_amount`) —
// the overlap, never the sum. `giga_unstake` pulls it lazily; nothing is
// maintained on the voting path. These cover add, remove, edit-down, edit-up,
// clear-all and the over-stake cap.
// ---------------------------------------------------------------------------

/// Removing the single largest vote drops `committed` to the next-highest
/// reservation (it is computed fresh from the surviving votes, not decremented).
#[test]
fn frozen_guard_should_recompute_to_second_highest_when_largest_vote_removed() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let x = 100 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), x));
		let gigahdx_total = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().gigahdx;

		let quarter = x / 4;

		// Two small reservations, then the large one created last so it is fresh
		// and unambiguously removable while ongoing.
		let r_a = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r_a,
			aye_with_conviction(quarter, Conviction::Locked1x),
		));
		let r_b = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r_b,
			aye_with_conviction(quarter, Conviction::Locked1x),
		));
		let r_max = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r_max,
			aye_with_conviction(x, Conviction::Locked1x),
		));

		// Remove the largest vote.
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(alice.clone()),
			Some(ROOT_TRACK_CLASS),
			r_max,
		));

		// committed = max(X/4, X/4) = X/4 (the removed X reservation is gone).
		assert_eq!(
			GigaHdxRewards::committed(&alice),
			quarter,
			"removing the largest vote drops committed to the next-highest reservation"
		);

		// The freed 3X/4 must be unstakeable.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_total - gigahdx_total / 4,
		));
	});
}

/// Editing the largest vote *down* lowers `committed` to the true overlap.
#[test]
fn frozen_guard_should_lower_when_largest_vote_edited_down() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let x = 100 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), x));
		let gigahdx_total = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().gigahdx;

		let quarter = x / 4;

		// A smaller reservation on another referendum.
		let r_other = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r_other,
			aye_with_conviction(quarter, Conviction::Locked1x),
		));

		// The large reservation, created last so it is still ongoing for the edit.
		let r_edit = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r_edit,
			aye_with_conviction(x, Conviction::Locked1x),
		));

		// Edit it down to X/4.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r_edit,
			aye_with_conviction(quarter, Conviction::Locked1x),
		));

		// committed = max(X/4 other, X/4 edited) = X/4.
		assert_eq!(
			GigaHdxRewards::committed(&alice),
			quarter,
			"editing the largest vote down lowers committed to the true overlap"
		);

		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_total - gigahdx_total / 4,
		));
	});
}

/// Editing a vote *up* raises `committed` to the new amount.
#[test]
fn frozen_guard_should_raise_when_vote_edited_up() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let x = 100 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), x));
		let gigahdx_total = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().gigahdx;

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(x / 4, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(x / 2, Conviction::Locked1x),
		));

		assert_eq!(
			GigaHdxRewards::committed(&alice),
			x / 2,
			"editing a vote up must raise frozen to the new reservation"
		);

		// The remaining free half is still unstakeable.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_total / 2,
		));
	});
}

/// Removing every vote clears `committed` to zero and frees the whole stake.
#[test]
fn frozen_guard_should_clear_when_all_votes_removed() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let x = 100 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), x));
		let gigahdx_total = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().gigahdx;

		let half = x / 2;
		let mut refs = sp_std::vec::Vec::new();
		for _ in 0..3 {
			let r = begin_referendum();
			assert_ok!(ConvictionVoting::vote(
				RuntimeOrigin::signed(alice.clone()),
				r,
				aye_with_conviction(half, Conviction::Locked1x),
			));
			refs.push(r);
		}

		for r in refs {
			assert_ok!(ConvictionVoting::remove_vote(
				RuntimeOrigin::signed(alice.clone()),
				Some(ROOT_TRACK_CLASS),
				r,
			));
		}

		assert_eq!(
			GigaHdxRewards::committed(&alice),
			0,
			"removing all votes must clear frozen"
		);

		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_total,
		));
	});
}

/// A vote larger than the staked amount commits only the stake (`min(vote,
/// staked)`), never the raw vote balance.
#[test]
fn frozen_guard_should_cap_at_stake_when_voting_more_than_staked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		let x = 100 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), x));

		let r = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			r,
			aye_with_conviction(2 * x, Conviction::Locked1x),
		));

		assert_eq!(
			GigaHdxRewards::committed(&alice),
			x,
			"frozen must be capped at the staked amount, not the raw vote balance"
		);
	});
}

/// `giga_unstake`'s weight declares the worst-case `UserVoteRecords` scan
/// (`reads(250)`), so pre-dispatch charges that fee up front; post-dispatch then
/// refunds the unused portion down to the reservations actually read. Exercised
/// through the real `ChargeTransactionPayment` extension: HDX leaves the account
/// at pre-dispatch, and some comes back at post-dispatch.
#[test]
fn giga_unstake_should_charge_worst_case_fee_then_refund_unused_scan() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_rewards();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// Two active votes: the actual scan reads 2 records, far below the
		// worst-case 250 the weight declares — so a real refund is due.
		let quarter = 25 * UNITS;
		for _ in 0..2 {
			let r = begin_referendum();
			assert_ok!(ConvictionVoting::vote(
				RuntimeOrigin::signed(alice.clone()),
				r,
				aye_with_conviction(quarter, Conviction::Locked1x),
			));
		}
		assert_eq!(GigaHdxRewards::committed(&alice), quarter);

		let call = hydradx_runtime::RuntimeCall::GigaHdx(pallet_gigahdx::Call::giga_unstake {
			gigahdx_amount: 10 * UNITS,
		});
		let info = call.get_dispatch_info();
		let len = call.encoded_size();

		// Staked HDX stays (locked) in Alice's account across the unstake, so the
		// only free-balance movement here is the fee charge / refund.
		let balance_before = Balances::free_balance(&alice);

		// Pre-dispatch: withdraw the worst-case fee (weight includes reads(250)).
		let pre = ChargeTransactionPayment::<Runtime>::from(0).validate_and_prepare(
			Some(alice.clone()).into(),
			&call,
			&info,
			len,
			0,
		);
		assert_ok!(&pre);
		let (pre_data, _) = pre.unwrap();

		let balance_after_charge = Balances::free_balance(&alice);
		let charged = balance_before - balance_after_charge;
		assert!(charged > 0, "pre-dispatch must charge the worst-case fee");

		// Dispatch: returns the actual weight (base + reads(2)).
		let result = call.dispatch(RuntimeOrigin::signed(alice.clone()));
		assert_ok!(result);

		// Post-dispatch: refund the over-declared scan weight.
		assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
			pre_data,
			&info,
			&mut result.unwrap(),
			len,
			&Ok(()),
		));

		let balance_final = Balances::free_balance(&alice);
		let refunded = balance_final - balance_after_charge;
		let actual_fee = balance_before - balance_final;

		assert!(
			refunded > 0,
			"post-dispatch must refund the unused worst-case scan weight"
		);
		assert!(
			actual_fee < charged,
			"net fee paid must be below the worst-case pre-charge"
		);
	});
}
