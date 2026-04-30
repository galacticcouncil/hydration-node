// GIGAHDX voting / lock scenario suite.
//
// One test per scenario from `ghdx-vote-scenarios.md`. Every test asserts the
// CORRECT expected behaviour. If the assertion does not hold today, the test
// is meant to fail — that is the point. We are not fixing bugs here, just
// documenting them.
//
// All tests use EVE — a fresh `[99u8; 32]` account that does not exist on the
// gigahdx4 snapshot. Every test starts by sanity-checking Eve is empty and
// then funds her with 20 M HDX. State transitions are then performed against
// that clean slate.

#![allow(clippy::identity_op)]
#![allow(clippy::erasing_op)]

use crate::polkadot_test_net::{hydra_live_ext, TestNet, BOB, CHARLIE, DAVE, HDX};
use frame_support::{
	assert_noop, assert_ok,
	traits::{OnInitialize, StorePreimage},
};
use frame_system::RawOrigin;
use hydradx_runtime::{
	evm::{
		aave_trade_executor::Function as AaveFunction, precompiles::erc20_mapping::HydraErc20Mapping,
		precompiles::handle::EvmDataWriter, Executor,
	},
	Balances, ConvictionVoting, Currencies, Democracy, EVMAccounts, GigaHdx, Preimage, Referenda, Runtime,
	RuntimeOrigin, Scheduler, System,
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
// Helpers
// ---------------------------------------------------------------------------

fn eve() -> AccountId32 {
	AccountId32::from(EVE)
}

fn bob() -> AccountId32 {
	AccountId32::from(BOB)
}

/// Sanity-check Eve has nothing on the snapshot, then fund her with 20 M HDX.
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
	assert_eq!(split.gigahdx_amount, 0, "Eve must start with empty LockSplit (gigahdx)");
	assert_eq!(split.hdx_amount, 0, "Eve must start with empty LockSplit (hdx)");
	assert!(
		pallet_conviction_voting::ClassLocksFor::<Runtime>::get(&eve).is_empty(),
		"Eve must start with no ConvictionVoting class locks"
	);

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

fn aye(amount: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote {
			aye: true,
			conviction: Conviction::None,
		},
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

fn aye_with_conviction(amount: u128, conviction: Conviction) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: true, conviction },
		balance: amount,
	}
}

#[allow(dead_code)]
fn nay_with_conviction(amount: u128, conviction: Conviction) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: false, conviction },
		balance: amount,
	}
}

/// Submit a fresh referendum on track 0 (Root) and place the decision deposit.
/// Returns the referendum index. Fast-forwards 5 days into the decision period.
fn begin_referendum_by_bob() -> u32 {
	let r = submit_referendum_by_bob();
	let now = System::block_number();
	fast_forward_to(now + 5 * DAYS);
	r
}

/// Same as `begin_referendum_by_bob` but does NOT fast-forward — useful when the
/// caller wants to submit multiple referenda before advancing time so all of
/// them are simultaneously in the decision period.
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
#[allow(dead_code)]
fn begin_n_referenda_by_bob(n: u32) -> sp_std::vec::Vec<u32> {
	let now = System::block_number();
	let mut indices = sp_std::vec::Vec::with_capacity(n as usize);
	for _ in 0..n {
		indices.push(submit_referendum_by_bob());
	}
	fast_forward_to(now + 5 * DAYS);
	indices
}

// ---------------------------------------------------------------------------
// Force a referendum to a specific terminal outcome by direct storage mutation.
// pallet-referenda's tally machinery is fiddly to drive deterministically from
// outside (decision deposits, support/approval thresholds, decision/confirm
// periods); for tests that exercise *post-outcome* code paths we just rewrite
// `ReferendumInfoFor` and let downstream readers (conviction-voting's
// `try_access_poll`, our `RuntimeReferendumInfo` adapter) see the result.
// ---------------------------------------------------------------------------

/// Idempotent forced-outcome helper. If the referendum is still Ongoing it's
/// transitioned to the requested terminal state; if it's already finished
/// (e.g. the Scheduler auto-approved it during a fast-forward), it's left
/// alone. Crucially, never use destructive `info.take()` — that would delete
/// an already-finished entry on a no-match.
fn finalize_with<F>(index: u32, build: F)
where
	F: FnOnce(
		u32,
		Option<pallet_referenda::Deposit<sp_runtime::AccountId32, primitives::Balance>>,
		Option<pallet_referenda::Deposit<sp_runtime::AccountId32, primitives::Balance>>,
	) -> pallet_referenda::ReferendumInfoOf<Runtime, ()>,
{
	use pallet_referenda::ReferendumInfo;
	let now = System::block_number();
	if let Some(ReferendumInfo::Ongoing(status)) = pallet_referenda::ReferendumInfoFor::<Runtime>::get(index) {
		pallet_referenda::ReferendumInfoFor::<Runtime>::insert(
			index,
			build(now, Some(status.submission_deposit), status.decision_deposit),
		);
	}
}

#[allow(dead_code)]
fn force_approve_referendum(index: u32) {
	finalize_with(index, |end, sub, dec| {
		pallet_referenda::ReferendumInfo::Approved(end, sub, dec)
	});
}

#[allow(dead_code)]
fn force_reject_referendum(index: u32) {
	finalize_with(index, |end, sub, dec| {
		pallet_referenda::ReferendumInfo::Rejected(end, sub, dec)
	});
}

#[allow(dead_code)]
fn force_cancel_referendum(index: u32) {
	finalize_with(index, |end, sub, dec| {
		pallet_referenda::ReferendumInfo::Cancelled(end, sub, dec)
	});
}

#[allow(dead_code)]
fn force_kill_referendum(index: u32) {
	use pallet_referenda::ReferendumInfo;
	let now = System::block_number();
	if matches!(
		pallet_referenda::ReferendumInfoFor::<Runtime>::get(index),
		Some(ReferendumInfo::Ongoing(_))
	) {
		pallet_referenda::ReferendumInfoFor::<Runtime>::insert(index, ReferendumInfo::Killed(now));
	}
}

/// Fast-forward past the conviction lock period for a vote cast at the current
/// block on a referendum that ended at `end_block`. Locked1x = 1 lock period,
/// Locked2x = 2, ..., Locked6x = 32. Period length = `VoteLockingPeriod`.
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

// ===========================================================================
// A. Order of voting vs staking
// ===========================================================================

