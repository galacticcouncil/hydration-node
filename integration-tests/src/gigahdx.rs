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

/// Flattened view of a single pending-unstake entry for tests that assume one.
#[derive(Clone, Debug)]
struct PendingView {
	#[allow(dead_code)]
	id: u32,
	amount: Balance,
	expires_at: hydradx_runtime::BlockNumber,
}

/// Read the only pending-unstake position for `who`. Panics if zero or more than one.
fn only_pending_position(who: &AccountId) -> PendingView {
	let mut iter = pallet_gigahdx::PendingUnstakes::<Runtime>::iter_prefix(who);
	let (id, p) = iter.next().expect("expected one pending position");
	assert!(iter.next().is_none(), "expected exactly one pending position");
	let cooldown: hydradx_runtime::BlockNumber = <Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
	PendingView {
		id,
		amount: p.amount,
		expires_at: id + cooldown,
	}
}

fn pending_count(who: &AccountId) -> u16 {
	pallet_gigahdx::Stakes::<Runtime>::get(who)
		.map(|s| s.unstaking_count)
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

		let entry = only_pending_position(&alice);
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
		let entry = only_pending_position(&alice);
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

		let entry = only_pending_position(&alice);
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

		let entry = only_pending_position(&alice);
		System::set_block_number(entry.expires_at);

		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), entry.id));

		assert_eq!(pending_count(&alice), 0);
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
		let entry = only_pending_position(&alice);
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
		assert_eq!(pending_count(&alice), 0);
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

		let entry = only_pending_position(&alice);
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

		let entry = only_pending_position(&alice);
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
		let entry = only_pending_position(&alice);
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

		let entry = only_pending_position(&alice);
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
		let entry1 = only_pending_position(&alice);
		assert_eq!(entry1.amount, 150 * UNITS);

		System::set_block_number(entry1.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), entry1.id));
		assert_eq!(pending_count(&alice), 0);

		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("atokens remain");
		assert_eq!(stake.hdx, 0);
		assert_eq!(stake.gigahdx, 50 * UNITS);
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 50 * UNITS);

		// Second unstake: pot 150, supply 50 → rate stays 3.0, payout = 150.
		assert_rate_eq(GigaHdx::exchange_rate(), 3, 1);
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS,));
		let entry2 = only_pending_position(&alice);
		assert_eq!(entry2.amount, 150 * UNITS);

		System::set_block_number(entry2.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), entry2.id));

		// Conservation: principal stayed in Alice's account (locked then unlocked);
		// yield transferred = 50 + 150 = 200 = original_stake × (rate − 1).
		assert_eq!(Balances::free_balance(&alice), starting_balance + 200 * UNITS);

		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).is_none());
		assert_eq!(pending_count(&alice), 0);
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
		let position1 = only_pending_position(&alice);
		System::set_block_number(position1.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), position1.id));
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
fn giga_unstake_should_fail_when_max_pending_positions_reached() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000_000 * UNITS);
		let max: u32 = <Runtime as pallet_gigahdx::Config>::MaxPendingUnstakes::get();
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			(max as Balance) * 100 * UNITS,
		));
		// Advance block between unstakes so each becomes a distinct position
		// (same-block unstakes compound into one).
		for _ in 0..max {
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS));
			System::set_block_number(System::block_number() + 1);
		}
		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 10 * UNITS),
			pallet_gigahdx::Error::<Runtime>::TooManyPendingUnstakes,
		);
	});
}

#[test]
fn cancel_unstake_should_fail_when_no_pending() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS,));

		assert_noop!(
			GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice), 0),
			pallet_gigahdx::Error::<Runtime>::PendingUnstakeNotFound,
		);
	});
}

