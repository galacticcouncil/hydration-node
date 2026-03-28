use crate::polkadot_test_net::hydra_live_ext;
use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, HDX};
use frame_support::{
	assert_noop, assert_ok,
	traits::{OnInitialize, StorePreimage},
};
use frame_system::RawOrigin;
use hydradx_runtime::{
	Balances, ConvictionVoting, Currencies, Democracy, GigaHdx, Preimage, Referenda, RuntimeOrigin, Scheduler, System,
};
use orml_traits::MultiCurrency;
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use primitives::constants::time::DAYS;
use primitives::Balance;
use sp_runtime::{DispatchError, TokenError};
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/gigahdx/gigahdx";

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
		assert_eq!(Balances::free_balance(&alice), 0, "After staking, alice should have 0 HDX");
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 400 * UNITS, "After staking, alice should have 400 GIGAHDX");

		// First unstake 100 GIGAHDX -> 300 GIGAHDX, ~100 locked HDX, 0 usable HDX
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 300 * UNITS, "After first unstake, alice should have 300 GIGAHDX");
		let hdx_after_first = Balances::free_balance(&alice);
		assert!(hdx_after_first > 0, "After first unstake, alice should have received HDX");
		assert_eq!(Balances::usable_balance(&alice), 0, "After first unstake, all HDX should be locked");

		// Second unstake 100 GIGAHDX -> 200 GIGAHDX, ~200 locked HDX, 0 usable HDX
		System::set_block_number(System::block_number() + 1);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 200 * UNITS, "After second unstake, alice should have 200 GIGAHDX");
		let hdx_after_second = Balances::free_balance(&alice);
		assert!(hdx_after_second > hdx_after_first, "After second unstake, alice should have more HDX");

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
		let lock_id_2 = positions[2].lock_id;

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

		assert_noop!(
			Balances::transfer_allow_death(RuntimeOrigin::signed(alice.clone()), bob.clone(), 10 * UNITS),
			TokenError::FundsUnavailable
		);

		let final_positions = pallet_gigahdx::UnstakePositions::<hydradx_runtime::Runtime>::get(&alice);
		let new_lock_id = final_positions[2].lock_id;
		assert_ne!(new_lock_id, lock_id_2);
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