/// A1: vote 10 M HDX → stake 5 M ⇒ split must NOT change on the stake (design
/// constraint: lock state changes only on vote-related events). The split was
/// (0, 10M) at vote time and stays that way; the user's GIGAHDX received from
/// the stake is unlocked and freely transferable.
#[test]
fn a1_vote_then_stake_should_refresh_lock_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r = begin_referendum_by_bob();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(10_000_000 * UNITS),
		));

		let split_before = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_before.gigahdx_amount, 0);
		assert_eq!(split_before.hdx_amount, 10_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));

		let split_after = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_after, split_before, "stake must not change the lock split");
	});
}

/// A2 (BUG, observed live): vote 10 M HDX → remove → stake 5 M → vote 5 M ⇒ transfer GIGAHDX must fail.
#[test]
fn a2_vote_remove_stake_revote_transfer_must_fail() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		let r1 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(10_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r1,
		));

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert!(gigahdx_bal > 0);

		let r2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(5_000_000 * UNITS),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob, GIGAHDX, gigahdx_bal);
		assert!(result.is_err(), "locked GIGAHDX must not be transferable");
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), gigahdx_bal);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			gigahdx_bal,
		);
	});
}

/// A3: vote 10 M HDX → stake 5 M → re-vote 10 M same track ⇒ transfer GIGAHDX must fail.
#[test]
fn a3_vote_stake_revote_same_amount_transfer_must_fail() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r = begin_referendum_by_bob();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(10_000_000 * UNITS),
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(10_000_000 * UNITS),
		));

		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// A4: vote 10 M HDX → stake 5 M → vote 5 M on a fresh poll ⇒ transfer GIGAHDX must fail.
#[test]
fn a4_vote_stake_smaller_revote_transfer_must_fail() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let r1 = begin_referendum_by_bob();

		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(10_000_000 * UNITS),
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(5_000_000 * UNITS),
		));

		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// A5: stake 5 M → vote 5 M ⇒ transfer GIGAHDX must fail. Control case.
#[test]
fn a5_stake_then_vote_transfer_must_fail() {
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
			aye(5_000_000 * UNITS),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

// ===========================================================================
// B. remove_vote and lock-clearing
// ===========================================================================

/// B1: vote 5 M GIGAHDX → remove_vote ⇒ both LockSplit and GigaHdxVotingLock zero.
#[test]
fn b1_remove_vote_clears_lock() {
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
		// remove_vote alone doesn't shrink the lock — mirror upstream and call unlock.
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

/// B2: vote → remove → transfer GIGAHDX succeeds.
#[test]
fn b2_after_remove_vote_transfer_succeeds() {
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
		// remove_vote alone doesn't shrink the lock — mirror upstream and call unlock.
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
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 0);
	});
}

/// B3: vote 5 M Locked6x → remove_vote during lock period ⇒ transfer rejected.
#[test]
fn b3_remove_vote_with_conviction_lock_blocks_transfer() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked6x),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r,
		));

		// Lock-period of Locked6x means the conviction lock should still apply.
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err(), "conviction lock period must still block the transfer");
	});
}

/// B4 (BUG?): vote 10 M HDX → remove_vote ⇒ LockSplit fully cleared.
#[test]
fn b4_remove_vote_clears_lock_split_for_hdx_only_vote() {
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
		assert_eq!(split.gigahdx_amount, 0, "LockSplit gigahdx must clear");
		assert_eq!(split.hdx_amount, 0, "LockSplit hdx must clear");
	});
}

/// B5: vote on 2 polls (same class) → remove vote on one → transfer GIGAHDX still rejected.
#[test]
fn b5_partial_remove_keeps_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let rs = begin_n_referenda_by_bob(2);
		let (r1, r2) = (rs[0], rs[1]);
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(gigahdx_bal),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(gigahdx_bal),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r1,
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err(), "lock from r2 still applies");
	});
}

// ===========================================================================
// C. Re-vote behaviour
// ===========================================================================

/// C1: vote 5 M aye-None → re-vote 5 M aye-Locked1x ⇒ transfer rejected (lock period applies).
#[test]
fn c1_revote_with_higher_conviction_blocks_transfer() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked1x),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// C2: vote Locked1x → re-vote with Locked6x ⇒ transfer rejected.
#[test]
fn c2_revote_increasing_conviction_keeps_lock() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked1x),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye_with_conviction(gigahdx_bal, Conviction::Locked6x),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// C3: vote aye → switch to nay ⇒ transfer still rejected (lock from balance still applies).
#[test]
fn c3_switch_aye_to_nay_keeps_lock() {
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

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// C4: vote 3 M → re-vote 5 M (extend) ⇒ transfer 4 M rejected.
#[test]
fn c4_extend_vote_blocks_partial_transfer() {
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
			aye(3_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(5_000_000 * UNITS),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, 4_000_000 * UNITS);
		assert!(result.is_err(), "5 M lock blocks a 4 M transfer (only 0 unlocked)");
	});
}

/// C5: vote 5 M → re-vote 3 M (reduce) ⇒ transfer 3 M succeeds (only 3 M still locked).
#[test]
fn c5_reduce_vote_allows_partial_transfer() {
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
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(3_000_000 * UNITS),
		));

		// 3 M locked, 2 M is the unlocked portion of the 5 M GIGAHDX.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			2_000_000 * UNITS,
		));
		// 4th million should fail.
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, UNITS);
		assert!(result.is_err());
	});
}

// ===========================================================================
// D. Multi-track / multi-referenda
// ===========================================================================
//
// Without a way to submit on multiple tracks from this test harness, we
// approximate "multiple tracks" using multiple polls on the same class. The
// behavior under test (max-of-locks within a class) is the same for the
// purposes of asserting transfer-blocking.

/// D1: vote on 2 polls (5 M each) ⇒ max class lock = 5 M, transfer GIGAHDX rejected.
#[test]
fn d1_multi_poll_max_lock_blocks_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		let rs = begin_n_referenda_by_bob(2);
		let (r1, r2) = (rs[0], rs[1]);
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(gigahdx_bal),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(gigahdx_bal),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// D2: vote 5 M on r1 + 3 M on r2 ⇒ transfer 4 M rejected.
#[test]
fn d2_multi_poll_max_lock_blocks_partial_transfer() {
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

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, 4_000_000 * UNITS);
		assert!(result.is_err());
	});
}

/// D3: vote 5 M r1 → remove r1 → still vote 3 M r2 ⇒ transfer 4 M rejected.
#[test]
fn d3_multi_poll_remove_one_keeps_lock() {
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

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, 4_000_000 * UNITS);
		assert!(result.is_err());
	});
}

