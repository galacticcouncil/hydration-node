use crate::polkadot_test_net::hydra_live_ext;
use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, HDX};
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
use sp_runtime::{DispatchError, TokenError};
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/gigahdx/gigahdx2";

const UNITS: Balance = 1_000_000_000_000;
const STHDX: u32 = 670;
const GIGAHDX: u32 = 67;

/// Requires snapshot with stHDX registered as an AAVE reserve and
/// GIGAHDX configured as the corresponding aToken.
#[test]
fn giga_stake_should_work_on_mainnet_snapshot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let stake_amount = 1_000 * UNITS;

		// Give ALICE some HDX
		assert_ok!(<hydradx_runtime::Currencies as MultiCurrency<_>>::deposit(
			HDX,
			&alice,
			10_000 * UNITS
		));

		let gigapot = GigaHdx::gigapot_account_id();

		let hdx_before = Currencies::free_balance(HDX, &alice);
		let gigapot_hdx_before = Currencies::free_balance(HDX, &gigapot);

		// Stake 1000 HDX
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));

		// HDX transferred from ALICE to gigapot
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_before - stake_amount);
		assert_eq!(
			Currencies::free_balance(HDX, &gigapot),
			gigapot_hdx_before + stake_amount
		);

		// stHDX minted and supplied to AAVE — alice should not hold any
		assert_eq!(Currencies::free_balance(STHDX, &alice), 0);

		// GIGAHDX (aToken) received by alice via real AAVE supply
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);

		// Exchange rate and totals updated correctly
		assert_eq!(GigaHdx::total_hdx(), stake_amount);
		assert_eq!(GigaHdx::total_st_hdx_supply(), stake_amount);
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_u32(1));
	});
}

// ---------------------------------------------------------------------------
// Helpers for snapshot-based voting tests
// ---------------------------------------------------------------------------

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

fn aye_with_conviction(amount: u128, conviction: Conviction) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: true, conviction },
		balance: amount,
	}
}

/// Helper: set up ALICE with GIGAHDX by staking all her HDX.
/// Returns the stake amount.
fn setup_alice_with_only_gigahdx(stake_amount: Balance) {
	let alice = sp_runtime::AccountId32::from(ALICE);

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		alice.clone(),
		stake_amount,
	));
	assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount,));
}

/// Helper: create a referendum submitted by BOB.
fn begin_referendum_by_bob() -> u32 {
	let bob = sp_runtime::AccountId32::from(BOB);
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<hydradx_runtime::Runtime>::get();
	let now = System::block_number();

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		bob.clone(),
		1_000_000 * UNITS,
	));
	let proposal = {
		let inner = pallet_balances::Call::force_set_balance {
			who: sp_runtime::AccountId32::from(CHARLIE),
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
		sp_runtime::AccountId32::from(DAVE),
		2_000_000_000 * UNITS,
	));
	assert_ok!(Referenda::place_decision_deposit(
		RuntimeOrigin::signed(sp_runtime::AccountId32::from(DAVE)),
		referendum_index,
	));

	fast_forward_to(now + 5 * DAYS);

	referendum_index
}

/// User with zero HDX can vote using only GIGAHDX received from giga_stake.
#[test]
fn vote_with_only_gigahdx_on_mainnet_snapshot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let stake_amount = 1_000 * UNITS;

		setup_alice_with_only_gigahdx(stake_amount);

		assert_eq!(Currencies::free_balance(HDX, &alice), 0);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);

		let referendum_index = begin_referendum_by_bob();

		// ALICE votes with GIGAHDX only (0 HDX).
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			referendum_index,
			aye(stake_amount),
		));

		// Vote should be recorded in GigaHdxVotes.
		let vote = pallet_gigahdx_voting::GigaHdxVotes::<hydradx_runtime::Runtime>::get(&alice, referendum_index);
		assert!(vote.is_some(), "GIGAHDX vote should be recorded");
		assert_eq!(vote.unwrap().amount, stake_amount);

		// LockSplit should show entire amount as gigahdx (no HDX portion).
		let split = pallet_gigahdx_voting::LockSplit::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(split.gigahdx_amount, stake_amount);
		assert_eq!(split.hdx_amount, 0);
	});
}

