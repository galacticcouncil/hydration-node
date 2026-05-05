// SPDX-License-Identifier: Apache-2.0
//
// Integration tests for `pallet-gigahdx` against a snapshot of mainnet
// state with the AAVE V3 fork already deployed (GIGAHDX listed as a
// reserve, `LockableAToken` consuming the lock-manager precompile at
// 0x0806).

use crate::polkadot_test_net::{hydra_live_ext, TestNet, ALICE, BOB, CHARLIE, DAVE, HDX, UNITS};
use frame_support::traits::OnInitialize;
use frame_support::traits::StorePreimage;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::evm::{
	aave_trade_executor::Function as AaveFunction, precompiles::erc20_mapping::HydraErc20Mapping,
	precompiles::handle::EvmDataWriter, Executor,
};
use hydradx_runtime::{
	Balances, ConvictionVoting, Currencies, Democracy, EVMAccounts, GigaHdx, Preimage, Referenda, Runtime,
	RuntimeOrigin, Scheduler, System,
};
use hydradx_traits::evm::{CallContext, Erc20Mapping, InspectEvmAccounts, EVM};
use orml_traits::MultiCurrency;
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use primitives::constants::time::DAYS;
use primitives::{AccountId, AssetId, Balance, EvmAddress};
use sp_core::{H160, H256, U256};
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/gigahdx/gigahdx";

#[allow(dead_code)]
pub const ST_HDX: AssetId = 670;
pub const GIGAHDX: AssetId = 67;

/// Aave V3 Pool deployed in the gigahdx snapshot.
pub fn pool_contract() -> EvmAddress {
	H160(hex!("820df200b69031a84bb8e608b0016f688e43051c"))
}

pub const GIGAHDX_LOCK_ID: frame_support::traits::LockIdentifier = *b"ghdxlock";

fn lock_amount(account: &AccountId, id: frame_support::traits::LockIdentifier) -> Balance {
	pallet_balances::Locks::<Runtime>::get(account)
		.iter()
		.find(|l| l.id == id)
		.map(|l| l.amount)
		.unwrap_or(0)
}

/// Set up the gigaHDX system: configure pool contract, fund Alice with HDX.
fn init_gigahdx() {
	// Set the deployed AAVE Pool address so the adapter knows where to call.
	assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract(),));

	// Give Alice plenty of HDX.
	let alice: AccountId = ALICE.into();
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		alice.clone(),
		1_000 * UNITS,
	));

	// Bind Alice's EVM address (idempotent — adapter does this too).
	let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice));
}

/// Fund Bob with enough HDX to cover an OpenGov decision deposit.
fn fund_bob_for_decision_deposit() {
	let bob: AccountId = BOB.into();
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		bob,
		2_000_000_000 * UNITS,
	));
}

fn locked_under_ghdx(account: &AccountId) -> Balance {
	pallet_balances::Locks::<Runtime>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

#[test]
fn giga_stake_locks_hdx_in_user_account_and_mints_atoken() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let alice_hdx_before = Balances::free_balance(&alice);
		let alice_atoken_before = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// HDX stays in Alice's account (lock model — not pool model).
		// `free_balance` reports total free; locks don't subtract from it.
		assert_eq!(
			Balances::free_balance(&alice),
			alice_hdx_before,
			"HDX must remain in Alice's account (lock model)"
		);

		// A `ghdxlock` lock of 100 HDX exists on Alice.
		assert_eq!(locked_under_ghdx(&alice), 100 * UNITS);

		// `Stakes[Alice]` populated.
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake should exist");
		assert_eq!(stake.hdx_locked, 100 * UNITS);
		assert_eq!(stake.st_minted, 100 * UNITS); // bootstrap 1:1

		// Alice received GIGAHDX (aToken) on the EVM side.
		let alice_atoken_after = Currencies::free_balance(GIGAHDX, &alice);
		assert!(
			alice_atoken_after > alice_atoken_before,
			"Alice should hold GIGAHDX after stake"
		);
	});
}

