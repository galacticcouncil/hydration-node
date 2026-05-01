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
use sp_runtime::TokenError;
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/gigahdx/gigahdx_slim_2lark";

const UNITS: Balance = 1_000_000_000_000;
const STHDX: u32 = 670;
const GIGAHDX: u32 = 67;

pub fn reset_giga_state_for_fixture() {
	orml_tokens::TotalIssuance::<Runtime>::set(670, 0);
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		GigaHdx::gigapot_account_id(),
		0,
	));
}

/// Requires snapshot with stHDX registered as an AAVE reserve and
/// GIGAHDX configured as the corresponding aToken.
#[test]
fn giga_stake_should_mint_gigahdx_on_mainnet_snapshot() {
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
		let total_hdx_before = GigaHdx::total_hdx();
		let total_st_hdx_before = GigaHdx::total_st_hdx_supply();

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

		// Totals incremented by the staked amount (clean 1:1 rate after bootstrap).
		assert_eq!(GigaHdx::total_hdx(), total_hdx_before + stake_amount);
		assert_eq!(GigaHdx::total_st_hdx_supply(), total_st_hdx_before + stake_amount);
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
fn vote_should_succeed_with_only_gigahdx_balance_on_mainnet_snapshot() {
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
fn exchange_rate_should_inflate_when_hdx_transferred_directly_to_gigapot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
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

		// A new staker can still participate after the rate inflation. Stake size
		// must produce above-ED stHDX given the inflated rate (~11x), so 10 UNITS
		// would mint sub-ED stHDX and fail.
		let charlie = sp_runtime::AccountId32::from(CHARLIE);
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			1_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(charlie), 100 * UNITS));
	});
}

#[test]
fn giga_unstake_should_succeed_at_extreme_exchange_rate() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
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
fn restake_should_succeed_after_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
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
fn first_unstake_amount_should_become_usable_when_second_unstake_executed() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
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
fn lock_id_should_collide_after_partial_unlock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
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
fn giga_unstake_should_fail_when_amount_exceeds_balance() {
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
fn giga_stake_should_succeed_at_min_amount_and_fail_below() {
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
fn gigahdx_transfer_should_succeed_when_unlocked() {
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
fn gigahdx_transfer_should_fail_when_locked_by_conviction_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// Use a fresh account that has no prior state on the testnet snapshot.
		const EVE: [u8; 32] = [99u8; 32];
		let eve = sp_runtime::AccountId32::from(EVE);
		let bob = sp_runtime::AccountId32::from(BOB);
		let stake_amount = 1_000 * UNITS;

		// Sanity: EVE must be empty on the snapshot.
		assert_eq!(Currencies::free_balance(GIGAHDX, &eve), 0);
		assert_eq!(Currencies::free_balance(HDX, &eve), 0);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			eve.clone(),
			stake_amount,
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(eve.clone()), stake_amount));

		let referendum_index = begin_referendum_by_bob();

		// EVE votes with Locked1x conviction — locks all her GIGAHDX.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(eve.clone()),
			referendum_index,
			aye_with_conviction(stake_amount, Conviction::Locked1x),
		));

		// Verify the lock is set.
		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<hydradx_runtime::Runtime>::get(&eve);
		assert_eq!(lock, stake_amount);

		// Transferring any GIGAHDX should fail — entire balance is locked.
		assert!(Currencies::transfer(RuntimeOrigin::signed(eve.clone()), bob.clone(), GIGAHDX, 1 * UNITS,).is_err());
	});
}