#[test]
fn direct_hdx_transfer_to_gigapot_inflates_exchange_rate() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let gigapot = GigaHdx::gigapot_account_id();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			gigapot.clone(),
			UNITS
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			1_000_000 * UNITS
		));

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_rational(101, 100));

		//Act
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			gigapot.clone(),
			HDX,
			1_000 * UNITS
		));

		//Assert
		let rate_after = GigaHdx::exchange_rate();
		assert_eq!(rate_after, sp_runtime::FixedU128::from_rational(1101, 100));

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));
		let bob_gigahdx = Currencies::free_balance(GIGAHDX, &bob);
		assert!(
			bob_gigahdx < 10 * UNITS,
			"BOB should receive far fewer GIGAHDX due to inflated rate: got {}",
			bob_gigahdx
		);

		let charlie = sp_runtime::AccountId32::from(CHARLIE);
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			1_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(charlie), 10 * UNITS));
	});
}

#[test]
fn giga_unstake_works_at_extreme_exchange_rate() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice = sp_runtime::AccountId32::from(ALICE);
		let gigapot = GigaHdx::gigapot_account_id();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 100 * UNITS);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			gigapot.clone(),
			1_000_000_000_000_000 * UNITS,
		));

		//Act
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		//Assert
		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1);
		assert_eq!(positions[0].amount, 1_000_000_000_000_000 * UNITS);
	});
}

#[test]
fn restake_works_after_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			1_000_000 * UNITS
		));

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(GigaHdx::total_st_hdx_supply(), 100 * UNITS);

		//Act
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		//Assert
		assert_eq!(GigaHdx::total_st_hdx_supply(), 0);
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_u32(1));

		//Act
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));

		//Assert
		assert_eq!(Currencies::free_balance(GIGAHDX, &bob), 100 * UNITS);
		assert_eq!(GigaHdx::total_st_hdx_supply(), 100 * UNITS);
	});
}

/// BUG: Two concurrent unstake positions don't stack locks.
/// set_lock uses max(locks) not sum(locks), so after two unstakes of 100 HDX each,
/// only 100 is locked instead of 200 — the other 100 is freely spendable.
#[test]
fn second_unstake_makes_first_unstake_amount_usable() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let gigapot = GigaHdx::gigapot_account_id();

		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), gigapot, UNITS));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			400 * UNITS
		));

		// Stake 400 HDX -> 0 HDX, 400 GIGAHDX
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 400 * UNITS));
		assert_eq!(
			Balances::free_balance(&alice),
			0,
			"After staking, alice should have 0 HDX"
		);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			400 * UNITS,
			"After staking, alice should have 400 GIGAHDX"
		);

		// First unstake 100 GIGAHDX -> 300 GIGAHDX, ~100 locked HDX, 0 usable HDX
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			300 * UNITS,
			"After first unstake, alice should have 300 GIGAHDX"
		);
		let hdx_after_first = Balances::free_balance(&alice);
		assert!(
			hdx_after_first > 0,
			"After first unstake, alice should have received HDX"
		);
		assert_eq!(
			Balances::usable_balance(&alice),
			0,
			"After first unstake, all HDX should be locked"
		);

		// Second unstake 100 GIGAHDX -> 200 GIGAHDX, ~200 locked HDX, 0 usable HDX
		System::set_block_number(System::block_number() + 1);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			200 * UNITS,
			"After second unstake, alice should have 200 GIGAHDX"
		);
		let hdx_after_second = Balances::free_balance(&alice);
		assert!(
			hdx_after_second > hdx_after_first,
			"After second unstake, alice should have more HDX"
		);

		// BUG: alice should NOT be able to transfer 100 HDX, but she can because only ~100 is effectively locked
		assert_noop!(
			Balances::transfer_allow_death(RuntimeOrigin::signed(alice.clone()), bob.clone(), 100 * UNITS),
			TokenError::Frozen
		);

		// BUG: should be 200 HDX locked, but only 100 is locked due to set_lock using max(locks) not sum(locks)
		let locks = pallet_balances::Locks::<hydradx_runtime::Runtime>::get(&alice);
		let total_locked: u128 = locks.iter().map(|l| l.amount).sum();
		assert_eq!(total_locked, 200_500_000_000_000, "All received HDX should be locked");

		// BUG: usable is ~100 instead of 0 because set_lock uses max(locks) not sum(locks)
		let usable = Balances::usable_balance(&alice);
		assert_eq!(usable, 0, "After second unstake, all HDX should be locked, 0 usable");
	});
}