#[test]
fn giga_unstake_releases_lock_and_burns_atoken() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let atoken_after_stake = Currencies::free_balance(GIGAHDX, &alice);

		// Full unstake — active stake drops to zero, position holds the payout.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// `Stakes[Alice]` is now zero-active (cleaned up only by `unlock`).
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake remains until unlock");
		assert_eq!(stake.hdx_locked, 0);
		assert_eq!(stake.st_minted, 0);

		// Combined lock now equals the position amount.
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		assert_eq!(locked_under_ghdx(&alice), entry.amount);

		// Alice's GIGAHDX balance dropped (aToken burned).
		let atoken_after_unstake = Currencies::free_balance(GIGAHDX, &alice);
		assert!(
			atoken_after_unstake < atoken_after_stake,
			"GIGAHDX should be burned on unstake"
		);
	});
}

#[test]
fn giga_unstake_partial_keeps_proportional_state() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake should exist");
		// st_minted always drops by exactly the unstaked amount.
		assert_eq!(stake.st_minted, 60 * UNITS);

		// Position payout depends on snapshot rate. With a richly-funded gigapot
		// it can exceed Alice's active 100, draining her active stake to zero
		// (case 2). With a near-bootstrap rate the active stake just shrinks
		// (case 1). Either way the combined lock equals active + position.
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		assert_eq!(locked_under_ghdx(&alice), stake.hdx_locked + entry.amount);
		assert!(entry.amount >= 40 * UNITS, "payout covers at least the principal share");
	});
}

#[test]
fn lock_manager_precompile_reports_st_minted() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let alice_evm = EVMAccounts::evm_address(&alice);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// Call lock-manager precompile at 0x0806. ABI:
		//   getLockedBalance(address token, address account) returns (uint256)
		// The `token` arg is unused; we pass any address.
		let lock_manager: EvmAddress = H160(hex!("0000000000000000000000000000000000000806"));
		let selector: [u8; 4] = sp_io::hashing::keccak_256(b"getLockedBalance(address,address)")[0..4]
			.try_into()
			.unwrap();
		let mut data = selector.to_vec();
		data.extend_from_slice(H256::from(EvmAddress::zero()).as_bytes()); // token (unused)
		data.extend_from_slice(H256::from(alice_evm).as_bytes()); // account

		use hydradx_runtime::evm::Executor;
		use hydradx_traits::evm::{CallContext, EVM};
		let result = Executor::<Runtime>::view(CallContext::new_view(lock_manager), data, 100_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"precompile call must succeed, got {:?}",
			result.exit_reason
		);
		let reported = U256::from_big_endian(&result.value);
		assert_eq!(reported, U256::from(100 * UNITS), "lock-manager must report st_minted");
	});
}

#[test]
fn giga_unstake_creates_pending_position_and_combined_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("entry exists");
		// Mainnet snapshot's gigapot may already hold yield → payout ≥ principal.
		assert!(entry.amount >= 40 * UNITS, "position covers at least principal");

		// Single combined lock: active stake (Stakes.hdx_locked) + position.amount.
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake remains");
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), stake.hdx_locked + entry.amount);
	});
}

#[test]
fn unlock_after_cooldown_releases_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).unwrap();
		System::set_block_number(entry.expires_at);

		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone())));

		assert!(pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).is_none());
		// Stakes was zero-active after full unstake → cleaned up by unlock.
		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).is_none());
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), 0);
	});
}