#[test]
fn cancel_unstake_should_restore_position_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let pre_lock = locked_under_ghdx(&alice);
		let pre_atokens = Currencies::free_balance(GIGAHDX, &alice);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));
		assert_eq!(pending_count(&alice), 1);
		let position_id = only_pending_position(&alice).id;

		assert_ok!(GigaHdx::cancel_unstake(
			RuntimeOrigin::signed(alice.clone()),
			position_id
		));

		assert_eq!(pending_count(&alice), 0);
		let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(s.hdx, 100 * UNITS);
		assert_eq!(locked_under_ghdx(&alice), pre_lock);
		// AAVE rounding may shave a wei or two; tolerate small loss, forbid growth.
		assert!(Currencies::free_balance(GIGAHDX, &alice) <= pre_atokens);
		assert!(Currencies::free_balance(GIGAHDX, &alice) + 10 >= pre_atokens);
	});
}

#[test]
fn cancel_unstake_should_work_with_inflated_rate_e2e() {
	// Pre-inflate pot so unstake pays yield from gigapot; cancel folds it back as principal.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		fund(&gigapot, 200 * UNITS);
		// rate = (100 + 200) / 100 = 3.0
		assert_rate_eq(GigaHdx::exchange_rate(), 3, 1);

		let pre_lock = locked_under_ghdx(&alice);
		let pre_pot = Balances::free_balance(&gigapot);

		// Full unstake → payout 300, principal 100, yield 200 (pot drained).
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(Balances::free_balance(&gigapot), pre_pot - 200 * UNITS);
		assert_eq!(locked_under_ghdx(&alice), pre_lock + 200 * UNITS);
		let position_id = only_pending_position(&alice).id;

		assert_ok!(GigaHdx::cancel_unstake(
			RuntimeOrigin::signed(alice.clone()),
			position_id
		));

		let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(s.hdx, 300 * UNITS, "yield folded into principal");
		// Re-supply at rate 1.0 (pot drained, supply 0 → bootstrap) → 300 atokens.
		assert!(Currencies::free_balance(GIGAHDX, &alice) >= 300 * UNITS - 10);
		assert_eq!(locked_under_ghdx(&alice), pre_lock + 200 * UNITS);
		assert_eq!(Balances::free_balance(&gigapot), 0);
	});
}

#[test]
fn repeated_unstake_cancel_cycles_should_not_grow_position_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let initial_atokens = Currencies::free_balance(GIGAHDX, &alice);
		let initial_balance = Balances::free_balance(&alice);

		for _ in 0..5 {
			let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), s.gigahdx));
			let id = only_pending_position(&alice).id;
			assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), id));
		}

		// AAVE rounding may shave a few wei per cycle; forbid any growth.
		assert!(Currencies::free_balance(GIGAHDX, &alice) <= initial_atokens);
		assert_eq!(Balances::free_balance(&alice), initial_balance);
	});
}

#[test]
fn repeated_unstake_cancel_cycles_should_preserve_gigapot_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&gigapot, 50 * UNITS);
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let initial_system_total =
			pallet_gigahdx::TotalLocked::<Runtime>::get().saturating_add(Balances::free_balance(&gigapot));

		for _ in 0..5 {
			let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), s.gigahdx));
			let id = only_pending_position(&alice).id;
			assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), id));
		}

		let final_system_total =
			pallet_gigahdx::TotalLocked::<Runtime>::get().saturating_add(Balances::free_balance(&gigapot));
		assert_eq!(final_system_total, initial_system_total);
	});
}

#[test]
fn cancel_unstake_should_preserve_frozen_when_user_has_active_vote_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		fund_bob_for_decision_deposit();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_200 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS));

		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));
		let frozen_before = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen;
		assert_eq!(frozen_before, 800 * UNITS);

		// Unstake the unfrozen portion (100 ≤ hdx 1000 − frozen 800).
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			frozen_before,
		);

		let position_id = only_pending_position(&alice).id;
		assert_ok!(GigaHdx::cancel_unstake(
			RuntimeOrigin::signed(alice.clone()),
			position_id
		));
		let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(s.frozen, frozen_before, "cancel must not touch frozen");
		assert_eq!(s.hdx, 1_000 * UNITS);
		assert!(s.frozen <= s.hdx);
	});
}