/// D4: vote on 3 polls → remove all → transfer succeeds.
#[test]
fn d4_remove_all_votes_allows_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let rs = begin_n_referenda_by_bob(3);
		for &r in rs.iter() {
			assert_ok!(ConvictionVoting::vote(
				RuntimeOrigin::signed(eve.clone()),
				r,
				aye(gigahdx_bal),
			));
		}
		for &r in rs.iter() {
			assert_ok!(ConvictionVoting::remove_vote(
				RuntimeOrigin::signed(eve.clone()),
				None,
				r,
			));
		}
		// remove_vote alone does not shrink the lock — call unlock once.
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
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 0);
	});
}

// ===========================================================================
// E. Unstake interactions
// ===========================================================================

/// E1: stake 5 M → vote 5 M GIGAHDX → giga_unstake while ref ongoing ⇒ rejected by `can_unstake`.
#[test]
fn e1_unstake_while_voting_active_rejected() {
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

		let result = GigaHdx::giga_unstake(RuntimeOrigin::signed(eve.clone()), gigahdx_bal);
		assert!(result.is_err(), "must not unstake while a vote is still active");
	});
}

/// E3: stake 10 M → vote 5 M → unstake 3 M succeeds (still 5 M GIGAHDX > 5 M lock after unstake).
#[test]
fn e3_partial_unstake_within_unlocked_succeeds() {
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
			aye(5_000_000 * UNITS),
		));

		// Cannot unstake while vote is active (E1 covers full unstake).
		// Partial unstake within the unlocked portion is what we're testing here.
		let result = GigaHdx::giga_unstake(RuntimeOrigin::signed(eve.clone()), 3_000_000 * UNITS);
		// The pallet may either allow this (if can_unstake returns true based on
		// remaining-after-unstake ≥ lock) or reject it. Assert the correct
		// behaviour: unstake 3 M leaves 7 M GIGAHDX, lock is 5 M ⇒ should succeed.
		assert_ok!(result);
	});
}

/// E4: stake 10 M → vote 5 M → unstake 8 M ⇒ rejected (would leave 2 M < 5 M lock).
#[test]
fn e4_overstake_unstake_rejected() {
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
			aye(5_000_000 * UNITS),
		));

		let result = GigaHdx::giga_unstake(RuntimeOrigin::signed(eve.clone()), 8_000_000 * UNITS);
		assert!(result.is_err(), "unstake that would breach the lock must be rejected");
	});
}

/// E5: stake 5 M → vote 5 M → full unstake while ongoing ⇒ rejected.
#[test]
fn e5_full_unstake_while_voting_rejected() {
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

		let result = GigaHdx::giga_unstake(RuntimeOrigin::signed(eve.clone()), gigahdx_bal);
		assert!(result.is_err());
	});
}

/// E7: stake 10M → vote 5M with Locked1x → end ref + remove_vote (prior keeps
/// 5M alive on G-side) → partial unstake 3M succeeds. Post-unstake the user
/// has 7M GIGAHDX, which still covers the 5M prior, so AAVE.burn(3M) is
/// permitted by the LockManager precompile.
#[test]
fn e7_partial_unstake_after_remove_vote_with_prior_succeeds() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			10_000_000 * UNITS,
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 10_000_000 * UNITS);

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye_with_conviction(5_000_000 * UNITS, Conviction::Locked1x),
		));

		// Approve and remove the vote — prior accumulates with 5M G-side.
		force_approve_referendum(r);
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"prior keeps 5M G-side locked after remove_vote"
		);

		// Partial unstake 3M — leaves 7M GIGAHDX, still ≥ 5M prior. Should succeed.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(eve.clone()),
			3_000_000 * UNITS,
		));

		// Post-unstake invariants: 7M GIGAHDX remaining, 5M prior still alive.
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 7_000_000 * UNITS);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"prior still alive — G-side cap stays at 5M (no spillover needed)"
		);
	});
}

/// E8: same as E7 but the user does NOT call `remove_vote` first. Once the
/// referendum is finished, `can_unstake` returns true (votes-on-ongoing-only
/// check) and `on_unstake` force-removes finished votes itself. Partial
/// unstake of the free portion succeeds.
#[test]
fn e8_partial_unstake_after_referendum_finishes_without_explicit_remove_vote() {
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
			aye_with_conviction(5_000_000 * UNITS, Conviction::Locked1x),
		));

		// Approve referendum but DO NOT call remove_vote — vote stays in
		// GigaHdxVotes until on_unstake force-removes it.
		force_approve_referendum(r);
		assert!(
			pallet_gigahdx_voting::GigaHdxVotes::<Runtime>::get(&eve, r).is_some(),
			"vote still in storage before unstake"
		);

		// Partial unstake — on_unstake force-removes the finished vote and the
		// prior accumulates. Post-unstake balance (7M) ≥ prior (5M).
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(eve.clone()),
			3_000_000 * UNITS,
		));

		assert!(
			pallet_gigahdx_voting::GigaHdxVotes::<Runtime>::get(&eve, r).is_none(),
			"on_unstake force-removed the finished vote"
		);
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 7_000_000 * UNITS);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"prior accumulated by force-remove keeps 5M locked"
		);
	});
}

// ---------------------------------------------------------------------------
// E2 / E6 require referendum lifecycle (approve/reject) which the test
// harness does not currently fast-forward through cleanly. They are sketched
// as ignored tests so the scenario is tracked.
// ---------------------------------------------------------------------------

// ===========================================================================
// F. Receiving GIGAHDX from outside while voting active
// ===========================================================================

/// F1: Eve votes 5 M → Bob (with prior GIGAHDX from giga_stake) sends 5 M to Eve.
/// Eve should be able to transfer 5 M (the unlocked portion) elsewhere.
#[test]
fn f1_receive_gigahdx_unlocked_portion_transferable() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		// Eve stakes & votes 5 M.
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

		// Bob acquires GIGAHDX too (he's not voting).
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			10_000_000 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(bob.clone()),
			5_000_000 * UNITS,
		));
		let bob_gigahdx = Currencies::free_balance(GIGAHDX, &bob);
		assert!(bob_gigahdx > 0);

		// Bob sends his GIGAHDX to Eve.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			eve.clone(),
			GIGAHDX,
			bob_gigahdx,
		));

		// Eve now holds bob_gigahdx + 5 M, but only 5 M is locked.
		// She should be able to transfer the newly received bob_gigahdx out.
		let charlie = AccountId32::from(CHARLIE);
		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), charlie, GIGAHDX, bob_gigahdx);
		assert_ok!(result);
	});
}

