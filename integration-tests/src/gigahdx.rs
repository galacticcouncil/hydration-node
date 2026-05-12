// SPDX-License-Identifier: Apache-2.0
//
// Integration tests for `pallet-gigahdx` against a mainnet-state snapshot
// with the AAVE V3 fork deployed (GIGAHDX listed as a reserve,
// `LockableAToken` consuming the lock-manager precompile at 0x0806).

use crate::polkadot_test_net::{hydra_live_ext, TestNet, ALICE, BOB, CHARLIE, DAVE, HDX, UNITS};
use frame_support::traits::OnInitialize;
use frame_support::traits::StorePreimage;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hex_literal::hex;
use hydra_dx_math::ratio::Ratio;
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

/// Asserts that the actual `Ratio` equals `expected_n / expected_d` via
/// cross-multiplication (Ratio's `PartialEq` is field-wise, so direct
/// `==` only matches when n and d are pointwise equal).
fn assert_rate_eq(actual: Ratio, expected_n: u128, expected_d: u128) {
	let expected = Ratio::new(expected_n, expected_d);
	assert_eq!(
		actual.cmp(&expected),
		std::cmp::Ordering::Equal,
		"rate mismatch: got {:?}, expected {expected_n}/{expected_d}",
		actual,
	);
}

/// AAVE pool address from the snapshot. Tests must not set it themselves —
/// a missing entry indicates a misconfigured snapshot and should fail loud.
fn pool_contract() -> EvmAddress {
	pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get().expect("snapshot must have GigaHdxPoolContract pre-populated")
}

pub const GIGAHDX_LOCK_ID: frame_support::traits::LockIdentifier = *b"ghdxlock";

fn lock_amount(account: &AccountId, id: frame_support::traits::LockIdentifier) -> Balance {
	pallet_balances::Locks::<Runtime>::get(account)
		.iter()
		.find(|l| l.id == id)
		.map(|l| l.amount)
		.unwrap_or(0)
}

/// Fund Alice with HDX and bind her EVM address. Snapshot already
/// configures the AAVE pool contract.
fn init_gigahdx() {
	let alice: AccountId = ALICE.into();
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		alice.clone(),
		1_000 * UNITS,
	));
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
fn giga_stake_should_lock_hdx_in_user_account_when_called() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let alice_hdx_before = Balances::free_balance(&alice);
		let alice_atoken_before = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// Lock model: HDX stays in Alice's account; `free_balance` doesn't subtract locks.
		assert_eq!(
			Balances::free_balance(&alice),
			alice_hdx_before,
			"HDX must remain in Alice's account (lock model)"
		);
		assert_eq!(locked_under_ghdx(&alice), 100 * UNITS);

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake should exist");
		assert_eq!(stake.hdx, 100 * UNITS);
		assert_eq!(stake.gigahdx, 100 * UNITS);

		let alice_atoken_after = Currencies::free_balance(GIGAHDX, &alice);
		assert!(
			alice_atoken_after > alice_atoken_before,
			"Alice should hold GIGAHDX after stake"
		);
	});
}

#[test]
fn giga_unstake_should_burn_atoken_when_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let atoken_after_stake = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// Stakes record persists (zero-active) until `unlock` cleans it up.
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake remains until unlock");
		assert_eq!(stake.hdx, 0);
		assert_eq!(stake.gigahdx, 0);

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		assert_eq!(locked_under_ghdx(&alice), entry.amount);

		let atoken_after_unstake = Currencies::free_balance(GIGAHDX, &alice);
		assert!(
			atoken_after_unstake < atoken_after_stake,
			"GIGAHDX should be burned on unstake"
		);
	});
}