fn advance_block() {
	System::set_block_number(System::block_number() + 1);
}

#[test]
fn multiple_unstakes_should_create_distinct_positions_when_blocks_advance_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 300 * UNITS));

		let mut expected_ids = vec![];
		for _ in 0..3 {
			expected_ids.push(System::block_number());
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS));
			advance_block();
		}

		let mut positions: Vec<(u32, Balance)> = pallet_gigahdx::PendingUnstakes::<Runtime>::iter_prefix(&alice)
			.map(|(id, p)| (id, p.amount))
			.collect();
		positions.sort_by_key(|(id, _)| *id);
		let actual_ids: Vec<u32> = positions.iter().map(|(id, _)| *id).collect();
		assert_eq!(actual_ids, expected_ids);
		assert!(positions.iter().all(|(_, amt)| *amt == 50 * UNITS));
	});
}

#[test]
fn unstakes_should_compound_when_called_in_same_block_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 300 * UNITS));

		let block = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 30 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 20 * UNITS));

		assert_eq!(pending_count(&alice), 1);
		assert_eq!(
			pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice, block)
				.unwrap()
				.amount,
			50 * UNITS,
		);
	});
}

#[test]
fn unlock_should_release_one_position_while_others_pending_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 300 * UNITS));

		let id_a = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS));
		advance_block();
		let id_b = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 60 * UNITS));

		let cooldown = <Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		System::set_block_number(id_a + cooldown);

		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), id_a));

		let remaining = only_pending_position(&alice);
		assert_eq!(remaining.id, id_b);
		assert_eq!(remaining.amount, 60 * UNITS);
		// active 190 + pending 60 = 250
		assert_eq!(locked_under_ghdx(&alice), 250 * UNITS);
	});
}

#[test]
fn cancel_unstake_should_target_specific_position_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 300 * UNITS));

		let id_a = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 30 * UNITS));
		advance_block();
		let id_b = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));
		advance_block();
		let id_c = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS));

		assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), id_b));

		let mut ids: Vec<u32> = pallet_gigahdx::PendingUnstakes::<Runtime>::iter_prefix(&alice)
			.map(|(id, _)| id)
			.collect();
		ids.sort();
		assert_eq!(ids, vec![id_a, id_c]);
		// Cancel folded 40 UNITS back into active; lock total unchanged.
		assert_eq!(locked_under_ghdx(&alice), 300 * UNITS);
		assert_eq!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().hdx, 220 * UNITS);
	});
}

#[test]
fn multi_position_cycle_should_preserve_lock_balance_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 300 * UNITS));
		let starting_lock = locked_under_ghdx(&alice);

		let id_a = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 50 * UNITS));
		advance_block();
		let id_b = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 60 * UNITS));
		advance_block();
		let id_c = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 70 * UNITS));

		assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), id_b));
		let cooldown = <Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		System::set_block_number(id_a + cooldown);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), id_a));

		let remaining = only_pending_position(&alice);
		assert_eq!(remaining.id, id_c);
		// Cancel keeps lock total; unlock(50) drops lock by 50.
		assert_eq!(locked_under_ghdx(&alice), starting_lock - 50 * UNITS);
	});
}

#[test]
fn vote_freeze_should_coexist_with_multiple_pending_positions_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		fund_bob_for_decision_deposit();
		let alice: AccountId = ALICE.into();
		fund(&alice, 1_500 * UNITS);
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_200 * UNITS));

		let id_a = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));
		let frozen_before = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen;
		assert_eq!(frozen_before, 800 * UNITS);

		advance_block();
		let id_b = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), id_a));
		let cooldown = <Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		System::set_block_number(id_b + cooldown);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), id_b));

		let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(s.frozen, frozen_before, "frozen must persist across multi-position ops");
		assert!(s.frozen <= s.hdx);
	});
}