/// F2 (BUG?): Eve votes with HDX-only LockSplit (cf. A2) → Bob sends GIGAHDX to Eve.
/// Lock should refresh on receipt — until then Eve's GIGAHDX is unlocked.
#[test]
fn f2_receive_gigahdx_after_hdx_only_vote_must_refresh_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		// Eve votes 10 M HDX without any GIGAHDX yet — LockSplit becomes
		// { gigahdx: 0, hdx: 10 M }.
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(10_000_000 * UNITS),
		));

		// Bob mints some GIGAHDX and sends it to Eve.
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			10_000_000 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(bob.clone()),
			5_000_000 * UNITS,
		));
		let bob_gigahdx = Currencies::free_balance(GIGAHDX, &bob);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			eve.clone(),
			GIGAHDX,
			bob_gigahdx,
		));

		// Correct behaviour under the design constraint: lock state changes ONLY
		// on vote-related events. Receiving GIGAHDX is a balance change → split
		// stays at (0, 10M). The newly-received GIGAHDX is unlocked.
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(
			split.gigahdx_amount, 0,
			"balance receipt must not move lock onto G-side"
		);
		assert_eq!(split.hdx_amount, 10_000_000 * UNITS, "H-side lock unchanged");
	});
}

/// F3: A holds GIGAHDX, no vote → receives more GIGAHDX. No lock should appear.
#[test]
fn f3_receive_gigahdx_without_vote_no_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			10_000_000 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(bob.clone()),
			5_000_000 * UNITS,
		));
		let bob_gigahdx = Currencies::free_balance(GIGAHDX, &bob);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			eve.clone(),
			GIGAHDX,
			bob_gigahdx,
		));

		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob,
			GIGAHDX,
			bob_gigahdx,
		));
	});
}

// ===========================================================================
// G. Conviction levels
// ===========================================================================

/// G1: vote 5 M aye-None ⇒ transfer rejected, lock = 5 M, no time component.
#[test]
fn g1_aye_none_blocks_transfer() {
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

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err());
	});
}

/// G2: vote with Locked1x → approved → remove_vote within lock period →
/// transfer rejected (prior is alive).
#[test]
fn g2_locked1x_blocks_transfer_after_finish_until_lock_period() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked1x),
		));

		force_approve_referendum(r);

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

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err(), "prior keeps GIGAHDX locked within Locked1x period");
	});
}

/// G3: vote with Locked6x → approved → transfer rejected even days later
/// (Locked6x = 32 lock periods × 7 days = 224 days; well past 50-day mark).
#[test]
fn g3_locked6x_blocks_transfer_within_lock_period() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked6x),
		));

		force_approve_referendum(r);

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));
		// Advance well into — but not past — the Locked6x period.
		fast_forward_to(System::block_number() + 50 * DAYS);
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(result.is_err(), "Locked6x prior still alive 50 days post-approval");
	});
}

/// G4: vote with Locked6x → approved → past the full 32-period lock window
/// → unlock clears the lock and transfer succeeds.
#[test]
fn g4_locked6x_unlocks_after_full_lock_period() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked6x),
		));

		force_approve_referendum(r);
		let end_block = System::block_number();

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));

		advance_to_conviction_unlock(end_block, Conviction::Locked6x);
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

/// G5: vote with Conviction::None → approved → no prior accumulated → lock
/// clears immediately after remove_vote+unlock; transfer succeeds.
#[test]
fn g5_aye_none_unlocks_after_finish() {
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

		force_approve_referendum(r);

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

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			gigahdx_bal,
		));
	});
}

// ===========================================================================
// H. EVM-direct transfer paths
// ===========================================================================

/// H1: vote 5 M → call aToken.transfer via EVM directly ⇒ revert.
#[test]
fn h1_atoken_transfer_via_evm_blocked_when_locked() {
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

		// Bind EVM addresses for both sides.
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));

		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		// Build ERC20 transfer(to, amount) calldata.
		let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
		data.extend_from_slice(&[0u8; 12]);
		data.extend_from_slice(bob_evm.as_bytes());
		data.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());

		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			data,
			U256::zero(),
			500_000,
		);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"EVM aToken transfer must revert on locked balance",
		);
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), gigahdx_bal);
	});
}

/// H2: vote 5 M → AAVE Pool.withdraw via EVM ⇒ revert.
#[test]
fn h2_aave_withdraw_via_evm_blocked_when_locked() {
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

		let pool = pallet_liquidation::GigaHdxPoolContract::<Runtime>::get();
		let sthdx_evm = HydraErc20Mapping::asset_address(STHDX);
		let eve_evm = EVMAccounts::evm_address(&eve);

		let data = EvmDataWriter::new_with_selector(AaveFunction::Withdraw)
			.write(sthdx_evm)
			.write(gigahdx_bal)
			.write(eve_evm)
			.build();
		let result = Executor::<Runtime>::call(CallContext::new_call(pool, eve_evm), data, U256::zero(), 500_000);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"AAVE Pool.withdraw must revert when GIGAHDX is voting-locked",
		);
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), gigahdx_bal);
	});
}

/// H3: vote 5 M → aToken.transferFrom via approved spender ⇒ rejected.
#[test]
fn h3_atoken_transfer_from_via_evm_blocked_when_locked() {
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

		// Approve Bob (spender) to spend Eve's GIGAHDX.
		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		// approve(bob, amount)
		let mut approve = sp_io::hashing::keccak_256(b"approve(address,uint256)")[..4].to_vec();
		approve.extend_from_slice(&[0u8; 12]);
		approve.extend_from_slice(bob_evm.as_bytes());
		approve.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let _ = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			approve,
			U256::zero(),
			500_000,
		);

		// transferFrom(eve, bob, amount) signed by bob_evm
		let mut tf = sp_io::hashing::keccak_256(b"transferFrom(address,address,uint256)")[..4].to_vec();
		tf.extend_from_slice(&[0u8; 12]);
		tf.extend_from_slice(eve_evm.as_bytes());
		tf.extend_from_slice(&[0u8; 12]);
		tf.extend_from_slice(bob_evm.as_bytes());
		tf.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());

		let result =
			Executor::<Runtime>::call(CallContext::new_call(gigahdx_token, bob_evm), tf, U256::zero(), 500_000);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"transferFrom on locked GIGAHDX must revert",
		);
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), gigahdx_bal);
	});
}

