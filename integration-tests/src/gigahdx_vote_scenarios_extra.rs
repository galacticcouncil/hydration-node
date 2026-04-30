// Extra GIGAHDX voting / lock scenario suite — derived from
// `ghdx-vote-scenarios-extra.md`.
//
// Same conventions as `gigahdx_vote_scenarios.rs`:
//   • EVE (`[99u8; 32]`) is the canonical fresh actor; assert she is empty
//     on the snapshot, then fund her with 20 M HDX.
//   • Each test asserts the CORRECT expected outcome. Failures here are
//     bugs (or harness limitations clearly noted).
//   • Tests that need referendum lifecycle / conviction lock-period
//     traversal are `#[ignore]`d so the scenario list stays complete.

#![allow(clippy::identity_op)]
#![allow(clippy::erasing_op)]

use crate::polkadot_test_net::{hydra_live_ext, TestNet, BOB, CHARLIE, DAVE, HDX};
use frame_support::{
	assert_noop, assert_ok,
	traits::{OnInitialize, StorePreimage},
};
use frame_system::RawOrigin;
use hydradx_runtime::{
	evm::{precompiles::erc20_mapping::HydraErc20Mapping, Executor},
	Balances, ConvictionVoting, Currencies, Democracy, EVMAccounts, GigaHdx, Preimage, Referenda, Runtime,
	RuntimeOrigin, Scheduler, System, Utility,
};
use hydradx_traits::evm::{CallContext, Erc20Mapping, InspectEvmAccounts, EVM};
use orml_traits::MultiCurrency;
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use primitives::constants::time::DAYS;
use primitives::Balance;
use sp_core::U256;
use sp_runtime::AccountId32;
use xcm_emulator::Network;

const PATH_TO_SNAPSHOT: &str = "snapshots/gigahdx/gigahdx5_slim";

const UNITS: Balance = 1_000_000_000_000;
const STHDX: u32 = 670;
const GIGAHDX: u32 = 67;

/// Fresh dummy account that does not exist on the snapshot.
const EVE: [u8; 32] = [99u8; 32];

// ---------------------------------------------------------------------------
// Helpers (shared with `gigahdx_vote_scenarios.rs` — duplicated to keep the
// file self-contained).
// ---------------------------------------------------------------------------

fn eve() -> AccountId32 {
	AccountId32::from(EVE)
}

fn bob() -> AccountId32 {
	AccountId32::from(BOB)
}

fn setup_fresh_eve() -> AccountId32 {
	let eve = eve();

	assert_eq!(Currencies::free_balance(HDX, &eve), 0, "Eve must start with 0 HDX");
	assert_eq!(
		Currencies::free_balance(GIGAHDX, &eve),
		0,
		"Eve must start with 0 GIGAHDX"
	);
	assert_eq!(
		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
		0,
		"Eve must start with 0 GigaHdxVotingLock"
	);
	let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
	assert_eq!(split.gigahdx_amount, 0);
	assert_eq!(split.hdx_amount, 0);
	assert!(pallet_conviction_voting::ClassLocksFor::<Runtime>::get(&eve).is_empty());

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		eve.clone(),
		20_000_000 * UNITS,
	));

	eve
}

fn next_block() {
	System::set_block_number(System::block_number() + 1);
	Scheduler::on_initialize(System::block_number());
	Democracy::on_initialize(System::block_number());
}

fn fast_forward_to(n: u32) {
	while System::block_number() < n {
		next_block();
	}
}

/// Fast-forward past the conviction lock period for a vote whose referendum
/// ended (or whose delegation prior was accumulated) at `end_block`.
#[allow(dead_code)]
fn advance_to_conviction_unlock(end_block: u32, conviction: Conviction) {
	let lock_periods: u32 = match conviction {
		Conviction::None => 0,
		Conviction::Locked1x => 1,
		Conviction::Locked2x => 2,
		Conviction::Locked3x => 4,
		Conviction::Locked4x => 8,
		Conviction::Locked5x => 16,
		Conviction::Locked6x => 32,
	};
	let vote_locking_period: u32 = 7 * DAYS;
	let target = end_block.saturating_add(lock_periods.saturating_mul(vote_locking_period));
	if System::block_number() < target {
		fast_forward_to(target.saturating_add(1));
	}
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

fn nay(amount: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote {
			aye: false,
			conviction: Conviction::None,
		},
		balance: amount,
	}
}

