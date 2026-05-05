// SPDX-License-Identifier: Apache-2.0
//
// Integration tests for `pallet-gigahdx` against a snapshot of mainnet
// state with the AAVE V3 fork already deployed (GIGAHDX listed as a
// reserve, `LockableAToken` consuming the lock-manager precompile at
// 0x0806).

use crate::polkadot_test_net::{hydra_live_ext, TestNet, ALICE, BOB, UNITS};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::{
	Balances, ConvictionVoting, Currencies, EVMAccounts, GigaHdx, Referenda, Runtime, RuntimeOrigin, System,
};
use hydradx_traits::evm::InspectEvmAccounts;
use orml_traits::MultiCurrency;
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
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