#[test]
fn giga_unstake_should_keep_proportional_state_when_partial() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake should exist");
		// gigahdx always drops by exactly the unstaked amount.
		assert_eq!(stake.gigahdx, 60 * UNITS);

		// Position payout depends on snapshot rate. With a richly-funded gigapot
		// it can exceed Alice's active 100, draining her active stake to zero;
		// with a near-bootstrap rate the active stake just shrinks. Either way
		// the combined lock equals active + position.
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		assert_eq!(locked_under_ghdx(&alice), stake.hdx + entry.amount);
		assert!(entry.amount >= 40 * UNITS, "payout covers at least the principal share");
	});
}

#[test]
fn lock_manager_precompile_should_report_gigahdx_when_account_has_stake() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		let alice_evm = EVMAccounts::evm_address(&alice);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// `getLockedBalance(token, account)` — `token` must be the GIGAHDX
		// aToken; the precompile returns 0 otherwise so unrelated aTokens
		// can't accidentally consume gigahdx-stake state.
		let lock_manager: EvmAddress = H160(hex!("0000000000000000000000000000000000000806"));
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);
		let selector: [u8; 4] = sp_io::hashing::keccak_256(b"getLockedBalance(address,address)")[0..4]
			.try_into()
			.unwrap();
		let mut data = selector.to_vec();
		data.extend_from_slice(H256::from(gigahdx_token).as_bytes());
		data.extend_from_slice(H256::from(alice_evm).as_bytes());

		let result = Executor::<Runtime>::view(CallContext::new_view(lock_manager), data, 100_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"precompile call must succeed, got {:?}",
			result.exit_reason
		);
		let reported = U256::from_big_endian(&result.value);
		assert_eq!(reported, U256::from(100 * UNITS), "lock-manager must report gigahdx");

		// Wrong token must return zero (no state leak to unrelated aTokens).
		let mut wrong = selector.to_vec();
		wrong.extend_from_slice(H256::from(EvmAddress::zero()).as_bytes());
		wrong.extend_from_slice(H256::from(alice_evm).as_bytes());
		let wrong_result = Executor::<Runtime>::view(CallContext::new_view(lock_manager), wrong, 100_000);
		assert!(matches!(wrong_result.exit_reason, fp_evm::ExitReason::Succeed(_)));
		assert_eq!(U256::from_big_endian(&wrong_result.value), U256::zero());
	});
}

#[test]
fn lock_manager_precompile_should_resolve_bound_evm_address_to_substrate_stake() {
	// Round-trip with an EVM-bound user (the realistic shape: a MetaMask
	// user calls `bind_evm_address` so their AAVE-side activity maps back to
	// a stable substrate AccountId). The precompile receives the **bound**
	// EVM address as `account` and must resolve it to the same substrate
	// AccountId that `pallet-gigahdx::Stakes` is keyed by — otherwise
	// `LockableAToken.freeBalance` would read zero for the very users who
	// participated through the EVM front door, defeating the lock.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();

		let alice_evm = EVMAccounts::evm_address(&alice);
		assert_eq!(
			EVMAccounts::bound_account_id(alice_evm),
			Some(alice.clone()),
			"precondition: Alice's EVM address must be bound before staking"
		);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// `Stakes` is keyed by substrate AccountId; the precompile resolves
		// the bound H160 via `AddressMapping::into_account_id`, which must
		// yield Alice's AccountId so `locked_gigahdx` is non-zero.
		let lock_manager: EvmAddress = H160(hex!("0000000000000000000000000000000000000806"));
		let gigahdx_token = HydraErc20Mapping::asset_address(GIGAHDX);
		let selector: [u8; 4] = sp_io::hashing::keccak_256(b"getLockedBalance(address,address)")[0..4]
			.try_into()
			.unwrap();
		let mut data = selector.to_vec();
		data.extend_from_slice(H256::from(gigahdx_token).as_bytes());
		data.extend_from_slice(H256::from(alice_evm).as_bytes());

		let result = Executor::<Runtime>::view(CallContext::new_view(lock_manager), data, 100_000);
		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"precompile call must succeed, got {:?}",
			result.exit_reason
		);
		let reported = U256::from_big_endian(&result.value);
		assert_eq!(
			reported,
			U256::from(100 * UNITS),
			"bound EVM caller must resolve to Alice's substrate stake"
		);
	});
}