#[test]
fn vote_with_locked_hdx_works_via_max_lock_semantics() {
	// Proves the lock model: HDX locked under `ghdxlock` is ALSO usable
	// for conviction voting via `LockableCurrency::max` semantics. We
	// submit a referendum, place its decision deposit, fast-forward into
	// the deciding period, and vote with the staked HDX.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		fund_bob_for_decision_deposit();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// Submit a trivial Root-track referendum (Alice pays the small submission deposit).
		use frame_support::traits::Bounded;
		use frame_support::traits::StorePreimage;
		use hydradx_runtime::Preimage;
		let proposal_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![1, 2, 3] });
		let bounded: Bounded<_, <Runtime as frame_system::Config>::Hashing> = Preimage::bound(proposal_call).unwrap();

		let now = System::block_number();
		let ref_index = pallet_referenda::ReferendumCount::<Runtime>::get();
		assert_ok!(Referenda::submit(
			RuntimeOrigin::signed(alice.clone()),
			Box::new(RawOrigin::Root.into()),
			bounded,
			frame_support::traits::schedule::DispatchTime::At(now + 100),
		));

		// Bob covers the (large) decision deposit.
		assert_ok!(Referenda::place_decision_deposit(RuntimeOrigin::signed(bob), ref_index,));

		// Vote with 50 HDX of conviction-locked balance — strictly less than
		// the gigaHDX-locked 100, so we're voting "into" the locked HDX.
		// `LockableCurrency::max` semantics allow conviction-voting to layer
		// its own lock on the same balance.
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			AccountVote::Standard {
				vote: Vote {
					aye: true,
					conviction: Conviction::Locked3x,
				},
				balance: 50 * UNITS,
			},
		));

		// Both locks coexist: `ghdxlock` (100) and conviction-voting's lock (50).
		assert_eq!(locked_under_ghdx(&alice), 100 * UNITS);
		let conviction_lock = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(
			conviction_lock,
			50 * UNITS,
			"conviction-voting must lock the voted balance"
		);
	});
}

// ---------------------------------------------------------------------------
// Snapshot-based scenario tests (ported from old gigahdx test suite).
//
// These run against the gigahdx snapshot with the AAVE fork live, exercising
// the lock-cooldown design end-to-end against the real money market.
// ---------------------------------------------------------------------------

/// Reset the gigapot balance and stHDX issuance so test math runs from a
/// clean baseline. The snapshot may carry pre-existing yield in the gigapot;
/// rate-sensitive scenarios need a known starting state.
fn reset_giga_state_for_fixture() {
	orml_tokens::TotalIssuance::<Runtime>::set(ST_HDX, 0);
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		GigaHdx::gigapot_account_id(),
		0,
	));
}

fn fund(account: &AccountId, amount: Balance) {
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		account.clone(),
		amount,
	));
}

#[allow(dead_code)]
fn next_block() {
	System::set_block_number(System::block_number() + 1);
	Scheduler::on_initialize(System::block_number());
	Democracy::on_initialize(System::block_number());
}

#[allow(dead_code)]
fn fast_forward_to(n: u32) {
	while System::block_number() < n {
		next_block();
	}
}

#[allow(dead_code)]
fn aye_with_conviction(amount: Balance, conviction: Conviction) -> AccountVote<Balance> {
	AccountVote::Standard {
		vote: Vote { aye: true, conviction },
		balance: amount,
	}
}

/// Submit a referendum by Bob, place its decision deposit (Dave), and
/// fast-forward into the deciding period. Returns the referendum index.
#[allow(dead_code)]
fn begin_referendum_by_bob() -> u32 {
	let bob: AccountId = BOB.into();
	let dave: AccountId = DAVE.into();
	let now = System::block_number();
	let ref_index = pallet_referenda::ReferendumCount::<Runtime>::get();

	fund(&bob, 1_000_000 * UNITS);
	let proposal = {
		use frame_support::traits::Bounded;
		let inner = pallet_balances::Call::<Runtime>::force_set_balance {
			who: AccountId::from(CHARLIE),
			new_free: 2,
		};
		let outer = hydradx_runtime::RuntimeCall::Balances(inner);
		let bounded: Bounded<_, <Runtime as frame_system::Config>::Hashing> = Preimage::bound(outer).unwrap();
		bounded
	};
	assert_ok!(Referenda::submit(
		RuntimeOrigin::signed(bob),
		Box::new(RawOrigin::Root.into()),
		proposal,
		frame_support::traits::schedule::DispatchTime::At(now + 10 * DAYS),
	));

	fund(&dave, 2_000_000_000 * UNITS);
	assert_ok!(Referenda::place_decision_deposit(
		RuntimeOrigin::signed(dave),
		ref_index,
	));

	fast_forward_to(now + 5 * DAYS);
	ref_index
}