#[test]
fn cancel_should_handle_compounded_position_with_yield_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		fund(&gigapot, 50 * UNITS);
		assert_rate_eq(GigaHdx::exchange_rate(), 3, 2);

		let block = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 60 * UNITS));

		assert_eq!(pending_count(&alice), 1);
		assert_eq!(
			pallet_gigahdx::PendingUnstakes::<Runtime>::get(&alice, block)
				.unwrap()
				.amount,
			150 * UNITS,
		);
		assert_eq!(Balances::free_balance(&gigapot), 0);

		assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), block));

		let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(s.unstaking, 0);
		assert_eq!(s.unstaking_count, 0);
		assert_eq!(s.hdx, 150 * UNITS);
		// AAVE may round scaled balance down by a few wei on re-supply.
		assert!(s.gigahdx >= 150 * UNITS - 10);
		assert_eq!(locked_under_ghdx(&alice), 150 * UNITS);
	});
}

#[test]
fn cancel_compounded_position_should_preserve_system_value_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		fund(&gigapot, 50 * UNITS);

		let block = System::block_number();
		let baseline_total =
			pallet_gigahdx::TotalLocked::<Runtime>::get().saturating_add(Balances::free_balance(&gigapot));

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 60 * UNITS));
		assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), block));

		let final_total =
			pallet_gigahdx::TotalLocked::<Runtime>::get().saturating_add(Balances::free_balance(&gigapot));
		assert_eq!(final_total, baseline_total);
	});
}

/// Drive 10 unstake calls compounded into 4 positions across 4 blocks with
/// rate inflation between phases. Returns the position ids in unstake order
/// and the total yield transferred from the pot to alice.
fn drive_complex_unstake_scenario(alice: &AccountId) -> (Vec<u32>, Balance) {
	let gigapot = GigaHdx::gigapot_account_id();

	// b1: 3× unstake at rate 1.0 → pending 300.
	let b1 = System::block_number();
	for _ in 0..3 {
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
	}

	// b2: pot += 700 → rate 2.0. 2× unstake principal-covered → pending 400.
	advance_block();
	let b2 = System::block_number();
	fund(&gigapot, 700 * UNITS);
	for _ in 0..2 {
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
	}

	// b3: pot total 1200 → rate 3.0. 3× unstake (1st drains active, rest pull yield) → pending 900.
	advance_block();
	let b3 = System::block_number();
	fund(&gigapot, 1200 * UNITS);
	for _ in 0..3 {
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
	}

	// b4: 2× unstake, both pull yield (active is 0) → pending 600.
	advance_block();
	let b4 = System::block_number();
	for _ in 0..2 {
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
	}

	let s = pallet_gigahdx::Stakes::<Runtime>::get(alice).unwrap();
	assert_eq!(s.hdx, 0);
	assert_eq!(s.gigahdx, 0);
	assert_eq!(s.unstaking_count, 4);
	assert_eq!(s.unstaking, 2200 * UNITS);
	assert_eq!(
		pallet_gigahdx::PendingUnstakes::<Runtime>::get(alice, b1)
			.unwrap()
			.amount,
		300 * UNITS
	);
	assert_eq!(
		pallet_gigahdx::PendingUnstakes::<Runtime>::get(alice, b2)
			.unwrap()
			.amount,
		400 * UNITS
	);
	assert_eq!(
		pallet_gigahdx::PendingUnstakes::<Runtime>::get(alice, b3)
			.unwrap()
			.amount,
		900 * UNITS
	);
	assert_eq!(
		pallet_gigahdx::PendingUnstakes::<Runtime>::get(alice, b4)
			.unwrap()
			.amount,
		600 * UNITS
	);
	assert_eq!(Balances::free_balance(&gigapot), 0);

	(vec![b1, b2, b3, b4], 1200 * UNITS)
}