#[test]
fn giga_unstake_should_create_pending_position_when_called() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("entry exists");
		// Snapshot's gigapot may already hold yield → payout ≥ principal.
		assert!(entry.amount >= 40 * UNITS, "position covers at least principal");

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake remains");
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), stake.hdx + entry.amount);
	});
}

#[test]
fn unlock_should_release_lock_when_cooldown_elapsed() {
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
fn vote_should_succeed_with_locked_hdx_when_max_lock_semantics() {
	// HDX locked under `ghdxlock` must remain usable for conviction voting
	// via `LockableCurrency::max` semantics.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		fund_bob_for_decision_deposit();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

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

		assert_ok!(Referenda::place_decision_deposit(RuntimeOrigin::signed(bob), ref_index,));

		// Vote with 50 HDX — strictly less than the gigaHDX-locked 100 — so
		// the conviction-vote lock layers onto the already-locked balance.
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

		// Both locks coexist: `ghdxlock` and conviction-voting's lock.
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

/// Reset gigapot balance and stHDX issuance so rate-sensitive scenarios run
/// from a clean baseline. The snapshot may carry pre-existing yield.
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
	// Mirrors the production pre-condition: stake/unstake callers are bound.
	let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(account.clone()));
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

#[test]
fn giga_stake_should_mint_gigahdx_when_called_on_mainnet_snapshot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let stake_amount = 1_000 * UNITS;
		assert_ok!(<Currencies as MultiCurrency<_>>::deposit(HDX, &alice, 10_000 * UNITS));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));

		let hdx_before = Currencies::free_balance(HDX, &alice);
		let total_staked_hdx_before = GigaHdx::total_staked_hdx();
		let total_st_hdx_before = GigaHdx::total_gigahdx_supply();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));

		// Lock model: HDX stays in Alice's account, just locked.
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_before);
		assert_eq!(locked_under_ghdx(&alice), stake_amount);

		// stHDX is held by AAVE (the user never touches it directly).
		assert_eq!(Currencies::free_balance(ST_HDX, &alice), 0);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);

		assert_eq!(GigaHdx::total_staked_hdx(), total_staked_hdx_before + stake_amount);
		assert_eq!(GigaHdx::total_gigahdx_supply(), total_st_hdx_before + stake_amount);
		assert_rate_eq(GigaHdx::exchange_rate(), 1, 1);
	});
}

#[test]
fn giga_unstake_should_succeed_when_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
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
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position exists");
		assert!(entry.amount > 0);
	});
}

#[test]
fn giga_unstake_should_fail_when_amount_exceeds_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
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
fn giga_stake_should_fail_when_amount_below_min_on_snapshot() {
	// 10 UNITS is safely above any AAVE-internal min-supply rounding.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
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
fn giga_stake_should_succeed_when_supply_zeroed_after_full_exit() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		fund(&alice, 1_000_000 * UNITS);
		fund(&bob, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(GigaHdx::total_gigahdx_supply(), 100 * UNITS);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS,));

		// Supply zeroed → rate falls back to bootstrap 1.0.
		assert_eq!(GigaHdx::total_gigahdx_supply(), 0);
		assert_rate_eq(GigaHdx::exchange_rate(), 1, 1);

		// Bob's stake is independent of Alice's cooldown.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &bob), 100 * UNITS);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 100 * UNITS);
	});
}

#[test]
fn exchange_rate_should_inflate_when_hdx_transferred_directly_to_gigapot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();
		let gigapot = GigaHdx::gigapot_account_id();

		fund(&gigapot, UNITS);
		fund(&alice, 1_000_000 * UNITS);
		fund(&bob, 1_000_000 * UNITS);

		// rate becomes (100 + 1) / 100 = 1.01
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice), 100 * UNITS));
		assert_rate_eq(GigaHdx::exchange_rate(), 101, 100);

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob.clone()),
			gigapot,
			HDX,
			1_000 * UNITS,
		));
		assert_rate_eq(GigaHdx::exchange_rate(), 1101, 100);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS));
		assert!(
			Currencies::free_balance(GIGAHDX, &bob) < 10 * UNITS,
			"inflated rate should mint far fewer atokens"
		);

		fund(&charlie, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(charlie), 100 * UNITS));
	});
}