/// Stake `stake_amount` HDX as ALICE — funds Alice with exactly that much
/// HDX first, so all of it lands in the gigahdx system.
#[allow(dead_code)]
fn setup_alice_with_only_gigahdx(stake_amount: Balance) {
	let alice: AccountId = ALICE.into();
	fund(&alice, stake_amount);
	assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice), stake_amount));
}

fn build_aave_withdraw_calldata(asset: H160, amount: Balance, to: H160) -> Vec<u8> {
	EvmDataWriter::new_with_selector(AaveFunction::Withdraw)
		.write(asset)
		.write(amount)
		.write(to)
		.build()
}

fn build_erc20_transfer_calldata(to: H160, amount: Balance) -> Vec<u8> {
	let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
	data.extend_from_slice(&[0u8; 12]);
	data.extend_from_slice(to.as_bytes());
	data.extend_from_slice(&U256::from(amount).to_big_endian());
	data
}

// ---------- Wave 1: snapshot integration tests ----------

#[test]
fn giga_stake_should_mint_gigahdx_on_mainnet_snapshot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let stake_amount = 1_000 * UNITS;
		assert_ok!(<Currencies as MultiCurrency<_>>::deposit(HDX, &alice, 10_000 * UNITS));

		let hdx_before = Currencies::free_balance(HDX, &alice);
		let total_hdx_before = GigaHdx::total_hdx();
		let total_st_hdx_before = GigaHdx::total_st_hdx_supply();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));

		// Lock model: HDX stays in Alice's account, just locked.
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_before);
		assert_eq!(locked_under_ghdx(&alice), stake_amount);

		// stHDX is held by AAVE (the user never touches it directly).
		assert_eq!(Currencies::free_balance(ST_HDX, &alice), 0);

		// GIGAHDX (aToken) minted to Alice via the real AAVE supply.
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);

		// Totals incremented; bootstrap rate = 1.
		assert_eq!(GigaHdx::total_hdx(), total_hdx_before + stake_amount);
		assert_eq!(GigaHdx::total_st_hdx_supply(), total_st_hdx_before + stake_amount);
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_u32(1));
	});
}

#[test]
fn giga_unstake_should_succeed_when_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS));

		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		assert!(gigahdx_balance > 0);

		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_balance,
		));

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 0);
		// Position created, st_minted zeroed, lock now equals position amount.
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position exists");
		assert!(entry.amount > 0);
	});
}

#[test]
fn giga_unstake_should_fail_when_amount_exceeds_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);
		let hdx_before = Balances::free_balance(&alice);

		assert!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 200 * UNITS).is_err());

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_before);
		assert_eq!(Balances::free_balance(&alice), hdx_before);
		assert!(pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).is_none());
	});
}

#[test]
fn giga_stake_should_succeed_above_min_and_fail_below() {
	// Pallet gate: amounts strictly below MinStake are rejected by the pallet
	// regardless of AAVE state. The "succeeds above min" half uses 10 UNITS,
	// safely clear of any AAVE-internal minimum supply rounding.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS));
		assert!(Currencies::free_balance(GIGAHDX, &alice) > 0);

		let min_stake = <Runtime as pallet_gigahdx::Config>::MinStake::get();
		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), min_stake - 1),
			pallet_gigahdx::Error::<Runtime>::BelowMinStake
		);
	});
}

#[test]
fn restake_should_succeed_after_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		fund(&alice, 1_000_000 * UNITS);
		fund(&bob, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(GigaHdx::total_st_hdx_supply(), 100 * UNITS);

		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// Supply zeroed; rate falls back to bootstrap 1.0.
		assert_eq!(GigaHdx::total_st_hdx_supply(), 0);
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_u32(1));

		// Bob can stake fresh — Alice's cooldown is hers, Bob is unaffected.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &bob), 100 * UNITS);
		assert_eq!(GigaHdx::total_st_hdx_supply(), 100 * UNITS);
	});
}