/// Lock ID collision: lock_id is derived from positions.len() at creation time.
/// After partial unlock removes an earlier position, the next unstake could generate
/// a lock_id that collides with an existing position's lock_id, freeing HDX early.
#[test]
fn lock_id_collides_after_partial_unlock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let gigapot = GigaHdx::gigapot_account_id();

		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), gigapot, UNITS));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			400 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 400 * UNITS));
		assert_eq!(Balances::free_balance(&alice), 0);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		System::set_block_number(System::block_number() + 1);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		System::set_block_number(System::block_number() + 1);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);

		System::set_block_number(positions[0].unlock_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), alice.clone()));

		let usable_after_unlock = Balances::usable_balance(&alice);
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			usable_after_unlock
		));

		let usable_before = Balances::usable_balance(&alice);
		assert_eq!(usable_before, 0);

		//Act
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS));

		//Assert

		// BUG: due to lock_id collision, the old lock was overwritten with a smaller amount, so usable INCREASED, but it should not
		let usable_after = Balances::usable_balance(&alice);
		assert_eq!(usable_after, 0);

		// With the single-aggregate-lock fix the balance exists but is fully
		// frozen — the runtime reports Frozen, not FundsUnavailable.
		assert_noop!(
			Balances::transfer_allow_death(RuntimeOrigin::signed(alice.clone()), bob.clone(), 10 * UNITS),
			TokenError::Frozen
		);
	});
}

#[test]
fn unstake_more_than_balance_fails() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);
		let hdx_before = Balances::free_balance(&alice);

		//Act & Assert
		assert!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 200 * UNITS).is_err());

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_before);
		assert_eq!(Balances::free_balance(&alice), hdx_before);

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 0);
	});
}

#[test]
fn giga_stake_at_min_amount_succeeds_and_below_min_fails() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//Arrange
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));

		//Act & Assert
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS));
		assert!(Currencies::free_balance(GIGAHDX, &alice) > 0);

		assert!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS - 1).is_err());
	});
}

/// GIGAHDX can be transferred when not locked by voting.
#[test]
fn gigahdx_transfer_succeeds_when_unlocked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let stake_amount = 1_000 * UNITS;
		let transfer_amount = 400 * UNITS;

		setup_alice_with_only_gigahdx(stake_amount);

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);

		// Transfer GIGAHDX to BOB — no voting lock, should succeed.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(alice.clone()),
			bob.clone(),
			GIGAHDX,
			transfer_amount,
		));

		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			stake_amount - transfer_amount
		);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &bob),
			bob_gigahdx_before + transfer_amount
		);
	});
}

/// GIGAHDX transfer fails when the balance is locked by a conviction vote.
#[test]
fn gigahdx_transfer_fails_when_locked_by_conviction_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let stake_amount = 1_000 * UNITS;

		setup_alice_with_only_gigahdx(stake_amount);

		let referendum_index = begin_referendum_by_bob();

		// ALICE votes with Locked1x conviction — locks all her GIGAHDX.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			referendum_index,
			aye_with_conviction(stake_amount, Conviction::Locked1x),
		));

		// Verify the lock is set.
		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(lock, stake_amount);

		// Transferring any GIGAHDX should fail — entire balance is locked.
		assert!(Currencies::transfer(RuntimeOrigin::signed(alice.clone()), bob.clone(), GIGAHDX, 1 * UNITS,).is_err());
	});
}

/// Unstaking that would leave a non-zero but sub-MinStake GIGAHDX position is rejected.
#[test]
fn giga_unstake_fails_when_remaining_below_min_stake() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		// Stake 100 UNITS → receive ~100 UNITS GIGAHDX at current rate.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		assert!(gigahdx_balance > 0);

		// Unstake all but 1 UNIT of GIGAHDX. The remaining 1 UNIT is worth < MinStake (10 UNITS).
		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), gigahdx_balance - UNITS),
			pallet_gigahdx::Error::<hydradx_runtime::Runtime>::RemainingBelowMinStake
		);

		// Balances unchanged after failed call.
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_balance);
	});
}