#[test]
fn giga_unstake_should_succeed_with_inflated_payout_when_pot_donated() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
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

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(bob),
			gigapot,
			HDX,
			500 * UNITS,
		));
		assert!(GigaHdx::exchange_rate() > Ratio::one());

		// A grief donation is a bonus to Alice on exit, not a DoS.
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
fn giga_unstake_should_succeed_when_exchange_rate_extreme() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 100 * UNITS);

		fund(&gigapot, 1_000_000_000_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS,));

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		// payout = 100 * (10^15 + 100) / 100 = 10^15 + 100
		assert_eq!(entry.amount, 1_000_000_000_000_000 * UNITS + 100 * UNITS);
	});
}

#[test]
fn aave_withdraw_should_revert_when_atokens_are_locked_by_active_stake() {
	// Direct EVM Pool.withdraw must be rejected while the user has an active
	// stake — `LockableAToken.burn`'s freeBalance check sees `0` because
	// `gigahdx` equals atoken balance. Without this the cooldown can be bypassed.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		let stake_amount = 1_000 * UNITS;
		fund(&alice, stake_amount);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount,));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let gigahdx_balance = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(gigahdx_balance, stake_amount);

		let pool = pool_contract();
		let sthdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let sthdx_before = Currencies::free_balance(ST_HDX, &alice);

		let data = build_aave_withdraw_calldata(sthdx_evm, gigahdx_balance, alice_evm);
		let result = Executor::<Runtime>::call(CallContext::new_call(pool, alice_evm), data, U256::zero(), 500_000);

		assert!(
			matches!(result.exit_reason, fp_evm::ExitReason::Revert(_)),
			"AAVE withdraw must revert on locked GIGAHDX (lock-manager precompile not honored?). exit_reason={:?}",
			result.exit_reason,
		);

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), gigahdx_balance);
		assert_eq!(Currencies::free_balance(ST_HDX, &alice), sthdx_before);
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake remains");
		assert_eq!(stake.gigahdx, stake_amount);
	});
}

#[test]
fn atoken_evm_transfer_should_fail_when_staked() {
	// While staked, atokens are 100% locked per the lock-manager precompile,
	// so any ERC20 transfer of GIGAHDX must revert.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let stake_amount = 1_000 * UNITS;
		fund(&alice, stake_amount);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount,));

		fund(&bob, UNITS);
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

#[test]
fn partial_unstake_should_not_leak_when_locks_aggregated_via_max() {
	// Regression: an earlier per-unstake-lock-id design let
	// `min(active_stake, cooldown)` HDX leak out during cooldown via
	// pallet-balances' max-of-locks semantics. The single combined lock
	// (`active + position`) closes the leak.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS,));
		assert_eq!(locked_under_ghdx(&alice), 1_000 * UNITS);

		// pot empty → payout equals principal, no yield is paid out.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 500 * UNITS,));

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(stake.hdx, 500 * UNITS);
		assert_eq!(entry.amount, 500 * UNITS);

		// Combined lock = 1000; the old buggy max(500, 500) = 500 leaked 500 HDX.
		assert_eq!(
			locked_under_ghdx(&alice),
			1_000 * UNITS,
			"combined lock must cover BOTH active stake and pending position",
		);

		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		let spendable =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(spendable, 0, "no HDX may leak out of the gigahdx system");
	});
}