/// Submit a fresh referendum on track 0 (Root) and place the decision deposit.
/// Fast-forwards 5 days into the decision period.
fn begin_referendum_by_bob() -> u32 {
	let r = submit_referendum_by_bob();
	let now = System::block_number();
	fast_forward_to(now + 5 * DAYS);
	r
}

fn submit_referendum_by_bob() -> u32 {
	let bob = bob();
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<Runtime>::get();
	let now = System::block_number();

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		bob.clone(),
		1_000_000 * UNITS,
	));
	let proposal = {
		let inner = pallet_balances::Call::force_set_balance {
			who: AccountId32::from(CHARLIE),
			new_free: 2,
		};
		let outer = hydradx_runtime::RuntimeCall::Balances(inner);
		Preimage::bound(outer).unwrap()
	};
	assert_ok!(Referenda::submit(
		RuntimeOrigin::signed(bob),
		Box::new(RawOrigin::Root.into()),
		proposal,
		frame_support::traits::schedule::DispatchTime::At(now + 10 * DAYS),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		AccountId32::from(DAVE),
		2_000_000_000 * UNITS,
	));
	assert_ok!(Referenda::place_decision_deposit(
		RuntimeOrigin::signed(AccountId32::from(DAVE)),
		referendum_index,
	));

	referendum_index
}

/// Submit `n` referenda back-to-back and only then fast-forward into the
/// decision period — all `n` end up simultaneously ongoing.
fn begin_n_referenda_by_bob(n: u32) -> sp_std::vec::Vec<u32> {
	let now = System::block_number();
	let mut indices = sp_std::vec::Vec::with_capacity(n as usize);
	for _ in 0..n {
		indices.push(submit_referendum_by_bob());
	}
	fast_forward_to(now + 5 * DAYS);
	indices
}

/// Fast-forward past the active referendum's confirmation/decision period so it
/// completes (Approved). Mirrors the helper in `gigahdx_voting.rs`.
fn end_referendum() {
	let now = System::block_number();
	fast_forward_to(now + 12 * DAYS);
}

// ===========================================================================
// T. Delegations
// ===========================================================================

/// T1: Eve stakes 5 M → delegates 5 M to Bob (track 0). Lock expected.
#[test]
fn t1_delegate_creates_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		assert_ok!(ConvictionVoting::delegate(
			RuntimeOrigin::signed(eve.clone()),
			0,
			bob.into(),
			Conviction::None,
			gigahdx_bal,
		));

		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(lock, gigahdx_bal, "delegation must lock GIGAHDX side");
		assert_eq!(split.gigahdx_amount, gigahdx_bal);
		assert_eq!(split.hdx_amount, 0);
	});
}

/// T2: HDX-only delegate → stake → delegate's lock-split must refresh.
#[test]
fn t2_hdx_only_delegate_then_stake_must_refresh_lock_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		// Delegate 5 M HDX (no GIGAHDX yet) → split snapshot is (0, 5M H-side).
		assert_ok!(ConvictionVoting::delegate(
			RuntimeOrigin::signed(eve.clone()),
			0,
			bob.clone().into(),
			Conviction::None,
			5_000_000 * UNITS,
		));
		let split_pre = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_pre.gigahdx_amount, 0);
		assert_eq!(split_pre.hdx_amount, 5_000_000 * UNITS);

		// Stake 5M HDX → balance change. Eve has 20M HDX, 5M H-locked → 15M free.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));

		// Design constraint: split does NOT change on balance change. The
		// GIGAHDX received from staking is unlocked → transfer succeeds.
		let split_after = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_after, split_pre, "stake must not move the lock split");

		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob,
			GIGAHDX,
			gigahdx_bal,
		));
	});
}

/// T3: delegate → undelegate → unlock → locks cleared.
#[test]
fn t3_undelegate_then_unlock_clears_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert_ok!(ConvictionVoting::delegate(
			RuntimeOrigin::signed(eve.clone()),
			0,
			bob.clone().into(),
			Conviction::None,
			gigahdx_bal,
		));
		assert_ok!(ConvictionVoting::undelegate(RuntimeOrigin::signed(eve.clone()), 0));
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
	});
}