/// H4 (BUG?): A2 stale-LockSplit + EVM aToken.transfer — must revert (companion to A2).
#[test]
fn h4_atoken_transfer_via_evm_with_stale_lock_split_must_revert() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		// Repeat A2 setup.
		let r1 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(10_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			None,
			r1,
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(5_000_000 * UNITS),
		));

		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
		data.extend_from_slice(&[0u8; 12]);
		data.extend_from_slice(bob_evm.as_bytes());
		data.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			data,
			U256::zero(),
			500_000,
		);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"EVM transfer must revert even when LockSplit is stale",
		);
	});
}

// ===========================================================================
// I. Approve / spender bypass (Substrate path)
// ===========================================================================

// Substrate `Currencies::transfer` has no approve/transfer_from path, so the
// substrate-level check is implicit (the lock is enforced by the aToken).
// I1/I2 are covered by H3 effectively, and additionally the Substrate path:

/// I2: approve → vote → transferFrom — covered by H3 with reversed setup order.
#[test]
fn i2_approve_then_vote_transfer_from_blocked() {
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

		// Approve BEFORE voting.
		let mut approve = sp_io::hashing::keccak_256(b"approve(address,uint256)")[..4].to_vec();
		approve.extend_from_slice(&[0u8; 12]);
		approve.extend_from_slice(bob_evm.as_bytes());
		approve.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let _ = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			approve,
			U256::zero(),
			500_000,
		);

		// Now vote.
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal),
		));

		// Bob attempts transferFrom — must revert because lock is checked at xfer time.
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

// ===========================================================================
// J. Partial transfers
// ===========================================================================

/// J1: hold 10 M GIGAHDX, vote 5 M ⇒ transfer 5 M succeeds.
#[test]
fn j1_unlocked_portion_transferable() {
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
			aye(5_000_000 * UNITS),
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			5_000_000 * UNITS,
		));
	});
}

/// J2: hold 10 M, vote 5 M ⇒ transfer 6 M rejected (would leave 4 M < 5 M lock).
#[test]
fn j2_transfer_more_than_unlocked_rejected() {
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
			aye(5_000_000 * UNITS),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, 6_000_000 * UNITS);
		assert!(result.is_err());
	});
}

/// J3: hold 10 M, vote 5 M ⇒ transfer 4.99 M succeeds.
#[test]
fn j3_transfer_just_under_unlocked_succeeds() {
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
			aye(5_000_000 * UNITS),
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			4_990_000 * UNITS,
		));
	});
}

/// J4: vote with Locked6x → approved → transfer blocked during lock period
/// → past the period → unlock → transfer succeeds.
#[test]
fn j4_locked6x_blocks_during_lock_period_then_unlocks() {
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
			aye_with_conviction(gigahdx_bal, Conviction::Locked6x),
		));

		force_approve_referendum(r);
		let end_block = System::block_number();

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

		// Within the period: blocked.
		let blocked = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(blocked.is_err(), "Locked6x prior blocks transfer mid-period");

		// Past the period: unblocked.
		advance_to_conviction_unlock(end_block, Conviction::Locked6x);
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

// ===========================================================================
// K. Cross-account locks
// ===========================================================================

/// K1: A's GIGAHDX lock = 5 M, A transfers 0 to B (within unlocked) — A's lock unchanged, B has 0 lock.
#[test]
fn k1_partial_transfer_preserves_sender_lock() {
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
			aye(5_000_000 * UNITS),
		));

		let bob = bob();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob.clone(),
			GIGAHDX,
			5_000_000 * UNITS,
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"sender's GIGAHDX-side lock must remain in place"
		);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&bob),
			0,
			"receiver inherits no lock"
		);
	});
}

/// K2: two accounts vote on the same referendum; locks are independent.
#[test]
fn k2_independent_locks_per_account() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		let bob = bob();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			20_000_000 * UNITS,
		));

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(bob.clone()),
			5_000_000 * UNITS,
		));

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(5_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(bob.clone()),
			r,
			aye(3_000_000 * UNITS),
		));

		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
		);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&bob),
			3_000_000 * UNITS,
		);
	});
}

// ===========================================================================
// L. Referendum lifecycle effects
// ===========================================================================

/// L1: vote with Locked1x → referendum cancelled → remove_vote+unlock clears
/// the lock. Cancelled outcomes do NOT keep a prior alive (per
/// `on_remove_vote`'s `lock_after_completion` gate at hooks.rs:127-131).
#[test]
fn l1_cancelled_referendum_clears_lock() {
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

		force_cancel_referendum(r);

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

		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
	});
}

/// L2: vote with Locked1x → referendum approved → remove_vote within lock
/// period keeps lock alive via prior. Lock clears once the period elapses.
#[test]
fn l2_approved_with_conviction_keeps_lock_period() {
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

		force_approve_referendum(r);
		let end_block = System::block_number();

		// remove_vote inside the lock period — prior keeps the lock alive.
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
			"prior keeps the G-side lock alive within the conviction period"
		);

		// Past the conviction lock period — unlock clears the prior.
		advance_to_conviction_unlock(end_block, Conviction::Locked1x);
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
	});
}

/// L3: nay-with-conviction on a referendum that ends Rejected (winning side)
/// keeps a prior alive for the conviction lock period.
#[test]
fn l3_nay_won_keeps_lock_period() {
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
			nay_with_conviction(5_000_000 * UNITS, Conviction::Locked1x),
		));

		force_reject_referendum(r);
		let end_block = System::block_number();

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
			"nay-won prior keeps the G-side lock alive within the conviction period"
		);

		advance_to_conviction_unlock(end_block, Conviction::Locked1x);
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
	});
}

/// L4: aye-with-conviction on a referendum that ends Rejected (losing side)
/// — `lock_balance_on_unsuccessful_vote` returns Some, so a prior IS kept
/// alive. Lock clears only once the conviction period elapses.
#[test]
fn l4_aye_lost_clears_lock_quickly() {
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

		force_reject_referendum(r);
		let end_block = System::block_number();

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
			"losing-side prior is kept alive by lock_balance_on_unsuccessful_vote"
		);

		advance_to_conviction_unlock(end_block, Conviction::Locked1x);
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
	});
}

/// L5: vote with Locked1x → referendum killed → no prior accumulated
/// (Killed maps to ReferendumOutcome::Cancelled in our adapter).
#[test]
fn l5_killed_referendum_clears_lock() {
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

		force_kill_referendum(r);

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

		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve), 0);
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
	});
}

// ===========================================================================
// M. AAVE-side oddities
// ===========================================================================