/// Unstaking that would leave a non-zero but sub-MinStake GIGAHDX position is rejected.
#[test]
fn giga_unstake_should_fail_when_remaining_below_min_stake_on_real_aave() {
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
fn giga_unstake_partial_should_succeed_when_remaining_meets_min_stake() {
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
fn giga_unstake_should_succeed_when_full_exit() {
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
/// `exchange_rate_should_inflate_when_hdx_transferred_directly_to_gigapot` proves the rate
/// updates; this test extends coverage by verifying that an existing staker
/// can still complete a full unstake AFTER the donation, with the inflated
/// payout, and that running into the cooldown lock works as expected.
#[test]
fn unstake_payout_should_succeed_after_donation_on_real_aave() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
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
// `gigahdx_transfer_should_fail_when_locked_by_conviction_vote`. The two tests
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
fn aave_withdraw_should_respect_voting_lock_when_called_outside_giga_unstake() {
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

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"AAVE withdraw on locked GIGAHDX must revert (lock-manager not honored?). \
			 exit_reason={:?}, data={}",
			result.exit_reason,
			hex::encode(&result.value),
		);

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
/// `gigahdx_transfer_should_fail_when_locked_by_conviction_vote`; this proves the
/// EVM path has equivalent enforcement.
#[test]
fn atoken_evm_transfer_should_fail_when_locked_by_vote() {
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

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"GIGAHDX aToken transfer of locked balance must revert. \
			 exit_reason={:?}, data={}",
			result.exit_reason,
			hex::encode(&result.value),
		);

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

// ---------------------------------------------------------------------------
// LockSplit-stale repro
//
// Reproduces the testnet observation that a user who voted with HDX before
// holding any GIGAHDX, then later staked HDX → GIGAHDX and voted again, can
// transfer their GIGAHDX even though it should be lock-bound by the second
// vote.
//
// Exact testnet sequence as observed against Lark2 (account 5CSXEX...BCH7):
//   1. fund account with 20 M HDX
//   2. vote 10 M aye-None on a referendum  (no GIGAHDX yet)
//   3. remove that vote
//   4. giga_stake 5 M HDX → 5 M GIGAHDX
//   5. vote 5 M aye-None on a referendum  (now holding only GIGAHDX)
//   6. transfer 5 M GIGAHDX out — succeeds despite step-5 vote (BUG)
//
// Expected post-condition (correct behaviour) would be a transfer revert with
// the precompile reporting locked balance == 5 M. Current behaviour leaves the
// LockSplit at { gigahdx: 0, hdx: 10 M } from step 2 because the lock-split
// allocator is never re-run with the post-stake GIGAHDX balance.
// ---------------------------------------------------------------------------
#[test]
fn lock_split_should_remain_at_snapshot_when_stake_increases_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let charlie = sp_runtime::AccountId32::from(CHARLIE);
		let bob = sp_runtime::AccountId32::from(BOB);

		// Sanity: charlie has no relevant state in this snapshot.
		assert_eq!(Currencies::free_balance(HDX, &charlie), 0);
		assert_eq!(Currencies::free_balance(GIGAHDX, &charlie), 0);
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&charlie), 0);
		let split_pre = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&charlie);
		assert_eq!(split_pre.gigahdx_amount, 0);
		assert_eq!(split_pre.hdx_amount, 0);

		// Step 1: fund Charlie with 20 M HDX.
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			20_000_000 * UNITS,
		));
		assert_eq!(Currencies::free_balance(HDX, &charlie), 20_000_000 * UNITS);

		// Step 2: Charlie votes 10 M aye-None before staking any GIGAHDX.
		let ref_index_1 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(charlie.clone()),
			ref_index_1,
			aye(10_000_000 * UNITS),
		));

		// Pre-stake invariant: split is HDX-only because GIGAHDX balance is 0.
		let split_after_step2 = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&charlie);
		assert_eq!(split_after_step2.gigahdx_amount, 0);
		assert_eq!(split_after_step2.hdx_amount, 10_000_000 * UNITS);
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&charlie), 0);

		// Step 3: Charlie removes that vote.
		assert_ok!(ConvictionVoting::remove_vote(
			RuntimeOrigin::signed(charlie.clone()),
			None,
			ref_index_1,
		));

		// Step 4: Charlie stakes 5 M HDX → ~5 M GIGAHDX via the real money market.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(charlie.clone()),
			5_000_000 * UNITS,
		));
		let charlie_gigahdx = Currencies::free_balance(GIGAHDX, &charlie);
		assert!(
			charlie_gigahdx > 0,
			"giga_stake must mint GIGAHDX (got {})",
			charlie_gigahdx
		);

		// Step 5: Charlie votes 5 M aye-None on a fresh referendum, now that he
		// holds GIGAHDX.
		let ref_index_2 = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(charlie.clone()),
			ref_index_2,
			aye(5_000_000 * UNITS),
		));

		// Diagnostic snapshot of the lock state right after step 5.
		let split_after_step5 = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&charlie);
		let lock_after_step5 = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&charlie);
		println!(
			"[diag] after step 5: LockSplit {{ gigahdx: {}, hdx: {} }}, GigaHdxVotingLock = {}",
			split_after_step5.gigahdx_amount, split_after_step5.hdx_amount, lock_after_step5,
		);

		// Step 6: try to transfer the entire 5 M GIGAHDX out.
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);
		let transfer_result = Currencies::transfer(
			RuntimeOrigin::signed(charlie.clone()),
			bob.clone(),
			GIGAHDX,
			charlie_gigahdx,
		);
		println!("[diag] step 6 transfer result = {:?}", transfer_result);
		println!(
			"[diag] charlie GIGAHDX after = {}",
			Currencies::free_balance(GIGAHDX, &charlie),
		);
		println!(
			"[diag] bob GIGAHDX after = {} (before {})",
			Currencies::free_balance(GIGAHDX, &bob),
			bob_gigahdx_before,
		);

		// Correct behaviour: a 5 M GIGAHDX-side voting lock should be in
		// effect after step 5, so this transfer must revert. Today this
		// assertion FAILS because LockSplit is never refreshed after the
		// stake (carry-over from the pre-stake 10 M HDX vote leaves
		// `LockSplit { gigahdx: 0, hdx: 10 M }` and the precompile reports
		// 0 locked GIGAHDX). The test will pass once the lock-split is
		// recomputed on stake / on every vote.
		assert!(
			transfer_result.is_err(),
			"locked GIGAHDX must not be transferable; got Ok",
		);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &charlie),
			charlie_gigahdx,
			"GIGAHDX balance must be unchanged after rejected transfer",
		);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &bob),
			bob_gigahdx_before,
			"bob must not have received any GIGAHDX",
		);
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&charlie),
			charlie_gigahdx,
			"GIGAHDX-side voting lock must reflect the active 5 M vote",
		);
	});
}