#[test]
fn exchange_rate_should_inflate_when_hdx_transferred_directly_to_gigapot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();
		let gigapot = GigaHdx::gigapot_account_id();

		// Pot starts with 1 UNIT yield.
		fund(&gigapot, UNITS);
		fund(&alice, 1_000_000 * UNITS);
		fund(&bob, 1_000_000 * UNITS);

		// Alice stakes 100 → rate becomes (100 + 1) / 100 = 1.01.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice), 100 * UNITS));
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_rational(101, 100));

		// Bob donates 1000 HDX directly to the gigapot → rate inflates.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			gigapot,
			HDX,
			1_000 * UNITS,
		));
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_rational(1101, 100));

		// New stake at the inflated rate gets fewer GIGAHDX.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));
		assert!(
			Currencies::free_balance(GIGAHDX, &bob) < 10 * UNITS,
			"inflated rate should mint far fewer atokens"
		);

		// A fresh staker can still participate.
		fund(&charlie, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(charlie), 100 * UNITS));
	});
}

#[test]
fn unstake_payout_should_succeed_after_donation_on_real_aave() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&gigapot, UNITS);
		fund(&alice, 1_000_000 * UNITS);
		fund(&bob, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let gigahdx_minted = Currencies::free_balance(GIGAHDX, &alice);
		assert!(gigahdx_minted > 0);

		// Bob grief-donates HDX to inflate the rate.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob),
			gigapot,
			HDX,
			500 * UNITS,
		));
		assert!(GigaHdx::exchange_rate() > sp_runtime::FixedU128::from(1));

		// Alice fully unstakes — the donation is a bonus to her, not a DoS.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			gigahdx_minted,
		));

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		assert!(
			entry.amount > 100 * UNITS,
			"payout reflects inflated rate: got {} (staked 100 UNITS)",
			entry.amount,
		);
	});
}

#[test]
fn giga_unstake_should_succeed_at_extreme_exchange_rate() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 100 * UNITS);

		// Inflate the gigapot to an extreme value.
		fund(&gigapot, 1_000_000_000_000_000 * UNITS);

		// Full unstake at extreme rate: case 2 — active drained, all yield from pot.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		// payout = 100 * UNITS * (10^15 * UNITS + 100 * UNITS) / (100 * UNITS)
		//        = 10^15 * UNITS + 100 * UNITS  (≈ 10^15 UNITS)
		assert_eq!(entry.amount, 1_000_000_000_000_000 * UNITS + 100 * UNITS);
	});
}

#[test]
fn aave_withdraw_should_revert_when_atokens_are_locked_by_active_stake() {
	// Direct EVM-level Pool.withdraw must be rejected by the lock-manager
	// precompile while the user still has an active stake — `st_minted`
	// equals atoken balance, so `LockableAToken.burn`'s freeBalance check
	// gives 0 and the burn reverts. This protects the cooldown semantics:
	// without it, users could bypass `giga_unstake` entirely.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));

		let alice: AccountId = ALICE.into();
		let stake_amount = 1_000 * UNITS;
		fund(&alice, stake_amount);
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			stake_amount,
		));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(gigahdx_balance, stake_amount);

		let pool = pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get();
		let sthdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let sthdx_before = Currencies::free_balance(ST_HDX, &alice);

		let data = build_aave_withdraw_calldata(sthdx_evm, gigahdx_balance, alice_evm);
		let result = Executor::<Runtime>::call(
			CallContext::new_call(pool, alice_evm),
			data,
			U256::zero(),
			500_000,
		);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"AAVE withdraw must revert on locked GIGAHDX (lock-manager precompile not honored?). exit_reason={:?}",
			result.exit_reason,
		);

		// Nothing moved.
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_balance);
		assert_eq!(Currencies::free_balance(ST_HDX, &alice), sthdx_before);
		// st_minted unchanged.
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake remains");
		assert_eq!(stake.st_minted, stake_amount);
	});
}