/// Unstaking that leaves a GIGAHDX position worth >= MinStake in HDX succeeds.
#[test]
fn giga_unstake_partial_succeeds_when_remaining_meets_min_stake() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		// Stake 100 UNITS → receive ~100 UNITS GIGAHDX.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);

		// Unstake half — remaining ~50 UNITS GIGAHDX is worth >= MinStake (10 UNITS).
		let unstake_amount = gigahdx_balance / 2;
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			unstake_amount
		));

		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			gigahdx_balance - unstake_amount
		);
	});
}

/// Full exit (unstaking entire GIGAHDX balance) is always permitted regardless of MinStake.
#[test]
fn giga_unstake_full_exit_always_succeeds() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS
		));
		// Stake exactly MinStake to get the smallest valid GIGAHDX position.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS));

		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		assert!(gigahdx_balance > 0);

		// Unstaking everything (remaining == 0) must always succeed.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_balance
		));

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 0);
	});
}

/// direct HDX donation to gigapot does not break unstake or rate math
/// against the real AAVE money market. Existing coverage:
/// `direct_hdx_transfer_to_gigapot_inflates_exchange_rate` proves the rate
/// updates; this test extends coverage by verifying that an existing staker
/// can still complete a full unstake AFTER the donation, with the inflated
/// payout, and that running into the cooldown lock works as expected.
#[test]
fn donation_does_not_break_unstake_payout_on_real_aave() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let gigapot = GigaHdx::gigapot_account_id();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			gigapot.clone(),
			UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000_000 * UNITS,
		));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			1_000_000 * UNITS,
		));

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let gigahdx_minted = Currencies::free_balance(GIGAHDX, &alice);
		assert!(gigahdx_minted > 0);

		// BOB grief-donates a lot of HDX directly to the gigapot.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			gigapot.clone(),
			HDX,
			500 * UNITS,
		));

		let rate_after = GigaHdx::exchange_rate();
		assert!(
			rate_after > sp_runtime::FixedU128::from(1),
			"donation must inflate the rate"
		);

		// ALICE can still unstake fully — the donation is a *bonus* to her,
		// not a denial-of-service.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_minted
		));

		let positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		assert_eq!(positions.len(), 1);
		// Payout should reflect the inflated rate — strictly more than her stake.
		assert!(
			positions[0].amount > 100 * UNITS,
			"unstaker received the donation as a bonus: got {} (staked 100 UNITS)",
			positions[0].amount
		);
	});
}

// ---------------------------------------------------------------------------
// Voting-lock enforcement on EVM-level paths that bypass the pallet.
//
// The Substrate `Currencies::transfer` path is already covered by
// `gigahdx_transfer_fails_when_locked_by_conviction_vote`. The two tests
// below exercise the corresponding EVM paths:
//   1. AAVE `Pool.withdraw(stHDX, amount, to)` — burns aTokens for underlying.
//   2. aToken (GIGAHDX) `transfer(to, amount)` — direct ERC20 transfer.
//
// Both are expected to revert when the user's GIGAHDX is locked by an
// active conviction vote (read via the LockManager precompile at 0x0806).
// If they don't, the lock can be bypassed at the EVM layer — see
// ---------------------------------------------------------------------------

const STHDX_AAVE_RESERVE: u32 = STHDX;

/// Build calldata for AAVE Pool.withdraw(asset, amount, to).
fn build_aave_withdraw_calldata(asset: sp_core::H160, amount: Balance, to: sp_core::H160) -> Vec<u8> {
	EvmDataWriter::new_with_selector(AaveFunction::Withdraw)
		.write(asset)
		.write(amount)
		.write(to)
		.build()
}

/// Build calldata for ERC20 transfer(to, amount). Uses keccak256 directly to
/// avoid pulling in the runtime-internal `erc20_currency::Function` enum,
/// which would couple the test to a private path.
fn build_erc20_transfer_calldata(to: sp_core::H160, amount: Balance) -> Vec<u8> {
	let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
	// 32-byte padded address
	data.extend_from_slice(&[0u8; 12]);
	data.extend_from_slice(to.as_bytes());
	// 32-byte amount
	data.extend_from_slice(&U256::from(amount).to_big_endian());
	data
}