#[test]
fn giga_unstake_should_keep_lock_layers_consistent_when_vote_active() {
	// gigahdx lock (active + position) and conviction lock must coexist;
	// spendable = balance − max(both).
	//
	// Stake must exceed the conviction-vote balance so that `pallet-gigahdx-rewards`'
	// freeze guard (frozen = min(vote_balance, stake.hdx)) doesn't block the
	// partial unstake. With stake=1000 and vote=800, frozen=800 and the
	// projected post-unstake hdx (=900) stays above frozen.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		fund_bob_for_decision_deposit();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_200 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS,));

		// Conviction balance 800 < stake 1000; freeze = 800, partial unstake of
		// 100 leaves hdx = 900 ≥ frozen, so the guard passes.
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS,));

		// ghdx lock = active (900) + pending position (100) = 1000.
		assert_eq!(locked_under_ghdx(&alice), 1_000 * UNITS);
		let conviction_lock = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(conviction_lock, 800 * UNITS);

		// spendable = 1200 − max(ghdx=1000, vote=800) = 200
		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		let spendable =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(spendable, 200 * UNITS);
	});
}

#[test]
fn partial_unstake_should_drain_active_when_payout_exceeds_active() {
	// Case 2 with partial unstake: payout for a fraction of atokens exceeds
	// active stake; active → 0, remainder from gigapot, leaving
	// `Stakes = { hdx: 0, gigahdx > 0 }` — atokens with zero cost basis.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		// Stake at bootstrap 1.0, then inflate pot → rate = (100 + 200) / 100 = 3.0.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS,));
		fund(&gigapot, 200 * UNITS);
		assert_rate_eq(GigaHdx::exchange_rate(), 3, 1);

		let alice_balance_before = Balances::free_balance(&alice);

		// Unstake half: payout = 50 × 3 = 150 > active 100 → active drained,
		// 50 yield from pot, position = 150.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS,));

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("record persists");
		assert_eq!(stake.hdx, 0, "active stake drained because payout exceeded principal");
		assert_eq!(stake.gigahdx, 50 * UNITS, "remaining atokens have zero cost basis now");
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 50 * UNITS);

		let entry = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).expect("position created");
		assert_eq!(entry.amount, 150 * UNITS);

		assert_eq!(Balances::free_balance(&alice), alice_balance_before + 50 * UNITS,);
		assert_eq!(Balances::free_balance(&gigapot), 150 * UNITS);

		assert_eq!(locked_under_ghdx(&alice), 150 * UNITS);

		assert_rate_eq(GigaHdx::exchange_rate(), 3, 1);
	});
}

#[test]
fn full_lifecycle_should_conserve_value_when_rate_inflated() {
	// End-to-end value conservation: stake 100 @ rate 1.0 → pot inflates to
	// rate 3.0 → drain across two payout-exceeds-active unstakes split by
	// the cooldown. Total receipts must equal original_stake × rate, gigapot
	// drained.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		let starting_balance = 1_000_000 * UNITS;
		fund(&alice, starting_balance);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS,));
		fund(&gigapot, 200 * UNITS);
		assert_rate_eq(GigaHdx::exchange_rate(), 3, 1);

		// First unstake: 50 stHDX → payout 150, active drained.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS,));
		let entry1 = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(entry1.amount, 150 * UNITS);

		System::set_block_number(entry1.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone())));
		assert!(pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).is_none());

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("atokens remain");
		assert_eq!(stake.hdx, 0);
		assert_eq!(stake.gigahdx, 50 * UNITS);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 50 * UNITS);

		// Second unstake: pot 150, supply 50 → rate stays 3.0, payout = 150.
		assert_rate_eq(GigaHdx::exchange_rate(), 3, 1);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS,));
		let entry2 = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(entry2.amount, 150 * UNITS);

		System::set_block_number(entry2.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone())));

		// Conservation: principal stayed in Alice's account (locked then unlocked);
		// yield transferred = 50 + 150 = 200 = original_stake × (rate − 1).
		assert_eq!(Balances::free_balance(&alice), starting_balance + 200 * UNITS);

		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).is_none());
		assert!(pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).is_none());
		assert_eq!(locked_under_ghdx(&alice), 0);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 0);
		assert_eq!(pallet_gigahdx::TotalLocked::<Runtime>::get(), 0);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 0);
		assert_eq!(Balances::free_balance(&gigapot), 0);
	});
}