#[test]
fn full_unstake_via_cancel_all_should_fold_yield_into_active_stake_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		let initial_alice_total: Balance = 10_000 * UNITS;
		fund(&alice, initial_alice_total);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS));

		let (ids, total_yield) = drive_complex_unstake_scenario(&alice);
		assert_eq!(total_yield, 1_200 * UNITS);
		assert_eq!(Balances::free_balance(&alice), initial_alice_total + total_yield);
		assert_eq!(locked_under_ghdx(&alice), 2200 * UNITS);

		for id in ids.iter().rev() {
			assert_ok!(GigaHdx::cancel_unstake(RuntimeOrigin::signed(alice.clone()), *id));
		}

		let s = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(s.unstaking, 0);
		assert_eq!(s.unstaking_count, 0);
		assert_eq!(s.hdx, 2200 * UNITS);
		assert_eq!(pallet_gigahdx::TotalLocked::<Runtime>::get(), 2200 * UNITS);
		assert_eq!(locked_under_ghdx(&alice), 2200 * UNITS);
		assert_eq!(Balances::free_balance(&alice), initial_alice_total + total_yield);
	});
}

#[test]
fn full_unstake_via_unlock_all_should_release_full_amount_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		let initial_alice_total: Balance = 10_000 * UNITS;
		fund(&alice, initial_alice_total);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS));

		let (ids, total_yield) = drive_complex_unstake_scenario(&alice);
		assert_eq!(total_yield, 1_200 * UNITS);

		let cooldown = <Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		let last_id = *ids.last().unwrap();
		System::set_block_number(last_id + cooldown);

		for id in ids.iter() {
			assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), *id));
		}

		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).is_none());
		assert_eq!(pallet_gigahdx::TotalLocked::<Runtime>::get(), 0);
		assert_eq!(locked_under_ghdx(&alice), 0);
		assert_eq!(Balances::free_balance(&alice), initial_alice_total + total_yield);
	});
}

#[test]
fn unlock_should_release_full_compounded_amount_e2e() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();
		let gigapot = GigaHdx::gigapot_account_id();
		fund(&alice, 1_000_000 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		fund(&gigapot, 50 * UNITS);

		let block = System::block_number();
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 40 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 60 * UNITS));
		let pre_free = Balances::free_balance(&alice);
		assert_eq!(locked_under_ghdx(&alice), 150 * UNITS);

		let cooldown = <Runtime as pallet_gigahdx::Config>::CooldownPeriod::get();
		System::set_block_number(block + cooldown);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), block));

		assert_eq!(pending_count(&alice), 0);
		assert_eq!(Balances::free_balance(&alice), pre_free);
		assert_eq!(locked_under_ghdx(&alice), 0);
	});
}

// Strict admission: any non-overlap-allowed lock (legacy staking, vesting,
// democracy, …) blocks `giga_stake` entirely, even when free_balance is
// sufficient. This prevents the lock-layering exploit at the root: the
// `stk_stks` + `ghdxlock` overlap can never be set up in the first place.
#[test]
fn giga_stake_should_fail_when_caller_has_legacy_staking_lock() {
	use frame_support::traits::{LockableCurrency, WithdrawReasons};

	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		let alice: AccountId = ALICE.into();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			2_000 * UNITS,
		));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));
		<Balances as LockableCurrency<_>>::set_lock(*b"stk_stks", &alice, 1_000 * UNITS, WithdrawReasons::all());

		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(alice), 500 * UNITS),
			pallet_gigahdx::Error::<Runtime>::BlockedByExternalLock,
		);
	});
}

// `pyconvot` is in the runtime's overlap allowlist (HdxExternalClaims), so a
// conviction-voting lock must NOT block stake admission — the voter's HDX is
// only earmarked, not committed to a payout, so sharing it with a gigahdx
// stake is safe.
#[test]
fn giga_stake_should_succeed_when_caller_has_conviction_voting_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_giga_state_for_fixture();
		fund_bob_for_decision_deposit();

		let alice: AccountId = ALICE.into();
		fund(&alice, 1_000 * UNITS);

		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(800 * UNITS, Conviction::Locked1x),
		));

		let conviction_lock = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(conviction_lock, 800 * UNITS);

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 500 * UNITS));
		assert_eq!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().hdx, 500 * UNITS,);
	});
}