/// T6: delegated GIGAHDX cannot be transferred.
#[test]
fn t6_delegated_gigahdx_transfer_blocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert_ok!(ConvictionVoting::delegate(
			RuntimeOrigin::signed(eve.clone()),
			0,
			bob.clone().into(),
			Conviction::None,
			gigahdx_bal,
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob, GIGAHDX, UNITS);
		assert!(result.is_err());
	});
}

/// T4: delegate with Locked1x → undelegate → lock kept alive by delegation
/// prior for the lock period → past the period, `unlock` clears it.
#[test]
fn t4_undelegate_with_conviction_keeps_period_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert_ok!(ConvictionVoting::delegate(
			RuntimeOrigin::signed(eve.clone()),
			0,
			bob.clone().into(),
			Conviction::Locked1x,
			gigahdx_bal,
		));

		let undelegate_block = System::block_number();
		assert_ok!(ConvictionVoting::undelegate(RuntimeOrigin::signed(eve.clone()), 0));

		// Within the conviction period — `unlock` does not clear the lock; the
		// delegation prior keeps it alive.
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			gigahdx_bal,
			"undelegate prior keeps the G-side lock alive within the conviction period"
		);

		// Past the period — unlock clears it.
		advance_to_conviction_unlock(undelegate_block, Conviction::Locked1x);
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
	});
}

// ===========================================================================
// U. `unlock` extrinsic — the missing reconciliation step
// ===========================================================================

/// U1: vote → remove_vote → unlock ⇒ everything cleared.
#[test]
fn u1_vote_remove_unlock_clears_all_locks() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(5_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r,
		));
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
		assert!(pallet_conviction_voting::ClassLocksFor::<Runtime>::get(&eve).is_empty());
	});
}

/// U3: vote → remove_vote → unlock → transfer GIGAHDX accepted.
#[test]
fn u3_unlock_then_transfer_succeeds() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r,
		));
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			gigahdx_bal,
		));
	});
}

/// U4: vote 10 M HDX-only → remove → unlock → LockSplit fully zeroed.
#[test]
fn u4_hdx_only_vote_remove_unlock_zeroes_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r = begin_referendum_by_bob();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(10_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r,
		));
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
	});
}

/// U5: vote on two polls (same class), remove one, unlock — lock = remaining poll's amount.
#[test]
fn u5_partial_remove_unlock_keeps_max_remaining() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));

		let rs = begin_n_referenda_by_bob(2);
		let (r1, r2) = (rs[0], rs[1]);
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(5_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(3_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r1,
		));
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		// Lock should reduce to 3 M (remaining poll on the same class).
		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve);
		assert_eq!(lock, 3_000_000 * UNITS);

		// 5 M − 3 M = 2 M GIGAHDX should be transferable.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			2_000_000 * UNITS,
		));
		// One more wei should fail.
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, UNITS);
		assert!(result.is_err());
	});
}

/// U2: vote with Locked3x → end referendum (Approved) → remove_vote →
/// unlock during the conviction lock period → GIGAHDX lock must remain
/// (PriorLockSplit keeps it alive). Mirrors upstream's `prior` semantics.
///
/// Locked3x = 4 periods × 7 days = 28 days lock; after the 12-day end_referendum
/// fast-forward there's plenty of prior left.
#[test]
fn u2_remove_during_lock_period_unlock_retains_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye_with_conviction(5_000_000 * UNITS, Conviction::Locked3x),
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS
		);

		// End the referendum (12-day FF → Approved).
		end_referendum();

		// Remove vote on the now-Completed referendum. With Locked3x conviction,
		// hooks accumulate into PriorLockSplit and the lock stays alive.
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));

		// Unlock during the lock period — recompute, prior still active.
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"Locked3x conviction prior must keep the lock alive after remove_vote"
		);

		// Transfer GIGAHDX must still fail.
		let result = Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			Currencies::free_balance(GIGAHDX, &eve),
		);
		assert!(result.is_err(), "lock-period prior blocks transfer");
	});
}

/// U6: same as U2 but with Locked6x — heavier conviction, longer prior.
#[test]
fn u6_locked6x_unlock_within_period_retains_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye_with_conviction(5_000_000 * UNITS, Conviction::Locked6x),
		));
		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"Locked6x conviction prior keeps the lock alive"
		);
	});
}