#[test]
fn giga_stake_should_fail_when_evm_address_unbound() {
	// Without a bound EVM address, AAVE rejects `Pool.supply` (truncated
	// `onBehalfOf` fails preconditions), the adapter surfaces
	// `MoneyMarketSupplyFailed`, and `with_transaction` rolls back the stHDX
	// mint. Pins this so atokens can't silently land on a phantom account.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		// Bypass `fund()` to leave Alice unbound.
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			1_000 * UNITS,
		));

		let alice_evm = EVMAccounts::evm_address(&alice);
		assert!(
			EVMAccounts::bound_account_id(alice_evm).is_none(),
			"precondition: Alice must be unbound for this scenario",
		);

		let atoken_before = Currencies::free_balance(GIGAHDX, &alice);
		let sthdx_before = <Currencies as MultiCurrency<_>>::total_issuance(ST_HDX);

		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS),
			pallet_gigahdx::Error::<Runtime>::MoneyMarketSupplyFailed,
		);

		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), atoken_before);
		assert_eq!(<Currencies as MultiCurrency<_>>::total_issuance(ST_HDX), sthdx_before);
		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).is_none());
	});
}

#[test]
fn first_staker_inflation_grief_should_be_self_defeating_against_real_aave() {
	// Audit lead: attacker leaves a 1-wei stHDX residual, donates HDX to
	// inflate the rate, then expects new stakers to round-to-zero atokens.
	// Self-defeating against real AAVE V3: `Pool.withdraw(1)` reverts on
	// AAVE's min-amount check, so the attacker can never reclaim the donation.
	// Pinned so any change to AAVE config that makes this profitable trips here.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		fund(&alice, 1_000_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let alice_gigahdx = Currencies::free_balance(GIGAHDX, &alice);
		assert_eq!(alice_gigahdx, 100 * UNITS);

		// Partial-unstake leaving 1 wei (bulk burn passes AAVE's min-amount).
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(alice.clone()),
			alice_gigahdx - 1,
		));
		let position1 = pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice).unwrap();
		System::set_block_number(position1.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone())));
		assert_eq!(GigaHdx::total_gigahdx_supply(), 1);

		let gigapot = GigaHdx::gigapot_account_id();
		let donation = 500_000 * UNITS;
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(alice.clone()),
			gigapot.clone(),
			HDX,
			donation,
		));
		assert!(
			GigaHdx::exchange_rate() > Ratio::new(donation, 1),
			"rate should be heavily inflated after donation",
		);

		// `gigahdx_to_mint` floors to 0 → pallet's `ZeroAmount` guard fires
		// before AAVE (so this holds even on forks that accept `Pool.supply(0)`).
		fund(&bob, 100 * UNITS);
		let bob_hdx_before = Balances::free_balance(&bob);
		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 100 * UNITS),
			pallet_gigahdx::Error::<Runtime>::ZeroAmount,
		);
		assert_eq!(Balances::free_balance(&bob), bob_hdx_before);
		assert_eq!(Currencies::free_balance(GIGAHDX, &bob), 0);
		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&bob).is_none());

		// Self-defeat: attacker cannot exit the 1-wei residual.
		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 1),
			pallet_gigahdx::Error::<Runtime>::MoneyMarketWithdrawFailed,
		);

		assert_eq!(Balances::free_balance(&gigapot), donation);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 1);
		assert!(GigaHdx::exchange_rate() > Ratio::new(donation, 1));
	});
}

#[test]
fn giga_unstake_should_fail_when_position_pending() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS,));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 300 * UNITS,));

		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS),
			pallet_gigahdx::Error::<Runtime>::PendingUnstakeAlreadyExists,
		);
	});
}