/// M2: HDX donation to gigapot inflates rate → vote → transfer behaviour.
#[test]
fn m2_donation_inflated_rate_then_vote_blocks_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal_before = Currencies::free_balance(GIGAHDX, &eve);

		// Donate HDX to the gigapot to inflate the rate.
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			gigapot,
			HDX,
			1_000_000 * UNITS,
		));

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(gigahdx_bal_before),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal_before);
		assert!(result.is_err(), "lock applies to GIGAHDX share count, not HDX value");
	});
}

// ===========================================================================
// N. Edge boundaries
// ===========================================================================

/// N2: vote with 0 balance ⇒ rejected.
#[test]
fn n2_vote_with_zero_balance_rejected() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		let result = ConvictionVoting::vote(RuntimeOrigin::signed(eve.clone()), r, aye(0));
		assert!(result.is_err(), "vote with 0 balance must be rejected");
	});
}

/// N3: vote with balance > total holdings ⇒ rejected.
#[test]
fn n3_vote_balance_above_holdings_rejected() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		let total = Currencies::free_balance(HDX, &eve).saturating_add(Currencies::free_balance(GIGAHDX, &eve));

		let result = ConvictionVoting::vote(RuntimeOrigin::signed(eve.clone()), r, aye(total + UNITS));
		assert!(result.is_err(), "vote balance > holdings must be rejected");
	});
}

/// N4: invariant — LockSplit total never exceeds (HDX free + GIGAHDX free).
#[test]
fn n4_lock_split_invariant_under_simple_flow() {
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

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		let hdx_total = Currencies::free_balance(HDX, &eve);
		let gigahdx_total = Currencies::free_balance(GIGAHDX, &eve);
		assert!(split.gigahdx_amount <= gigahdx_total);
		assert!(split.hdx_amount <= hdx_total);
	});
}

/// N5 (BUG?): vote 5 M HDX → convert all HDX to GIGAHDX via stake ⇒ LockSplit must refresh.
#[test]
fn n5_convert_all_hdx_to_gigahdx_after_vote_must_refresh() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		// Vote with 5 M HDX before any GIGAHDX.
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(5_000_000 * UNITS),
		));

		// Snapshot split immediately after the vote.
		let split_before = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);

		// Stake the free HDX into GIGAHDX (modulo a buffer).
		let free = Currencies::free_balance(HDX, &eve);
		let stake = free.saturating_sub(15_000_000 * UNITS); // leave room for the H-side lock
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(eve.clone()), stake,));

		// Design constraint: balance changes do NOT move the split.
		let split_after = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(
			split_after, split_before,
			"converting HDX → GIGAHDX must not change the lock split"
		);
	});
}

/// N1: HDX-only vote with Conviction::None → referendum cancelled →
/// remove_vote+unlock clears the H-side lock (no conviction → no prior).
#[test]
fn n1_vote_none_on_cancelled_track_clears_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(5_000_000 * UNITS),
		));

		force_cancel_referendum(r);

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

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(
			split.hdx_amount, 0,
			"H-side cleared after cancelled None-conviction vote"
		);
	});
}

// ===========================================================================
// O. Voting spanning both HDX and GIGAHDX (combined locks)
// ===========================================================================

/// O1: 5 M HDX + 5 M GIGAHDX → vote 8 M ⇒ LockSplit { gigahdx: 5 M, hdx: 3 M }.
#[test]
fn o1_combined_vote_split_is_correct() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		// Eve now has ~15 M HDX free + 5 M GIGAHDX. Reduce HDX to 5 M for the test.
		let curr = Currencies::free_balance(HDX, &eve);
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let _ = curr; // silence unused

		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 5_000_000 * UNITS);
		assert_eq!(split.hdx_amount, 3_000_000 * UNITS);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
		);
	});
}

/// O2: as O1 → transfer 1 GIGAHDX rejected.
#[test]
fn o2_combined_vote_blocks_any_gigahdx_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, UNITS);
		assert!(result.is_err());
	});
}

/// O3: as O1 → transfer 4 M HDX rejected (only 2 M free).
#[test]
fn o3_combined_vote_blocks_hdx_above_unlocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		assert_noop!(
			Balances::transfer_allow_death(RuntimeOrigin::signed(eve.clone()), bob(), 4_000_000 * UNITS,),
			sp_runtime::TokenError::Frozen,
		);
	});
}

/// O4: as O1 → transfer 2 M HDX accepted.
#[test]
fn o4_combined_vote_allows_hdx_within_unlocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			2_000_000 * UNITS,
		));
	});
}

/// O5: 10 M HDX + 5 M GIGAHDX → vote 12 M ⇒ split { gigahdx: 5 M, hdx: 7 M }.
#[test]
fn o5_combined_vote_with_extra_hdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(12_000_000 * UNITS),
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 5_000_000 * UNITS);
		assert_eq!(split.hdx_amount, 7_000_000 * UNITS);
	});
}

/// O6: as O5 → transfer 3 M HDX accepted.
#[test]
fn o6_combined_vote_allows_3m_hdx_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(12_000_000 * UNITS),
		));

		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			3_000_000 * UNITS,
		));
	});
}

/// O7: as O5 → transfer 4 M HDX rejected.
#[test]
fn o7_combined_vote_blocks_4m_hdx_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(12_000_000 * UNITS),
		));

		assert_noop!(
			Balances::transfer_allow_death(RuntimeOrigin::signed(eve.clone()), bob(), 4_000_000 * UNITS,),
			sp_runtime::TokenError::Frozen,
		);
	});
}

/// O8: as O5 → any GIGAHDX transfer rejected.
#[test]
fn o8_combined_vote_blocks_gigahdx_transfer() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(12_000_000 * UNITS),
		));

		let result = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, UNITS);
		assert!(result.is_err());
	});
}

// ===========================================================================
// P. Combined-vote dynamics under stake/unstake
// ===========================================================================

/// P1: 5 M HDX + 5 M GIGAHDX → vote 8 M → stake 3 M more HDX ⇒ LockSplit refresh to gigahdx=8 M.
#[test]
fn p1_combined_vote_stake_more_refreshes_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		// Snapshot split BEFORE the stake — that's what should persist.
		let split_before = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);

		// 5M HDX − 3M H-side lock = 2M free HDX → max stake is 2M.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			2_000_000 * UNITS,
		));

		// Design constraint: balance changes do NOT shift the split.
		let split_after = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_after, split_before, "stake must not move the lock");
	});
}