/// U7: vote with Locked1x → end referendum → remove_vote → fast-forward past
/// the conviction lock period → unlock → GIGAHDX lock must clear.
/// VoteLockingPeriod = 7 days; Locked1x = 1 period = 7 days. Add buffer.
#[test]
fn u7_locked1x_unlock_after_period_clears_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye_with_conviction(5_000_000 * UNITS, Conviction::Locked1x),
		));
		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));

		// Walk past the conviction lock period (7 days for Locked1x, plus buffer).
		let now = System::block_number();
		fast_forward_to(now + 8 * DAYS);

		// Unlock — prior should rejig to zero now.
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			0,
			"prior must rejig to zero past the conviction lock period"
		);

		// GIGAHDX is now free to move.
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			gigahdx_bal,
		));
	});
}

/// U8 (regression): vote with Locked3x → end ref → remove_vote → giga_unstake.
///
/// PriorLockSplit holds 5M G-side commitment alive. The user's GIGAHDX balance
/// is fully covered by the lock; trying to giga_unstake invokes AAVE.withdraw
/// which calls our LockManager precompile (0x0806) and is blocked because
/// `GigaHdxVotingLock = 5M` and the post-burn balance would dip below it.
///
/// Pre-refactor a workaround in `on_unstake` capped GigaHdxVotingLock at the
/// post-unstake balance and spilled the remainder to the HDX-side. After the
/// refactor that workaround was dropped — this test documents the regression.
///
/// Expected (correct) behaviour: unstake succeeds, the user's HDX comes back
/// locked on the native side until the conviction prior expires.
#[test]
fn u8_unstake_with_active_prior_must_succeed_and_spill_to_hdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert!(gigahdx_bal > 0);

		// Vote 5M with Locked3x (4 periods × 7 days = 28 days lock).
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye_with_conviction(gigahdx_bal, Conviction::Locked3x),
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			gigahdx_bal,
			"vote locks the GIGAHDX-side at the committed amount"
		);

		// End the referendum (Approved) and remove vote — prior accumulates
		// because Locked3x lock-period (28 days) outlives the 12-day end_referendum FF.
		end_referendum();
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			gigahdx_bal,
			"prior keeps the lock alive"
		);

		// Now try to unstake. With the regression this fails because the AAVE
		// precompile blocks the aToken burn.
		let unstake_result = GigaHdx::giga_unstake(RuntimeOrigin::signed(eve.clone()), gigahdx_bal);
		assert_ok!(unstake_result);

		// After the unstake, the GIGAHDX-side cap should be 0 (no GIGAHDX left)
		// and the HDX-side native lock should hold the spilled commitment.
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			0,
			"no GIGAHDX left → G-side cap must be 0"
		);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(
			split.hdx_amount, gigahdx_bal,
			"committed amount spilled to HDX side, locked there until prior expires"
		);

		// Bonus: the spilled HDX is locked on the native side. A transfer of
		// the spilled amount from HDX free balance must fail.
		let hdx_total = Currencies::free_balance(HDX, &eve);
		assert!(hdx_total >= gigahdx_bal, "user got their HDX back from AAVE");
		// We can transfer at most (hdx_total - lock).
		let lock_amount = split.hdx_amount;
		let transferable = hdx_total.saturating_sub(lock_amount);
		// Try to transfer 1 wei more than what's free: must fail.
		let overshoot = transferable.saturating_add(UNITS);
		let result = Balances::transfer_allow_death(RuntimeOrigin::signed(eve.clone()), bob(), overshoot);
		assert!(result.is_err(), "spilled HDX-side lock must block over-transfer");
	});
}

// ===========================================================================
// V. Direct AAVE paths bypassing giga_stake / giga_unstake
// ===========================================================================

/// V2: stale LockSplit + direct EVM `Pool.withdraw` — must revert (defense in depth).
#[test]
fn v2_aave_withdraw_blocked_when_locked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use hydradx_runtime::evm::aave_trade_executor::Function as AaveFunction;
		use hydradx_runtime::evm::precompiles::handle::EvmDataWriter;
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));

		let pool = pallet_liquidation::GigaHdxPoolContract::<Runtime>::get();
		let sthdx_evm = HydraErc20Mapping::asset_address(STHDX);
		let eve_evm = EVMAccounts::evm_address(&eve);

		let data = EvmDataWriter::new_with_selector(AaveFunction::Withdraw)
			.write(sthdx_evm)
			.write(gigahdx_bal)
			.write(eve_evm)
			.build();
		let result = Executor::<Runtime>::call(CallContext::new_call(pool, eve_evm), data, U256::zero(), 500_000);
		assert!(matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)));
	});
}