#[test]
fn atoken_evm_transfer_should_fail_while_staked() {
	// ERC20 `transfer` of GIGAHDX must revert while the user has an active
	// stake — atokens are 100% locked-balance per the lock-manager precompile.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let stake_amount = 1_000 * UNITS;
		fund(&alice, stake_amount);
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			stake_amount,
		));

		fund(&bob, UNITS);
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone())));
		let bob_evm = EVMAccounts::evm_address(&bob);
		let bob_gigahdx_before = Currencies::free_balance(GIGAHDX, &bob);

		let alice_evm = EVMAccounts::evm_address(&alice);
		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);

		let data = build_erc20_transfer_calldata(bob_evm, gigahdx_balance);
		let result = Executor::<Runtime>::call(
			CallContext::new_call(gigahdx_token, alice_evm),
			data,
			U256::zero(),
			500_000,
		);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"GIGAHDX ERC20 transfer must revert while staked. exit_reason={:?}",
			result.exit_reason,
		);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_balance);
		assert_eq!(Currencies::free_balance(GIGAHDX, &bob), bob_gigahdx_before);
	});
}

// ---------- Wave 2: cooldown × voting-lock co-existence ----------

#[test]
fn partial_unstake_should_not_leak_via_max_aggregated_lock_ids() {
	// Regression test for the per-unstake-lock-id design where pallet-balances'
	// max-of-locks semantics let `min(active_stake, cooldown)` HDX leak out
	// during cooldown. Under the new single-combined-lock model the lock
	// equals `active + position`, so partial unstake never frees any HDX.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			1_000 * UNITS,
		));
		assert_eq!(locked_under_ghdx(&alice), 1_000 * UNITS);

		// Partial unstake — half. With pot empty, payout = principal (case 1).
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			500 * UNITS,
		));

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(stake.hdx_locked, 500 * UNITS);
		assert_eq!(entry.amount, 500 * UNITS);

		// Combined lock = active(500) + position(500) = 1000. Old buggy
		// design produced max(500, 500) = 500, leaking 500 HDX out of
		// cooldown immediately after partial unstake.
		assert_eq!(
			locked_under_ghdx(&alice),
			1_000 * UNITS,
			"combined lock must cover BOTH active stake and pending position",
		);

		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		let spendable = <Balances as Inspect<AccountId>>::reducible_balance(
			&alice,
			Preservation::Expendable,
			Fortitude::Polite,
		);
		assert_eq!(spendable, 0, "no HDX may leak out of the gigahdx system");
	});
}

#[test]
fn unstake_during_active_vote_keeps_lock_layers_consistent() {
	// Stake → vote with conviction on a balance larger than the stake → partial
	// unstake. The gigahdx lock (active + position) and the conviction lock
	// must coexist; spendable balance is `balance − max(both)`.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));
		reset_giga_state_for_fixture();
		fund_bob_for_decision_deposit();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			500 * UNITS,
		));

		// Vote with 800 HDX conviction — exceeds the stake amount, layers
		// over both staked and free HDX.
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));

		// Partial unstake — 100 stHDX. Pot empty → payout = principal = 100.
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			100 * UNITS,
		));

		// Combined gigahdx lock = active(400) + position(100) = 500.
		assert_eq!(locked_under_ghdx(&alice), 500 * UNITS);
		// Conviction lock unchanged at 800.
		let conviction_lock = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(conviction_lock, 800 * UNITS);

		// Spendable = balance(1000) − max(ghdx=500, vote=800) = 200.
		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		let spendable = <Balances as Inspect<AccountId>>::reducible_balance(
			&alice,
			Preservation::Expendable,
			Fortitude::Polite,
		);
		assert_eq!(spendable, 200 * UNITS);
	});
}

#[test]
fn second_unstake_is_rejected_while_position_pending() {
	// One pending position per account — no concurrent unstakes.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), pool_contract()));

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			1_000 * UNITS,
		));
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			300 * UNITS,
		));

		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS),
			pallet_gigahdx::Error::<Runtime>::PendingUnstakeAlreadyExists,
		);
	});
}