// ---------------------------------------------------------------------------
// Control case for the LockSplit-stale repro above.
//
// Same actor (Charlie) and same end-state target (5 M GIGAHDX, vote 5 M aye-
// None), but **no pre-stake HDX vote**. Sequence:
//   1. fund Charlie with 20 M HDX
//   2. giga_stake 5 M HDX → 5 M GIGAHDX
//   3. vote 5 M aye-None
//   4. transfer 5 M GIGAHDX out — must FAIL because the GIGAHDX-side lock is
//      computed correctly when no stale LockSplit existed beforehand.
//
// Together with `lock_split_should_remain_at_snapshot_when_stake_increases_balance`
// this proves the issue is the carry-over from a pre-stake HDX vote, not the
// per-vote enforcement itself.
// ---------------------------------------------------------------------------
#[test]
fn gigahdx_transfer_should_fail_when_voted_after_stake() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let charlie = sp_runtime::AccountId32::from(CHARLIE);
		let bob = sp_runtime::AccountId32::from(BOB);

		// Sanity: Charlie has no relevant state in this snapshot.
		assert_eq!(Currencies::free_balance(HDX, &charlie), 0);
		assert_eq!(Currencies::free_balance(GIGAHDX, &charlie), 0);
		assert_eq!(pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&charlie), 0);
		let split_pre = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&charlie);
		assert_eq!(split_pre.gigahdx_amount, 0);
		assert_eq!(split_pre.hdx_amount, 0);

		// Step 1: fund Charlie with 20 M HDX.
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			20_000_000 * UNITS,
		));

		// Step 2: stake 5 M HDX → 5 M GIGAHDX (no prior HDX vote).
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(charlie.clone()),
			5_000_000 * UNITS,
		));
		let charlie_gigahdx = Currencies::free_balance(GIGAHDX, &charlie);
		assert!(
			charlie_gigahdx > 0,
			"giga_stake must mint GIGAHDX (got {})",
			charlie_gigahdx
		);

		// Step 3: vote 5 M aye-None.
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(charlie.clone()),
			ref_index,
			aye(5_000_000 * UNITS),
		));

		// Diagnostic: the GIGAHDX-side lock should be 5 M, the HDX side 0.
		let split = pallet_gigahdx_voting::LockSplit::<Runtime>::get(&charlie);
		let lock = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&charlie);
		println!(
			"[diag] after vote: LockSplit {{ gigahdx: {}, hdx: {} }}, GigaHdxVotingLock = {}",
			split.gigahdx_amount, split.hdx_amount, lock,
		);
		assert_eq!(split.gigahdx_amount, 5_000_000 * UNITS);
		assert_eq!(split.hdx_amount, 0);
		assert_eq!(lock, 5_000_000 * UNITS);

		// Step 4: try to transfer the entire 5 M GIGAHDX. This MUST be rejected
		// — the aToken contract reads `getLockedBalance = 5 M` from the
		// precompile and reverts.
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);
		let transfer_result = Currencies::transfer(
			RuntimeOrigin::signed(charlie.clone()),
			bob.clone(),
			GIGAHDX,
			charlie_gigahdx,
		);
		println!("[diag] transfer result = {:?}", transfer_result);

		assert!(transfer_result.is_err(), "locked GIGAHDX transfer must fail; got Ok",);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &charlie),
			charlie_gigahdx,
			"GIGAHDX balance must be unchanged after rejected transfer",
		);
		assert_eq!(
			Currencies::free_balance(GIGAHDX, &bob),
			bob_gigahdx_before,
			"bob must not have received any GIGAHDX",
		);
	});
}