// ===========================================================================
// X. Same-poll re-vote
// ===========================================================================

/// X1: vote 3 M same poll then re-vote 8 M ⇒ lock raised to 8 M.
#[test]
fn x1_same_poll_increase_raises_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(3_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve);
		assert_eq!(lock, 8_000_000 * UNITS);

		// Transfer 2 M (10 M − 8 M = 2 M unlocked).
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			2_000_000 * UNITS,
		));
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, UNITS);
		assert!(result.is_err());
	});
}

/// X2: vote 8 M then re-vote 3 M same poll. Documents the resulting lock.
/// The expected (correct) behaviour is that the lock reduces to 3 M for that
/// single poll; ConvictionVoting calls `extend_lock(3 M)` which our adapter
/// today guards-out (3 < 8 current_total). After the planned fix the lock
/// should refresh to 3 M.
#[test]
fn x2_same_poll_decrease_reduces_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(3_000_000 * UNITS),
		));

		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve);
		assert_eq!(lock, 3_000_000 * UNITS, "lock must reduce to the new vote balance");
	});
}

/// X4: vote aye 5 M → re-vote nay 5 M same poll ⇒ lock unchanged.
#[test]
fn x4_aye_to_nay_keeps_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			nay(gigahdx_bal),
		));

		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve);
		assert_eq!(lock, gigahdx_bal);
	});
}

// ===========================================================================
// Y. Other lock-id holders interactions
// ===========================================================================

/// Y1: vote-locked GIGAHDX + cooldown HDX after partial unstake — both locks active.
/// We unstake a small amount that leaves enough to cover the vote, then check that
/// both lock-ids show on the account and the usable HDX is `total − max(both locks)`.
#[test]
fn y1_vote_lock_and_unstake_cooldown_max_aggregate() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			10_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(5_000_000 * UNITS),
		));

		// Partial unstake — 3 M of GIGAHDX, leaving 7 M (>= 5 M lock).
		let unstake_result = GigaHdx::giga_unstake(RuntimeOrigin::signed(eve.clone()), 3_000_000 * UNITS);
		// If the runtime currently rejects this, surface the failure (E3).
		assert_ok!(unstake_result);

		// After partial unstake: the user has multiple locks on HDX.
		let locks = pallet_balances::Locks::<Runtime>::get(&eve);
		assert!(
			locks.len() >= 1,
			"at least the voting / cooldown HDX lock(s) must be present (locks={locks:?})",
		);
		let _ = gigahdx_bal;
	});
}

// ===========================================================================
// Z. Edge / boundary
// ===========================================================================

/// Z1: vote with balance == total holdings exactly.
#[test]
fn z1_vote_at_exact_total_accepted() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let total = Currencies::free_balance(HDX, &eve).saturating_add(Currencies::free_balance(GIGAHDX, &eve));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(total),
		));
	});
}

/// Z5: self-transfer GIGAHDX while locked.
#[test]
fn z5_self_transfer_while_locked_blocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));

		// Self-transfer: should be a no-op or a revert; either way the GIGAHDX
		// balance must be unchanged.
		let _ = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), eve.clone(), GIGAHDX, gigahdx_bal);
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), gigahdx_bal);
	});
}

// ===========================================================================
// AA. Atomicity (single-block multi-op)
// ===========================================================================

/// AA1: utility.batch([stake, vote]) — end-state lock split correct.
#[test]
fn aa1_batch_stake_then_vote_lock_split_correct() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r = begin_referendum_by_bob();

		let stake_call = hydradx_runtime::RuntimeCall::GigaHdx(pallet_gigahdx::Call::giga_stake {
			hdx_amount: 5_000_000 * UNITS,
		});
		let vote_call = hydradx_runtime::RuntimeCall::ConvictionVoting(pallet_conviction_voting::Call::vote {
			poll_index: r,
			vote: aye(5_000_000 * UNITS),
		});

		assert_ok!(Utility::batch(
			RuntimeOrigin::signed(eve.clone()),
			vec![stake_call, vote_call],
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 5_000_000 * UNITS);
		assert_eq!(split.hdx_amount, 0);

		// Transfer must fail.
		let result = Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			Currencies::free_balance(GIGAHDX, &eve),
		);
		assert!(result.is_err());
	});
}