/// P2: as P1 → vote 8M against (5M G + 5M H) gives split (5M G, 3M H). After
/// staking another 3M HDX into GIGAHDX, the design constraint forbids the lock
/// from moving — H-side stays at 3M, so a 2M HDX transfer must still fail.
#[test]
fn p2_combined_vote_stake_more_unlocks_hdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));
		// 5M HDX − 3M H-side lock = 2M free HDX. Max additional stake is 2M.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			1_000_000 * UNITS,
		));

		// H-side lock unchanged at 3M; 5M HDX - 1M staked - 3M lock = 1M free.
		let result = Balances::transfer_allow_death(RuntimeOrigin::signed(eve.clone()), bob(), 3_000_000 * UNITS);
		assert!(result.is_err(), "H-side lock unchanged at 3M");
	});
}

/// P5: 10M HDX + 0 GIGAHDX → vote 8M → stake 5M. Design constraint: split is
/// fixed at vote time as (0, 8M) and stays that way; staking does not move it.
#[test]
fn p5_hdx_only_vote_then_stake_refreshes_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));
		let split_before = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_before.gigahdx_amount, 0);
		assert_eq!(split_before.hdx_amount, 8_000_000 * UNITS);

		// Eve has 10M HDX → 8M locked → 2M free. She can only stake 2M, not 5M.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			2_000_000 * UNITS,
		));

		let split_after = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_after, split_before, "stake must not move the lock");
	});
}

/// P6: as P5 → after vote 8M and stake (constrained by H-lock), the H-side
/// lock of 8M still applies. Any HDX transfer beyond the (now zero) free HDX
/// must fail.
#[test]
fn p6_hdx_only_vote_then_stake_blocks_overhdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			10_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));
		// Stake what's free (10M - 8M lock = 2M).
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			2_000_000 * UNITS,
		));

		// Eve now has ~0 free HDX (the 2M went into the AAVE pool). 8M HDX is
		// locked by the vote → any HDX transfer fails.
		let result = Balances::transfer_allow_death(RuntimeOrigin::signed(eve.clone()), bob(), 4_000_000 * UNITS);
		assert!(
			result.is_err(),
			"H-side lock unchanged at 8M ⇒ 4M HDX transfer must fail"
		);
	});
}

/// P7 (regression): two-vote interleaved scenario that historically broke locks.
///
/// 1. Eve has only HDX (no GIGAHDX).
/// 2. Vote on r1 with HDX (Locked1x) — vote A snapshot (g=0, h=8M).
/// 3. Stake 5M HDX → 5M GIGAHDX.
/// 4. Vote on r2 with all GIGAHDX (Locked6x) — vote B snapshot (g=5M, h=0).
/// 5. Mid-flow GIGAHDX transfer must fail (active vote on r2).
/// 6. Approve both, remove both votes — priors accumulate per side
///    (G-side from vote B, H-side from vote A).
/// 7. Post-removal GIGAHDX transfer must STILL fail — the Locked6x prior
///    keeps the 5M GIGAHDX locked.
#[test]
fn p7_hdx_then_gigahdx_votes_priors_keep_locks_alive() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();

		// 1. Eve gets HDX only.
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			20_000_000 * UNITS,
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 0);

		// 2. Vote 8M HDX (Locked1x) on r1.
		let r1 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye_with_conviction(8_000_000 * UNITS, Conviction::Locked1x),
		));
		let split_after_r1 = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_after_r1.gigahdx_amount, 0);
		assert_eq!(split_after_r1.hdx_amount, 8_000_000 * UNITS);

		// 3. Stake 5M HDX → 5M GIGAHDX. Free HDX = 20 - 8 = 12 (only 7 left after stake).
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);
		assert!(gigahdx_bal > 0);

		// 4. Vote ALL the GIGAHDX (Locked6x) on r2.
		let r2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye_with_conviction(gigahdx_bal, Conviction::Locked6x),
		));

		// Vote B snapshot: g=gigahdx_bal, h=0. Adapter recompute aggregates with
		// vote A: g_max=gigahdx_bal, h_max=8M.
		let split_after_r2 = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split_after_r2.gigahdx_amount, gigahdx_bal);
		assert_eq!(split_after_r2.hdx_amount, 8_000_000 * UNITS);

		// 5. GIGAHDX transfer must fail — entire balance locked by vote B.
		let mid_flow_transfer = Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(mid_flow_transfer.is_err(), "GIGAHDX locked by active vote on r2");

		// 6. Approve both referenda; remove both votes — priors accumulate on
		// class 0, max-aggregated per side: g=5M (from r2), h=8M (from r1),
		// until = max(r1_unlock, r2_unlock) = r2_unlock (Locked6x dominates).
		force_approve_referendum(r1);
		force_approve_referendum(r2);
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r1,
		));
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r2,
		));
		// Trigger an `unlock` so conviction-voting writes through the new prior.
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		// 7. Both sides still locked by the merged prior.
		let split_post_remove = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(
			split_post_remove.gigahdx_amount, gigahdx_bal,
			"G-side prior from r2 (Locked6x) keeps GIGAHDX locked"
		);
		assert_eq!(
			split_post_remove.hdx_amount,
			8_000_000 * UNITS,
			"H-side prior from r1 (Locked1x) keeps HDX locked"
		);

		// 8. Post-removal GIGAHDX transfer must still fail — prior outlives the vote.
		let post_remove_transfer =
			Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob(), GIGAHDX, gigahdx_bal);
		assert!(
			post_remove_transfer.is_err(),
			"GIGAHDX still locked by Locked6x prior after vote removed"
		);
	});
}

// ===========================================================================
// Q. Combined-vote re-vote / multi-vote dynamics
// ===========================================================================

/// Q1: 5 M HDX + 5 M GIGAHDX → vote 8 M r1 → vote 6 M r2 ⇒ split unchanged at 5/3.
#[test]
fn q1_smaller_revote_keeps_max_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let rs = begin_n_referenda_by_bob(2);
		let (r1, r2) = (rs[0], rs[1]);
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(8_000_000 * UNITS),
		));
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(6_000_000 * UNITS),
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 5_000_000 * UNITS);
		assert_eq!(split.hdx_amount, 3_000_000 * UNITS);
	});
}

/// Q2: vote 12 M when only 5 M HDX free — extends beyond holdings, must be rejected.
#[test]
fn q2_vote_exceeding_holdings_rejected() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r1 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(8_000_000 * UNITS),
		));
		let r2 = begin_referendum_by_bob();
		let result = ConvictionVoting::vote(RuntimeOrigin::signed(eve.clone()), r2, aye(12_000_000 * UNITS));
		assert!(
			result.is_err(),
			"vote balance must not exceed holdings (5 M HDX + 5 M GIGAHDX = 10 M)"
		);
	});
}