/// Decodes `Error(string)` and the `NotEnoughAvailableUserBalance(uint256,uint256)`
/// custom error emitted by the LockableAToken hook. Used to ensure an asserted
/// revert came from the lock-rejection path, not an incidental gas-out.
fn decode_evm_revert(value: &[u8]) -> Result<String, String> {
	if value.is_empty() {
		return Err("revert with no data (gas-out, no contract code, or revert() with no reason)".into());
	}
	if value.len() < 4 {
		return Err(format!("revert payload too short: 0x{}", hex::encode(value)));
	}
	let selector = &value[..4];

	if selector == [0x08, 0xc3, 0x79, 0xa0] {
		if value.len() < 4 + 64 {
			return Err(format!("Error(string) header truncated: 0x{}", hex::encode(value)));
		}
		let len = U256::from_big_endian(&value[36..68]).low_u64() as usize;
		if value.len() < 68 + len {
			return Err(format!("Error(string) body truncated: 0x{}", hex::encode(value)));
		}
		let msg = core::str::from_utf8(&value[68..68 + len])
			.map_err(|e| format!("Error(string) body not utf-8: {e:?}"))?;
		return Ok(format!("Error(string): {msg:?}"));
	}

	if selector == [0x9e, 0x17, 0x6a, 0xc9] {
		if value.len() < 4 + 64 {
			return Err(format!(
				"NotEnoughAvailableUserBalance args truncated: 0x{}",
				hex::encode(value)
			));
		}
		let amount = U256::from_big_endian(&value[4..36]);
		let user_balance = U256::from_big_endian(&value[36..68]);
		return Ok(format!(
			"NotEnoughAvailableUserBalance(amount={amount}, userBalance={user_balance})"
		));
	}

	Err(format!(
		"unrecognised custom-error selector 0x{}; raw=0x{}",
		hex::encode(selector),
		hex::encode(value),
	))
}

/// Rejects empty reverts that would otherwise pattern-match `Revert(_)`
/// and mask a regression on a non-lock path.
#[track_caller]
fn assert_evm_reverted_with_reason(result: &hydradx_traits::evm::CallResult, context: &str) -> String {
	assert!(
		matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
		"{context}: expected Revert, got exit_reason={:?}, data=0x{}",
		result.exit_reason,
		hex::encode(&result.value),
	);
	match decode_evm_revert(&result.value) {
		Ok(reason) => {
			println!("[{context}] revert reason: {reason:?}");
			reason
		}
		Err(detail) => panic!(
			"{context}: revert had no usable reason — {detail}. \
			 An empty/unknown revert payload usually means gas exhaustion or a missing \
			 contract, not the lock-rejection path the test is asserting.",
		),
	}
}

/// Stake `stake_amount` HDX for ALICE and lock it all under a Locked6x vote.
/// Reuses the proven `setup_alice_with_only_gigahdx` flow used by other
/// snapshot-based tests in this file.
/// Returns ALICE's `(substrate_account, evm_address, gigahdx_balance)`.
fn setup_alice_with_locked_gigahdx(stake_amount: Balance) -> (sp_runtime::AccountId32, sp_core::H160, Balance) {
	setup_alice_with_only_gigahdx(stake_amount);

	let alice = sp_runtime::AccountId32::from(ALICE);
	let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
	assert!(
		gigahdx_balance > 0,
		"stake must produce GIGAHDX — snapshot may not have AAVE configured for stHDX",
	);

	let referendum_index = begin_referendum_by_bob();
	assert_ok!(ConvictionVoting::vote(
		RuntimeOrigin::signed(alice.clone()),
		referendum_index,
		AccountVote::Standard {
			vote: Vote {
				aye: true,
				conviction: Conviction::Locked6x,
			},
			balance: gigahdx_balance,
		},
	));
	assert_eq!(
		pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&alice),
		gigahdx_balance,
		"entire GIGAHDX balance must be locked by the conviction vote",
	);

	// `giga_stake` binds the EVM address internally during MM supply, so this
	// just reads the now-bound mapping.
	let alice_evm = EVMAccounts::evm_address(&alice);
	(alice, alice_evm, gigahdx_balance)
}