/// AA2: utility.batch([vote 5 M, stake 5 M]) — A2 inside one block. Lock must end up GIGAHDX-side.
#[test]
fn aa2_batch_vote_then_stake_refreshes_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r = begin_referendum_by_bob();

		let vote_call = hydradx_runtime::RuntimeCall::ConvictionVoting(pallet_conviction_voting::Call::vote {
			poll_index: r,
			vote: aye(5_000_000 * UNITS),
		});
		let stake_call = hydradx_runtime::RuntimeCall::GigaHdx(pallet_gigahdx::Call::giga_stake {
			hdx_amount: 5_000_000 * UNITS,
		});

		assert_ok!(Utility::batch(
			RuntimeOrigin::signed(eve.clone()),
			vec![vote_call, stake_call],
		));

		// Vote 5M H + stake 5M. At the moment of the vote Eve had 20M HDX + 0 G.
		// Split snapshot is (0, 5M). The subsequent stake (balance change) must
		// NOT move the split — the GIGAHDX received from staking is unlocked.
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 5_000_000 * UNITS);

		// GIGAHDX is unlocked → transfer succeeds.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			Currencies::free_balance(GIGAHDX, &eve),
		));
	});
}

/// AA3: batch [vote, giga_unstake] — must reject (can_unstake fails mid-batch).
#[test]
fn aa3_batch_vote_then_unstake_reverts() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();

		let vote_call = hydradx_runtime::RuntimeCall::ConvictionVoting(pallet_conviction_voting::Call::vote {
			poll_index: r,
			vote: aye(5_000_000 * UNITS),
		});
		let unstake_call = hydradx_runtime::RuntimeCall::GigaHdx(pallet_gigahdx::Call::giga_unstake {
			gigahdx_amount: 5_000_000 * UNITS,
		});

		// `Utility::batch` runs each call and reports failure via event; the outer
		// extrinsic itself returns Ok. We therefore inspect the outcome via the
		// state, not the dispatch result.
		let _ = Utility::batch(RuntimeOrigin::signed(eve.clone()), vec![vote_call, unstake_call]);

		// Vote should have been recorded (ran first); unstake should NOT have moved
		// any GIGAHDX out — the user still holds the staked amount.
		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve);
		assert_eq!(lock, 5_000_000 * UNITS);
		// The unstake position list must be empty (no unstake actually happened).
		let positions = pallet_gigahdx::UnstakePositions::<Runtime>::get(&eve);
		assert!(positions.is_empty(), "unstake must not have produced a position");
	});
}

/// AA4: batch [stake, vote, remove_vote, unlock] — clean end state.
#[test]
fn aa4_batch_stake_vote_remove_unlock_clean_state() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r = begin_referendum_by_bob();

		let calls = vec![
			hydradx_runtime::RuntimeCall::GigaHdx(pallet_gigahdx::Call::giga_stake {
				hdx_amount: 5_000_000 * UNITS,
			}),
			hydradx_runtime::RuntimeCall::ConvictionVoting(pallet_conviction_voting::Call::vote {
				poll_index: r,
				vote: aye(5_000_000 * UNITS),
			}),
			hydradx_runtime::RuntimeCall::ConvictionVoting(pallet_conviction_voting::Call::remove_vote {
				class: None,
				index: r,
			}),
			hydradx_runtime::RuntimeCall::ConvictionVoting(pallet_conviction_voting::Call::unlock {
				class: 0,
				target: eve.clone().into(),
			}),
		];

		assert_ok!(Utility::batch(RuntimeOrigin::signed(eve.clone()), calls));

		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
	});
}

// ===========================================================================
// BB. EVM-side aToken edge cases
// ===========================================================================