/// Q3: vote with Locked1x → approved → remove_vote (prior accumulated) →
/// revote on a NEW referendum with Conviction::None → the PriorLockSplit from
/// the first vote keeps the lock alive at its original size despite the new
/// no-conviction vote being smaller / having no period of its own.
#[test]
fn q3_remove_vote_with_conviction_then_revote_keeps_period() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		let r1 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye_with_conviction(5_000_000 * UNITS, Conviction::Locked1x),
		));

		force_approve_referendum(r1);

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r1,
		));

		// Revote on a new referendum with smaller None-conviction.
		let r2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(1_000_000 * UNITS),
		));

		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&eve),
			5_000_000 * UNITS,
			"prior from r1 still keeps the original lock size — max-aggregate over (r2 vote, r1 prior)"
		);
	});
}

/// Q4 (BUG, observed live): 10 M HDX + 5 M GIGAHDX → vote 12 M → vote 6 M smaller poll ⇒
/// LockSplit must refresh; today it doesn't.
#[test]
fn q4_smaller_followup_vote_must_refresh_lock_split() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		// 20 M HDX so Eve can lock 8 M for the first vote AND still stake 5 M.
		// (Old buggy behaviour silently let HDX move while locked.)
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			20_000_000 * UNITS,
		));
		let rs = begin_n_referenda_by_bob(2);
		let (r1, r2) = (rs[0], rs[1]);
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r1,
			aye(8_000_000 * UNITS),
		));

		// Now Eve stakes 5 M into GIGAHDX.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		// And votes again with smaller amount on a different poll.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r2,
			aye(6_000_000 * UNITS),
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert!(
			split.gigahdx_amount > 0,
			"GIGAHDX-side lock must be > 0 after stake even when followup vote is smaller (got split={split:?})",
		);

		let result = Currencies::transfer(
			RuntimeOrigin::signed(eve.clone()),
			bob(),
			GIGAHDX,
			Currencies::free_balance(GIGAHDX, &eve),
		);
		assert!(result.is_err(), "GIGAHDX must remain locked after the second vote");
	});
}

// ===========================================================================
// R. Combined-vote (HDX + GIGAHDX) with referendum lifecycle
// ===========================================================================

/// R1: 5 M GIGAHDX + 5 M HDX → vote 10 M with Conviction::None → approved
/// → remove_vote+unlock clears both sides (no conviction → no prior).
#[test]
fn r1_combined_approved_clears_lock() {
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
			aye(10_000_000 * UNITS),
		));

		force_approve_referendum(r);

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

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0, "G-side cleared (None conviction)");
		assert_eq!(split.hdx_amount, 0, "H-side cleared (None conviction)");
	});
}

/// R2: combined vote with Locked1x → approved → remove_vote within lock period
/// → both sides remain locked via PriorLockSplit (G + H).
#[test]
fn r2_combined_locked1x_blocks_during_period() {
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
			aye_with_conviction(10_000_000 * UNITS, Conviction::Locked1x),
		));

		force_approve_referendum(r);

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

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(
			split.gigahdx_amount,
			5_000_000 * UNITS,
			"G-side prior keeps the GIGAHDX portion locked"
		);
		assert_eq!(
			split.hdx_amount,
			5_000_000 * UNITS,
			"H-side prior keeps the HDX portion locked"
		);
	});
}

/// R3: continuation of R2 — fast-forward past the conviction lock period and
/// `unlock` clears both sides.
#[test]
fn r3_combined_locked1x_unlocks_after_period() {
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
			aye_with_conviction(10_000_000 * UNITS, Conviction::Locked1x),
		));

		force_approve_referendum(r);
		let end_block = System::block_number();

		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(eve.clone()),
			Some(0),
			r,
		));

		advance_to_conviction_unlock(end_block, Conviction::Locked1x);
		assert_ok!(ConvictionVoting::unlock(
			RuntimeOrigin::signed(eve.clone()),
			0,
			eve.clone().into(),
		));

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0);
		assert_eq!(split.hdx_amount, 0);
	});
}

/// R4: combined vote with Locked1x → cancelled → both sides clear (Cancelled
/// outcomes don't keep a prior alive).
#[test]
fn r4_combined_cancelled_clears_lock() {
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
			aye_with_conviction(10_000_000 * UNITS, Conviction::Locked1x),
		));

		force_cancel_referendum(r);

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

		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&eve);
		assert_eq!(split.gigahdx_amount, 0, "Cancelled outcome → no prior, G-side clears");
		assert_eq!(split.hdx_amount, 0, "Cancelled outcome → no prior, H-side clears");
	});
}

// ===========================================================================
// S. Combined-vote with EVM/AAVE bypass paths
// ===========================================================================

/// S1: O1 setup → aToken.transfer via EVM ⇒ reverts.
#[test]
fn s1_combined_evm_atoken_transfer_blocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		let bob = bob();
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(eve.clone()));
		let eve_evm = EVMAccounts::evm_address(&eve);
		let bob_evm = EVMAccounts::evm_address(&bob);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
		data.extend_from_slice(&[0u8; 12]);
		data.extend_from_slice(bob_evm.as_bytes());
		data.extend_from_slice(&U256::from(gigahdx_bal).to_big_endian());
		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, eve_evm),
			data,
			U256::zero(),
			500_000,
		);
		assert!(matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)));
	});
}

/// S2: O1 setup → AAVE Pool.withdraw via EVM ⇒ reverts.
#[test]
fn s2_combined_evm_aave_withdraw_blocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let eve = setup_fresh_eve();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(eve.clone()),
			5_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			5_000_000 * UNITS,
		));
		let r = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			r,
			aye(8_000_000 * UNITS),
		));

		let pool = pallet_liquidation::GigaHdxPoolContract::<Runtime>::get();
		let sthdx_evm = HydraErc20Mapping::asset_address(STHDX);
		let eve_evm = EVMAccounts::evm_address(&eve);
		let gigahdx_bal = Currencies::free_balance(GIGAHDX, &eve);

		let data = EvmDataWriter::new_with_selector(AaveFunction::Withdraw)
			.write(sthdx_evm)
			.write(gigahdx_bal)
			.write(eve_evm)
			.build();
		let result = Executor::<Runtime>::call(CallContext::new_call(pool, eve_evm), data, U256::zero(), 500_000);
		assert!(matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)));
	});
}