/// AAVE `Pool.withdraw` against a user's locked GIGAHDX must revert. Otherwise
/// the user can drain aTokens for underlying stHDX without going through
/// `giga_unstake`, sidestepping cooldowns and rewards bookkeeping.
#[test]
fn aave_withdraw_outside_giga_unstake_respects_voting_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let stake_amount = 1_000 * UNITS;
		let (alice, alice_evm, gigahdx_balance) = setup_alice_with_locked_gigahdx(stake_amount);

		let pool = pallet_liquidation::GigaHdxPoolContract::<Runtime>::get();
		let sthdx_evm = HydraErc20Mapping::asset_address(STHDX_AAVE_RESERVE);
		let sthdx_before = Currencies::free_balance(STHDX as u32, &alice);

		// Attempt the withdraw directly via EVM — bypasses `giga_unstake`
		// entirely.
		let data = build_aave_withdraw_calldata(sthdx_evm, gigahdx_balance, alice_evm);
		let result = Executor::<Runtime>::call(CallContext::new_call(pool, alice_evm), data, U256::zero(), 500_000);

		let _reason =
			assert_evm_reverted_with_reason(&result, "AAVE withdraw on locked GIGAHDX (lock-manager not honored?)");

		// State invariants — nothing moved.
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			gigahdx_balance,
			"GIGAHDX balance must be unchanged after rejected withdraw",
		);
		assert_eq!(
			Currencies::free_balance(STHDX as u32, &alice),
			sthdx_before,
			"no stHDX should have been credited",
		);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&alice),
			gigahdx_balance,
			"voting lock must still be in place",
		);
	});
}

/// aToken (GIGAHDX) `transfer` via the ERC20 EVM interface must revert when
/// the source's balance is locked by a conviction vote. The Substrate
/// `Currencies::transfer` path is already enforced by
/// `gigahdx_transfer_fails_when_locked_by_conviction_vote`; this proves the
/// EVM path has equivalent enforcement.
#[test]
fn vote_then_transfer_atoken_via_evm_blocked_when_locked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let stake_amount = 1_000 * UNITS;
		let (alice, alice_evm, gigahdx_balance) = setup_alice_with_locked_gigahdx(stake_amount);

		let bob = sp_runtime::AccountId32::from(BOB);
		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let bob_evm = EVMAccounts::evm_address(&bob);
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);

		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		// Try to transfer the entire locked balance via the ERC20 EVM call.
		let data = build_erc20_transfer_calldata(bob_evm, gigahdx_balance);
		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, alice_evm),
			data,
			U256::zero(),
			500_000,
		);

		let _reason = assert_evm_reverted_with_reason(&result, "GIGAHDX aToken transfer of locked balance");

		// State invariants — nothing moved.
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &alice),
			gigahdx_balance,
			"sender's GIGAHDX must be unchanged",
		);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &bob),
			bob_gigahdx_before,
			"recipient's GIGAHDX must be unchanged",
		);
	});
}

/// Control for `vote_then_transfer_atoken_via_evm_blocked_when_locked` —
/// proves the same EVM transfer succeeds when there is no conviction lock,
/// so the locked variant is reverting because of the lock and not for
/// some unrelated reason.
#[test]
fn transfer_atoken_via_evm_succeeds_when_not_locked() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let stake_amount = 1_000 * UNITS;
		let transfer_amount = 400 * UNITS;

		setup_alice_with_only_gigahdx(stake_amount);
		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(gigahdx_balance, stake_amount);
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&alice), 0);

		assert_ok!(Balances::force_set_balance(RawOrigin::Root.into(), bob.clone(), UNITS,));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let bob_evm = EVMAccounts::evm_address(&bob);
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);

		let alice_evm = EVMAccounts::evm_address(&alice);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		let data = build_erc20_transfer_calldata(bob_evm, transfer_amount);
		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, alice_evm),
			data,
			U256::zero(),
			500_000,
		);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"unlocked GIGAHDX aToken transfer must succeed. exit_reason={:?}, data=0x{}",
			result.exit_reason,
			hex::encode(&result.value),
		);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_balance - transfer_amount);
		assert_eq!(Currencies::free_balance(GIGAHDX, &bob), bob_gigahdx_before + transfer_amount);
	});
}