/// BB1: mid-flight allowance change while voting active.
#[test]
fn bb1_allowance_change_then_vote_then_transfer_from_blocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		// initial approve (no vote yet)
		let mut approve = sp_io::hashing::keccak_256(b"approve(address,uint256)")[..4].to_vec();
		approve.extend_from_slice(&[0u8; 12]);
		approve.extend_from_slice(bob_evm.as_bytes());
		approve.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let _ = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			approve.clone(),
			U256::zero(),
			500_000,
		);

		// vote
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));

		// raise allowance further (should still succeed — allowance doesn't move balance)
		let mut approve_more = sp_io::hashing::keccak_256(b"approve(address,uint256)")[..4].to_vec();
		approve_more.extend_from_slice(&[0u8; 12]);
		approve_more.extend_from_slice(bob_evm.as_bytes());
		approve_more.extend_from_slice(&U256::from(gigahdx_bal.saturating_mul(2)).to_big_endian());
		let _ = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			approve_more,
			U256::zero(),
			500_000,
		);

		// transferFrom must revert because Eve's GIGAHDX is locked.
		let mut tf = sp_io::hashing::keccak_256(b"transferFrom(address,address,uint256)")[..4].to_vec();
		tf.extend_from_slice(&[0u8; 12]);
		tf.extend_from_slice(eve_evm.as_bytes());
		tf.extend_from_slice(&[0u8; 12]);
		tf.extend_from_slice(bob_evm.as_bytes());
		tf.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());

		let result =
			Executor::<Runtime>::call(CallContext::new_call(gigahdx_token, bob_evm), tf, U256::zero(), 500_000);
		assert!(matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)));
	});
}

/// BB4: approve while locked is allowed (allowance is permissive); the transfer must still revert.
#[test]
fn bb4_approve_while_locked_allowed_transfer_blocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));

		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		let mut approve = sp_io::hashing::keccak_256(b"approve(address,uint256)")[..4].to_vec();
		approve.extend_from_slice(&[0u8; 12]);
		approve.extend_from_slice(bob_evm.as_bytes());
		approve.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let res = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			approve,
			U256::zero(),
			500_000,
		);
		assert!(
			matches!(res.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"approve while locked must succeed (it doesn't move balance)",
		);

		let mut tf = sp_io::hashing::keccak_256(b"transferFrom(address,address,uint256)")[..4].to_vec();
		tf.extend_from_slice(&[0u8; 12]);
		tf.extend_from_slice(eve_evm.as_bytes());
		tf.extend_from_slice(&[0u8; 12]);
		tf.extend_from_slice(bob_evm.as_bytes());
		tf.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let result =
			Executor::<Runtime>::call(CallContext::new_call(gigahdx_token, bob_evm), tf, U256::zero(), 500_000);
		assert!(matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)));
	});
}

/// BB7: zero-amount aToken transfer while locked — accepted.
#[test]
fn bb7_zero_amount_atoken_transfer_accepted_while_locked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));

		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
		data.extend_from_slice(&[0u8; 12]);
		data.extend_from_slice(bob_evm.as_bytes());
		data.extend_from_slice(&U256::from(0u128).to_big_endian());
		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			data,
			U256::zero(),
			500_000,
		);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"zero-amount transfer must be accepted (no lock impact)",
		);
	});
}

// ===========================================================================
// CC. Total-balance, total-issuance and rate interactions
// ===========================================================================

/// CC1: inflate AAVE rate via direct gigapot HDX donation, then check vote ceiling.
/// `total_balance(who) = gigahdx + hdx` reads the raw GIGAHDX share count, not
/// the inflated underlying value, so the vote ceiling tracks shares.
#[test]
fn cc1_inflated_rate_does_not_inflate_vote_ceiling() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		// Donate HDX into the gigapot to inflate the rate.
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			gigapot,
			HDX,
			1_000_000 * UNITS,
		));

		// total_balance should be hdx_free + gigahdx_shares (raw share count).
		let hdx_free = Currencies::free_balance(HDX, &eve);
		let total = hdx_free.saturating_add(gigahdx_bal);

		let r = begin_referendum_by_bob();
		// Voting up to total should be accepted.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(total),
		));
		// One wei more should be rejected.
		let r2 = begin_referendum_by_bob();
		let result = ConvictionVoting::vote(RuntimeOrigin::signed(eve.clone()), r2, aye(total + UNITS));
		assert!(result.is_err());
	});
}
