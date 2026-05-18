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
	Balances, ConvictionVoting, Currencies, Democracy, EVMAccounts, GigaHdx, Liquidation, Preimage, Referenda, Runtime,
	RuntimeOrigin, Scheduler, Staking, System,
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
fn realize_yield_should_fold_accrued_into_principal_when_rate_increased() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		init_gigahdx();
		reset_giga_state_for_fixture();

		let alice: AccountId = ALICE.into();
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));

		// Inject yield into the gigapot so rate = (100 + 100) / 100 = 2.
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			100 * UNITS,
		));

		let rate_before = GigaHdx::exchange_rate();
		let stake_before = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake exists");
		assert_eq!(stake_before.hdx, 100 * UNITS);

		assert_ok!(GigaHdx::realize_yield(RuntimeOrigin::signed(alice.clone())));

		let stake_after = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("stake exists");
		assert_eq!(stake_after.hdx, 200 * UNITS);
		assert_eq!(stake_after.gigahdx, stake_before.gigahdx, "gigahdx unchanged");
		assert_eq!(locked_under_ghdx(&alice), 200 * UNITS);
		assert_eq!(Balances::free_balance(GigaHdx::gigapot_account_id()), 0);
		assert_eq!(GigaHdx::exchange_rate(), rate_before, "exchange rate unchanged");
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

/// Initialize legacy staking inside the gigahdx snapshot (which doesn't ship
/// it pre-initialized). Funds the pot to clear `MissingPotBalance`.
fn init_legacy_staking() {
	let pot = pallet_staking::Pallet::<Runtime>::pot_account_id();
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		pot,
		HDX,
		(10_000 * UNITS) as i128,
	));
	assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));
}

#[test]
fn migrate_should_move_legacy_position_into_gigahdx_when_called() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			10_000 * UNITS,
		));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));
		init_legacy_staking();

		let stake_amount = 5_000 * UNITS;
		assert_ok!(Staking::stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert!(
			pallet_staking::Pallet::<Runtime>::get_user_position_id(&alice)
				.unwrap()
				.is_some(),
			"legacy position must exist pre-migrate"
		);
		assert_eq!(lock_amount(&alice, *b"stk_stks"), stake_amount);

		assert_ok!(GigaHdx::migrate(RuntimeOrigin::signed(alice.clone())));

		// Legacy side cleaned.
		assert_eq!(
			pallet_staking::Pallet::<Runtime>::get_user_position_id(&alice).unwrap(),
			None
		);
		assert_eq!(lock_amount(&alice, *b"stk_stks"), 0);

		// Gigahdx side populated. No legacy rewards accrued (pot just initialized),
		// so unlocked equals stake_amount exactly.
		let stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).expect("gigahdx stake must exist");
		assert_eq!(stake.hdx, stake_amount);
		assert!(stake.gigahdx > 0, "aToken minted");
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), stake_amount);
	});
}

#[test]
fn migrate_should_refuse_when_no_legacy_position() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			10_000 * UNITS,
		));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));
		init_legacy_staking();

		assert_noop!(
			GigaHdx::migrate(RuntimeOrigin::signed(alice.clone())),
			pallet_staking::Error::<Runtime>::InconsistentState(
				pallet_staking::pallet::InconsistentStateError::PositionNotFound
			)
		);
		assert!(pallet_gigahdx::Stakes::<Runtime>::get(&alice).is_none());
	});
}

#[test]
fn legacy_stake_should_refuse_when_gigahdx_lock_present() {
	// Strict policy: HDX already pledged under `ghdxlock` cannot back a legacy
	// stake, otherwise the same balance would earn rewards from both pallets.
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			10_000 * UNITS,
		));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));
		init_legacy_staking();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert!(locked_under_ghdx(&alice) > 0, "ghdxlock must be set");

		assert_noop!(
			Staking::stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS),
			pallet_staking::Error::<Runtime>::BlockedByExternalLock
		);
	});
}

#[test]
fn legacy_stake_should_succeed_after_giga_position_fully_exits() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			10_000 * UNITS,
		));
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));
		init_legacy_staking();

		// Stake → unstake → wait cooldown → unlock. Cleans the ghdxlock entirely.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(alice.clone()), 100 * UNITS));
		let entry = only_pending_position(&alice);
		System::set_block_number(entry.expires_at);
		assert_ok!(GigaHdx::unlock(RuntimeOrigin::signed(alice.clone()), entry.id));
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), 0);

		// Legacy stake now succeeds — no overlapping claim left.
		assert_ok!(Staking::stake(RuntimeOrigin::signed(alice.clone()), 1_000 * UNITS));
	});
}

// ---------------------------------------------------------------------------
// Liquidation integration smoke tests.
//
// A full end-to-end liquidation (open a HOLLAR borrow in the GIGAHDX pool,
// push HF<1 via oracle manipulation, run `Liquidation::liquidate`, assert the
// seize) requires the snapshot to already contain a borrower position in the
// GIGAHDX pool — non-trivial to construct in-test. These cover the entry-
// point routing instead. The full-flow case is tracked as a follow-up.
// ---------------------------------------------------------------------------

const HOLLAR_ASSET_ID: AssetId = 222;

#[test]
fn liquidate_gigahdx_should_refuse_when_debt_asset_is_not_hollar() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		let alice_evm = EVMAccounts::evm_address(&alice);
		let route = hydradx_traits::router::Route::default();

		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::signed(alice),
				GIGAHDX,
				HDX, // not HOLLAR
				alice_evm,
				1_000 * UNITS,
				route,
			),
			pallet_liquidation::Error::<Runtime>::UnsupportedDebtAsset
		);
	});
}

#[test]
fn liquidate_gigahdx_should_refuse_when_borrower_has_no_position() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice: AccountId = ALICE.into();
		let alice_evm = EVMAccounts::evm_address(&alice);
		let route = hydradx_traits::router::Route::default();

		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::signed(alice),
				GIGAHDX,
				HOLLAR_ASSET_ID,
				alice_evm,
				1_000 * UNITS,
				route,
			),
			pallet_liquidation::Error::<Runtime>::NoGigaHdxPosition
		);
	});
}

// ============================================================================
// Real-liquidation integration tests
//
// These exercise the full liquidate_gigahdx flow against the live snapshot:
// real EVM borrow on the GIGAHDX pool, real oracle crash via a JIT-deployed
// fixed-price mock, real `Pool.liquidationCall`. Adapted from the previous
// gigahdx_voting impl tests (preserved at tmp/gigahdx_liquidation.rs).
// ============================================================================

const HOLLAR_DECIMALS_18: Balance = 1_000_000_000_000_000_000;

/// Deploys a tiny EVM contract that returns a fixed `uint256` price.
/// Used to crash the GIGAHDX pool's oracle for stHDX so the borrower's HF
/// drops below 1.
fn deploy_fixed_price_oracle(price: U256) -> EvmAddress {
	use hex_literal::hex;
	let acl_admin = EvmAddress::from_slice(&hex!("aa7e0000000000000000000000000000000aa7e0"));

	// Runtime: PUSH32 <price> | PUSH1 0 | MSTORE | PUSH1 32 | PUSH1 0 | RETURN
	let mut runtime = vec![0x7f];
	runtime.extend_from_slice(&price.to_big_endian());
	runtime.extend_from_slice(&[0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xF3]);
	let rt_len = runtime.len() as u8;
	let code_offset = 12u8;
	let mut init_code = vec![
		0x60,
		rt_len,
		0x60,
		code_offset,
		0x60,
		0x00,
		0x39,
		0x60,
		rt_len,
		0x60,
		0x00,
		0xF3,
	];
	init_code.extend_from_slice(&runtime);

	use pallet_evm::Runner;
	<Runtime as pallet_evm::Config>::Runner::create(
		acl_admin,
		init_code,
		U256::zero(),
		1_000_000,
		Some(U256::from(1_000_000_000u64)),
		None,
		None,
		vec![],
		vec![],
		false,
		true,
		None,
		None,
		<Runtime as pallet_evm::Config>::config(),
	)
	.expect("mock oracle deploy")
	.value
}

fn selector(sig: &str) -> Vec<u8> {
	sp_io::hashing::keccak_256(sig.as_bytes())[0..4].to_vec()
}

/// Calls `setAssetSources([asset], [source])` on the AaveOracle as the ACL admin.
fn set_oracle_price_source(oracle: EvmAddress, asset: EvmAddress, source: EvmAddress) {
	use hex_literal::hex;
	let acl_admin = EvmAddress::from_slice(&hex!("aa7e0000000000000000000000000000000aa7e0"));
	let mut data = selector("setAssetSources(address[],address[])");
	data.extend_from_slice(&H256::from_low_u64_be(64).0);
	data.extend_from_slice(&H256::from_low_u64_be(128).0);
	data.extend_from_slice(&H256::from_low_u64_be(1).0);
	data.extend_from_slice(H256::from(asset).as_bytes());
	data.extend_from_slice(&H256::from_low_u64_be(1).0);
	data.extend_from_slice(H256::from(source).as_bytes());
	let result = Executor::<Runtime>::call(CallContext::new_call(oracle, acl_admin), data, U256::zero(), 500_000);
	assert!(
		matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
		"setAssetSources failed: {:?}",
		result.exit_reason
	);
}

/// Resolve the GIGAHDX pool's own oracle (it's distinct from the main pool's).
fn giga_pool_oracle(giga_pool: EvmAddress) -> EvmAddress {
	let r = Executor::<Runtime>::view(
		CallContext::new_view(giga_pool),
		selector("ADDRESSES_PROVIDER()"),
		100_000,
	);
	let pap = EvmAddress::from_slice(&r.value[12..32]);
	let r = Executor::<Runtime>::view(CallContext::new_view(pap), selector("getPriceOracle()"), 100_000);
	EvmAddress::from_slice(&r.value[12..32])
}

/// Common setup: bind EVM addresses, fund Alice + Bob, return the gigahdx
/// pool address + the oracle that prices stHDX for that pool.
fn liquidation_test_setup() -> (AccountId, AccountId, EvmAddress, EvmAddress, EvmAddress, EvmAddress) {
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone()));
	let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(bob.clone()));

	let alice_evm = EVMAccounts::evm_address(&alice);
	let pool = pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get().expect("snapshot must have GigaHdxPoolContract");

	assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool));
	let oracle = giga_pool_oracle(pool);
	let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR_ASSET_ID);

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		alice.clone(),
		100_000 * UNITS,
	));

	(alice, bob, alice_evm, pool, oracle, hollar_addr)
}

/// Fund the treasury and giga-stake on its behalf so it has collateral to
/// borrow HOLLAR against during a liquidation. Also enables stHDX as
/// collateral for the treasury's EVM account.
fn fund_treasury_for_liquidation(pool: EvmAddress) {
	use hydradx_runtime::BorrowingTreasuryAccount;
	let treasury = BorrowingTreasuryAccount::get();
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		treasury.clone(),
		2_000_000 * UNITS,
	));
	let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(treasury.clone()));
	assert_ok!(GigaHdx::giga_stake(
		RuntimeOrigin::signed(treasury.clone()),
		1_000_000 * UNITS
	));
	let treasury_evm = EVMAccounts::evm_address(&treasury);
}

/// Crash stHDX price to 30% of `original` on the gigahdx pool's oracle.
fn crash_st_hdx_price(oracle: EvmAddress, st_hdx_evm: EvmAddress) {
	let original = U256::from(200_833u64);
	let mock = deploy_fixed_price_oracle(original * 30 / 100);
	set_oracle_price_source(oracle, st_hdx_evm, mock);
}

/// Executes the same flow as `pallet_liquidation::liquidate_gigahdx` step by
/// step from the test, so we exercise the real building blocks (AAVE pool,
/// LockableAToken precompile, Seize trait, lock refresh) end-to-end against
/// the live snapshot. Mirrors what the extrinsic does — borrow HOLLAR as
/// treasury, liquidationCall on Aave, transfer seized aToken to the protocol
/// holder, run finalise_seize to move HDX and refresh locks.
#[test]
fn gigahdx_liquidation_flow_should_seize_collateral_and_close_debt() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury = BorrowingTreasuryAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&treasury);

		// Alice stakes, enables collateral, borrows HOLLAR.
		let stake_amount = 10_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);
		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);

		fund_treasury_for_liquidation(pool);

		// Pre-liquidation snapshot.
		let alice_gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);
		let liq_gigahdx_before = Currencies::free_balance(GIGAHDX, &liq_account);
		let alice_stake_before = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();

		crash_st_hdx_price(oracle, st_hdx_evm);
		let pre = get_user_account_data(pool, alice_evm).unwrap();
		assert!(
			pre.health_factor < U256::from(1_000_000_000_000_000_000u128),
			"HF must be < 1; got {:?}",
			pre.health_factor
		);
		let debt_before = pre.total_debt_base;

		// === Replicate the pallet's liquidate_gigahdx flow ===
		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();

		// pre_seize: zero Alice's recorded gigahdx so LockableAToken accepts
		// the burn during Aave's liquidationCall.
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();

		// Treasury borrows HOLLAR.
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);

		// Aave liquidationCall with receiveAToken=true.
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);
		let gigahdx_before_call = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		let liq_result = Executor::<Runtime>::call(
			CallContext::new_call(pool, treasury_evm),
			liq_data,
			U256::zero(),
			50_000_000,
		);
		assert!(
			matches!(liq_result.exit_reason, fp_evm::ExitReason::Succeed(_)),
			"liquidationCall failed: {:?} {}",
			liq_result.exit_reason,
			hex::encode(&liq_result.value)
		);
		let gigahdx_after_call = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let actual_seized = gigahdx_after_call - gigahdx_before_call;
		assert!(actual_seized > 0);

		// Pro-rata HDX matching the seized aToken portion.
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		let residual = orig_gigahdx - actual_seized;

		// finalise_seize: HDX move + lock refresh.
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			residual,
		)
		.unwrap();

		// Move seized aToken from treasury's EVM account to the protocol holder.
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_evm_account.clone()),
			liq_account.clone(),
			GIGAHDX,
			actual_seized,
		));

		// === Assertions ===

		// Alice's gigahdx aToken (substrate side) shrank.
		let alice_gigahdx_after = Currencies::free_balance(GIGAHDX, &alice);
		assert!(alice_gigahdx_after < alice_gigahdx_before);

		// Liquidation account picked up the seized aToken.
		let liq_gigahdx_after = Currencies::free_balance(GIGAHDX, &liq_account);
		assert_eq!(liq_gigahdx_after - liq_gigahdx_before, actual_seized);

		// Pallet-side Stakes record on alice shrank both hdx and gigahdx pro-rata.
		let alice_stake_after = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert!(alice_stake_after.hdx < alice_stake_before.hdx);
		assert_eq!(alice_stake_after.gigahdx, residual);
		assert_eq!(alice_stake_before.hdx - alice_stake_after.hdx, seize_hdx);

		// Liquidation account got a Stakes entry mirroring the seize.
		let liq_stake = pallet_gigahdx::Stakes::<Runtime>::get(&liq_account).unwrap();
		assert_eq!(liq_stake.hdx, seize_hdx);
		assert_eq!(liq_stake.gigahdx, actual_seized);

		// ghdxlock refreshed on both accounts.
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), alice_stake_after.hdx);
		assert_eq!(lock_amount(&liq_account, GIGAHDX_LOCK_ID), seize_hdx);

		// Debt fell on the Aave side.
		let post = get_user_account_data(pool, alice_evm).unwrap();
		assert!(post.total_debt_base < debt_before);

		// Treasury accumulated the HOLLAR borrow it took to fund the liquidation.
		let treasury_data = get_user_account_data(pool, treasury_evm).unwrap();
		assert!(treasury_data.total_debt_base > U256::zero());
	});
}

/// Partial liquidation must seize from the ghdxlock-covered portion only;
/// the borrower's transferable balance above the lock stays untouched.
#[test]
fn gigahdx_liquidation_should_not_seize_from_users_free_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		// Stake, then pin balance to stake + an explicit free buffer.
		let stake_amount = 10_000 * UNITS;
		let free_buffer = 250 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			stake_amount + free_buffer,
		));
		assert_eq!(Balances::free_balance(&alice), stake_amount + free_buffer);
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), stake_amount);

		// Sanity: pre-liquidation reducible balance == free buffer.
		let transferable_before =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(
			transferable_before, free_buffer,
			"setup: transferable HDX equals the free buffer above the gigahdx lock"
		);

		// Drive to undercollateralized + run a partial liquidation.
		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		let total_balance_before = Balances::free_balance(&alice);
		let ghdxlock_before = lock_amount(&alice, GIGAHDX_LOCK_ID);

		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2; // partial liquidation
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let g_before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - g_before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		assert!(seize_hdx > 0);
		assert!(
			seize_hdx < stake_amount,
			"partial liquidation: only part of the stake is seized"
		);

		// No-op here (Alice has no vote); kept for parity with the production flow.
		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice, seize_hdx,
		)
		.unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();

		let total_balance_after = Balances::free_balance(&alice);
		assert_eq!(total_balance_before - total_balance_after, seize_hdx);

		let ghdxlock_after = lock_amount(&alice, GIGAHDX_LOCK_ID);
		assert_eq!(ghdxlock_before - ghdxlock_after, seize_hdx);

		// Free buffer is untouched — the seize came from the locked HDX.
		let transferable_after =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(transferable_after, transferable_before);
		assert_eq!(transferable_after, free_buffer);
	});
}

/// Same as above with an active full-stake conviction vote: the surgical
/// pyconvot reduction must not over-release.
#[test]
fn gigahdx_liquidation_should_not_seize_from_free_balance_with_active_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		let stake_amount = 10_000 * UNITS;
		let free_buffer = 250 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			stake_amount + free_buffer,
		));

		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(stake_amount, Conviction::Locked3x),
		));
		let transferable_before =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(transferable_before, free_buffer);

		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		let total_balance_before = Balances::free_balance(&alice);
		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let g_before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - g_before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		assert!(seize_hdx > 0 && seize_hdx < stake_amount);

		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice, seize_hdx,
		)
		.unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();

		assert_eq!(total_balance_before - Balances::free_balance(&alice), seize_hdx);
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), stake_amount - seize_hdx);

		let pyconvot_after = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(pyconvot_after, stake_amount - seize_hdx);

		let transferable_after =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(transferable_after, transferable_before);
	});
}

/// Healthy positions cannot be liquidated. Even when our pallet's Seize
/// machinery is invoked, the underlying AAVE liquidationCall rejects with
/// HF >= 1.
#[test]
fn gigahdx_liquidation_should_fail_when_position_is_healthy() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;

		let (alice, _bob, alice_evm, pool, _oracle, hollar_addr) = liquidation_test_setup();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());

		// Healthy borrow position.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			10_000 * UNITS
		));
		borrow(pool, alice_evm, hollar_addr, HOLLAR_DECIMALS_18); // tiny

		fund_treasury_for_liquidation(pool);

		// HF > 1 — no crash.
		let data = get_user_account_data(pool, alice_evm).unwrap();
		assert!(data.health_factor > U256::from(1_000_000_000_000_000_000u128));

		// Simulate the pallet flow up to liquidationCall — should revert.
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		borrow(pool, treasury_evm, hollar_addr, HOLLAR_DECIMALS_18 / 2);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			HOLLAR_DECIMALS_18 / 2,
			true,
		);
		let r = Executor::<Runtime>::call(
			CallContext::new_call(pool, treasury_evm),
			liq_data,
			U256::zero(),
			50_000_000,
		);
		assert!(
			matches!(r.exit_reason, fp_evm::ExitReason::Revert(_)),
			"liquidationCall on a healthy position must revert, got {:?}",
			r.exit_reason
		);
	});
}

/// `TotalLocked` and the gigahdx supply are invariant across a liquidation —
/// HDX is moved between borrower and the liquidation account, not minted or
/// burned; the same applies to the aToken (transferred, not burned).
#[test]
fn gigahdx_liquidation_should_keep_total_locked_invariant() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			10_000 * UNITS
		));
		borrow(pool, alice_evm, hollar_addr, 5 * HOLLAR_DECIMALS_18);
		fund_treasury_for_liquidation(pool);

		// Snapshot total invariants before the seize.
		let total_locked_before = pallet_gigahdx::TotalLocked::<Runtime>::get();
		let exchange_rate_before = pallet_gigahdx::Pallet::<Runtime>::exchange_rate();

		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = 5 * HOLLAR_DECIMALS_18 / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_evm_account),
			liq_account.clone(),
			GIGAHDX,
			actual_seized
		));

		// Invariants.
		assert_eq!(
			pallet_gigahdx::TotalLocked::<Runtime>::get(),
			total_locked_before,
			"TotalLocked must be invariant — HDX moves, not burns/mints"
		);
		assert_eq!(
			pallet_gigahdx::Pallet::<Runtime>::exchange_rate(),
			exchange_rate_before,
			"exchange_rate must be invariant — no impact on the gigapot or stHDX supply"
		);
	});
}

/// After a borrower is liquidated, an uninvolved staker can still stake and
/// unstake at the unchanged rate.
#[test]
fn other_users_should_stake_normally_after_liquidation() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let charlie: AccountId = CHARLIE.into();
		let _ = EVMAccounts::bind_evm_address(RuntimeOrigin::signed(charlie.clone()));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			100_000 * UNITS,
		));
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			10_000 * UNITS
		));
		borrow(pool, alice_evm, hollar_addr, 5 * HOLLAR_DECIMALS_18);
		fund_treasury_for_liquidation(pool);

		crash_st_hdx_price(oracle, st_hdx_evm);
		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = 5 * HOLLAR_DECIMALS_18 / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_evm_account),
			liq_account,
			GIGAHDX,
			actual_seized
		));

		// Charlie stakes fresh — flow unaffected.
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(charlie.clone()),
			5_000 * UNITS
		));
		let charlie_gigahdx = Currencies::free_balance(GIGAHDX, &charlie);
		assert!(charlie_gigahdx > 0);
		// And he can unstake.
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(charlie), charlie_gigahdx));
	});
}

/// Borrower with an active ongoing vote that pins their full balance via
/// `pyconvot` + `Stakes.frozen` — liquidation must force-remove the vote so
/// the HDX seizure can proceed. Verifies the full vote-clearance path:
/// `ClearConflictingVotes` dispatches `remove_vote` as the borrower's
/// signed origin (`UnvoteScope::Any`) which fires the rewards-pallet
/// `on_remove_vote` hook → `Stakes.frozen` decrements, `UserVoteRecord`
/// dropped, `pyconvot` lock recomputed by conviction-voting.
#[test]
fn gigahdx_liquidation_should_force_remove_conflicting_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		// Alice stakes.
		let stake_amount = 10_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));

		// Alice casts a Locked3x vote on an ongoing referendum with her full
		// stake — pins everything via `Stakes.frozen` and `pyconvot`.
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(stake_amount, Conviction::Locked3x),
		));
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			stake_amount,
			"freeze must equal full vote amount"
		);
		assert!(
			pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, ref_index).is_some(),
			"rewards-side vote record must exist"
		);

		// Alice borrows HOLLAR.
		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);

		fund_treasury_for_liquidation(pool);

		// Crash price → HF < 1.
		crash_st_hdx_price(oracle, st_hdx_evm);
		let alice_data = get_user_account_data(pool, alice_evm).unwrap();
		assert!(alice_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

		// === Run the liquidation flow ===
		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();

		// THE key step: force-remove the conflicting vote so the rewards-side
		// `Stakes.frozen` drops, freeing the HDX for transfer in finalise_seize.
		let cleared =
			<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::clear_conflicting_votes(
				&alice, 0,
			)
			.unwrap();
		assert_eq!(cleared, 1, "exactly one vote removed");
		assert!(
			pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, ref_index).is_none(),
			"UserVoteRecord cleared by the rewards on_remove_vote hook"
		);
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			0,
			"frozen must drop after vote removal"
		);
		// `pyconvot` may persist via conviction-voting's prior_lock for the
		// conviction-period even after `remove_vote`. The load-bearing signal
		// for our seize is that `Stakes.frozen` dropped to zero (above) — that
		// re-enables `do_unstake`/`finalise_seize`'s `hdx >= frozen` guard.

		// Continue with the rest of the seize flow.
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - before;
		assert!(actual_seized > 0);

		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_evm_account),
			liq_account.clone(),
			GIGAHDX,
			actual_seized,
		));

		// Post-conditions:
		let alice_stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(alice_stake.hdx, orig_hdx - seize_hdx, "alice.hdx shrank by seize_hdx");
		assert_eq!(alice_stake.frozen, 0, "frozen stays zero post-seize");
		assert_eq!(
			lock_amount(&alice, GIGAHDX_LOCK_ID),
			alice_stake.hdx,
			"ghdxlock refreshed"
		);
	});
}

// ============================================================================
// HOLLAR borrow regression baseline.
//
// After `giga_stake`, the freshly-supplied stHDX is auto-enabled as collateral
// by AAVE (isolation-mode default for a user with no other collateral). HOLLAR
// borrow lands directly — no explicit `setUserUseReserveAsCollateral` toggle.
// ============================================================================

#[test]
fn giga_stake_should_give_collateral_and_borrow_power() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::get_user_account_data;

		let (alice, _bob, alice_evm, pool, _oracle, _hollar_addr) = liquidation_test_setup();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			10_000 * UNITS
		));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), 10_000 * UNITS);

		let data = get_user_account_data(pool, alice_evm).unwrap();
		assert!(data.total_collateral_base > U256::zero(), "collateral counted");
		assert!(data.available_borrows_base > U256::zero(), "borrow power live");
	});
}

#[test]
fn borrow_hollar_should_succeed_directly_after_giga_stake() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};

		let (alice, _bob, alice_evm, pool, _oracle, hollar_addr) = liquidation_test_setup();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			10_000 * UNITS
		));

		let amount: Balance = HOLLAR_DECIMALS_18; // 1 HOLLAR
		let hollar_before = Currencies::free_balance(HOLLAR_ASSET_ID, &alice);
		borrow(pool, alice_evm, hollar_addr, amount);
		let hollar_after = Currencies::free_balance(HOLLAR_ASSET_ID, &alice);
		assert_eq!(hollar_after - hollar_before, amount);

		let post = get_user_account_data(pool, alice_evm).unwrap();
		assert!(post.total_debt_base > U256::zero());
	});
}

/// Audit3 Finding 3 — Split / SplitAbstain votes are recorded in
/// `UserVoteRecords` (with `weighted = 0` so no rewards) precisely so
/// liquidation's `clear_conflicting_votes` can find and remove them.
#[test]
fn clear_conflicting_votes_should_remove_split_votes() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;

		let (alice, _bob, _alice_evm, _pool, _oracle, _hollar_addr) = liquidation_test_setup();

		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			10_000 * UNITS
		));

		// Cast a Split vote on an ongoing referendum.
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			AccountVote::Split {
				aye: 3_000 * UNITS,
				nay: 2_000 * UNITS,
			},
		));

		// Recorded as a zero-weight slot so clearance can reach it.
		let rec = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, ref_index).unwrap();
		assert_eq!(rec.weighted, 0, "Split votes carry no reward weight");
		assert_eq!(rec.staked_vote_amount, 5_000 * UNITS, "tracked = aye + nay");

		let cleared =
			<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::clear_conflicting_votes(
				&alice, 0,
			)
			.unwrap();
		assert!(cleared >= 1, "Split vote must be cleared");
		assert!(
			pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, ref_index).is_none(),
			"record gone after clearance"
		);
	});
}

/// Liquidation succeeds when the borrower's only vote is a Split vote;
/// the record stays in `UserVoteRecords` (used for `Stakes.frozen` accounting).
#[test]
fn gigahdx_liquidation_should_succeed_when_borrower_has_split_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		// Alice stakes.
		let stake_amount = 10_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));

		// Alice casts a Split vote committing her full stake (aye + nay).
		let ref_index = begin_referendum_by_bob();
		let aye = 6_000 * UNITS;
		let nay = 4_000 * UNITS;
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			AccountVote::Split { aye, nay },
		));
		let rec_pre = pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, ref_index).unwrap();
		assert_eq!(rec_pre.staked_vote_amount, aye + nay);
		assert_eq!(rec_pre.weighted, 0, "Split votes earn no rewards");
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().frozen,
			aye + nay,
			"freeze covers the full Split commitment"
		);

		// Alice borrows HOLLAR.
		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);

		// Crash price → HF < 1.
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		// === Run the liquidation flow ===
		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();

		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - before;
		assert!(actual_seized > 0);

		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();

		// Surgical pyconvot release before finalise_seize.
		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice, seize_hdx,
		)
		.unwrap();

		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_evm_account),
			liq_account.clone(),
			GIGAHDX,
			actual_seized,
		));

		// Post-conditions.
		let alice_stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(alice_stake.hdx, orig_hdx - seize_hdx);
		assert_eq!(
			alice_stake.frozen, alice_stake.hdx,
			"frozen clamped to remaining hdx (vote kept intact)"
		);
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), alice_stake.hdx);

		// The Split vote record is still present in UserVoteRecords because
		// liquidation no longer clears votes.
		assert!(
			pallet_gigahdx_rewards::UserVoteRecords::<Runtime>::get(&alice, ref_index).is_some(),
			"Split vote record stays intact"
		);
	});
}

/// Audit3 Finding 2: seize must succeed when the borrower has an active
/// conviction vote covering the full stake. `pyconvot` drops by exactly
/// `seize_hdx`; the vote stays in `VotingFor`.
#[test]
fn liquidate_should_succeed_when_borrower_has_active_conviction_vote() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		// Tighten Alice's balance — free balance just above the staked amount.
		let stake_amount = 10_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			stake_amount + 100 * UNITS,
		));

		// Active Locked3x vote committing the full stake → pyconvot = 10k.
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(stake_amount, Conviction::Locked3x),
		));
		let pyconvot_pre = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(pyconvot_pre, stake_amount);

		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		// Replay the pallet flow step by step (the extrinsic path hits an
		// unrelated EVM borrow revert inside `#[transactional]`).
		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();

		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();

		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		assert!(seize_hdx > 0);

		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice, seize_hdx,
		)
		.unwrap();

		let pyconvot_after_release = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(pyconvot_after_release, stake_amount - seize_hdx);

		assert_ok!(<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		));

		let alice_stake = pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap();
		assert_eq!(alice_stake.hdx, orig_hdx - seize_hdx);
		assert_eq!(alice_stake.gigahdx, orig_gigahdx - actual_seized);
		assert_eq!(alice_stake.frozen, alice_stake.hdx);

		let voting = pallet_conviction_voting::VotingFor::<Runtime>::get(&alice, 0u16);
		match voting {
			pallet_conviction_voting::Voting::Casting(casting) => {
				assert_eq!(casting.votes.len(), 1);
				assert_eq!(casting.votes[0].0, ref_index);
			}
			pallet_conviction_voting::Voting::Delegating(_) => panic!("unexpected delegating state"),
		}
	});
}

/// When the borrower's reducible balance exceeds `seize_hdx`, the seize
/// transfer succeeds — but the HDX physically comes out of the
/// transferable buffer rather than the staked portion. `Stakes` shrinks
/// correctly; the stale `ormlvest` lock now over-commits the smaller
/// balance until it naturally clears.
#[test]
fn liquidation_should_seize_from_buffer_when_unrelated_lock_blocks_staked_portion() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use frame_support::traits::fungible::Inspect;
		use frame_support::traits::tokens::{Fortitude, Preservation};
		use frame_support::traits::{LockableCurrency, WithdrawReasons};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		let stake_amount = 10_000 * UNITS;
		let free_buffer = 5_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			stake_amount + free_buffer,
		));

		// ormlvest covers the staked HDX but leaves the free buffer untouched.
		<Balances as LockableCurrency<_>>::set_lock(*b"ormlvest", &alice, stake_amount, WithdrawReasons::TRANSFER);
		let transferable_before =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(transferable_before, free_buffer);

		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let g_before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - g_before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		assert!(
			seize_hdx > 0 && seize_hdx < free_buffer,
			"seize must fit inside the free buffer"
		);

		let wallet_before = Balances::free_balance(&alice);
		assert_ok!(<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		));

		// Wallet and Stakes accounting both moved by exactly seize_hdx.
		assert_eq!(wallet_before - Balances::free_balance(&alice), seize_hdx);
		assert_eq!(
			pallet_gigahdx::Stakes::<Runtime>::get(&alice).unwrap().hdx,
			orig_hdx - seize_hdx,
		);
		assert_eq!(lock_amount(&alice, GIGAHDX_LOCK_ID), stake_amount - seize_hdx);

		// ormlvest is untouched (we don't surgically reduce non-pyconvot locks today).
		let ormlvest_after = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"ormlvest")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(ormlvest_after, stake_amount);

		// The 200 came out of the transferable buffer — that's where the rub is.
		let transferable_after =
			<Balances as Inspect<AccountId>>::reducible_balance(&alice, Preservation::Expendable, Fortitude::Polite);
		assert_eq!(
			transferable_after,
			transferable_before - seize_hdx,
			"transferable buffer shrinks by seize_hdx because ormlvest didn't release"
		);
	});
}

/// FAILING REGRESSION — a non-protocol lock (e.g. `ormlvest`) on the
/// borrower's HDX, added after `giga_stake`, blocks the seize transfer.
/// Liquidation is top priority and must succeed regardless of other locks
/// the borrower might have acquired.
#[test]
fn liquidate_should_succeed_when_borrower_has_unrelated_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use frame_support::traits::{LockableCurrency, WithdrawReasons};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		let stake_amount = 10_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			stake_amount + 100 * UNITS,
		));

		// Some other pallet places a vesting-style lock on Alice's HDX AFTER
		// she staked. There's no runtime gate that prevents this today.
		<Balances as LockableCurrency<_>>::set_lock(*b"ormlvest", &alice, stake_amount, WithdrawReasons::TRANSFER);

		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let g_before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - g_before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();
		assert!(seize_hdx > 0);

		let alice_before = Balances::free_balance(&alice);
		let liq_before = Balances::free_balance(&liq_account);
		let issuance_before = Balances::total_issuance();

		let result = <pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		);
		assert!(
			result.is_ok(),
			"seize must succeed regardless of unrelated locks. Got: {result:?}"
		);

		// Slash + resolve_creating moves the funds end-to-end.
		assert_eq!(alice_before - Balances::free_balance(&alice), seize_hdx);
		assert_eq!(Balances::free_balance(&liq_account) - liq_before, seize_hdx);
		assert_eq!(Balances::total_issuance(), issuance_before);
	});
}

/// After seize: vote stays in `VotingFor` (tally unchanged), lock is the
/// reduced one, and a later `unlock` recomputes — never above the original
/// commitment.
#[test]
fn liquidation_should_leave_vote_record_intact_but_release_lock() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use crate::liquidation::{borrow, get_user_account_data};
		use hydradx_runtime::BorrowingTreasuryAccount;
		use pallet_gigahdx::traits::Seize;
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;
		use sp_core::Get;

		let (alice, _bob, alice_evm, pool, oracle, hollar_addr) = liquidation_test_setup();
		let st_hdx_evm = HydraErc20Mapping::asset_address(ST_HDX);
		let liq_account = <Runtime as pallet_liquidation::Config>::GigaHdxLiquidationAccount::get();
		let treasury_evm = EVMAccounts::evm_address(&BorrowingTreasuryAccount::get());
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);

		let stake_amount = 10_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			stake_amount + 100 * UNITS,
		));

		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(stake_amount, Conviction::Locked3x),
		));

		// Capture pre-liquidation tally for the referendum.
		let tally_before = match pallet_referenda::ReferendumInfoFor::<Runtime>::get(ref_index).unwrap() {
			pallet_referenda::ReferendumInfo::Ongoing(s) => s.tally.ayes,
			_ => panic!("referendum should be ongoing"),
		};
		assert!(tally_before > 0);

		// Drive to undercollateralized + run the seize.
		let borrow_amount: Balance = 5 * HOLLAR_DECIMALS_18;
		borrow(pool, alice_evm, hollar_addr, borrow_amount);
		fund_treasury_for_liquidation(pool);
		crash_st_hdx_price(oracle, st_hdx_evm);
		assert!(
			get_user_account_data(pool, alice_evm).unwrap().health_factor < U256::from(1_000_000_000_000_000_000u128)
		);

		let (orig_hdx, orig_gigahdx) =
			<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::snapshot_stake(&alice).unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::pre_seize(&alice).unwrap();
		let debt_to_cover = borrow_amount / 2;
		borrow(pool, treasury_evm, hollar_addr, debt_to_cover);
		let before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let liq_data = pallet_liquidation::Pallet::<Runtime>::encode_liquidation_call_data(
			ST_HDX,
			HOLLAR_ASSET_ID,
			alice_evm,
			debt_to_cover,
			true,
		);
		assert!(matches!(
			Executor::<Runtime>::call(
				CallContext::new_call(pool, treasury_evm),
				liq_data,
				U256::zero(),
				50_000_000
			)
			.exit_reason,
			fp_evm::ExitReason::Succeed(_)
		));
		let actual_seized = Currencies::free_balance(GIGAHDX, &treasury_evm_account) - before;
		let seize_hdx = (U256::from(orig_hdx) * U256::from(actual_seized) / U256::from(orig_gigahdx)).as_u128();

		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice, seize_hdx,
		)
		.unwrap();
		<pallet_gigahdx::Pallet<Runtime> as Seize<AccountId>>::finalise_seize(
			&alice,
			&liq_account,
			seize_hdx,
			actual_seized,
			orig_gigahdx - actual_seized,
		)
		.unwrap();

		// Vote and tally are unchanged.
		let voting = pallet_conviction_voting::VotingFor::<Runtime>::get(&alice, 0u16);
		match voting {
			pallet_conviction_voting::Voting::Casting(casting) => {
				assert_eq!(casting.votes.len(), 1);
				let (recorded_ref, recorded_vote) = casting.votes[0];
				assert_eq!(recorded_ref, ref_index);
				assert_eq!(recorded_vote.balance(), stake_amount);
			}
			pallet_conviction_voting::Voting::Delegating(_) => panic!("unexpected delegating state"),
		}
		let tally_after = match pallet_referenda::ReferendumInfoFor::<Runtime>::get(ref_index).unwrap() {
			pallet_referenda::ReferendumInfo::Ongoing(s) => s.tally.ayes,
			_ => panic!("referendum should still be ongoing"),
		};
		assert_eq!(tally_after, tally_before);

		let pyconvot_after_seize = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(pyconvot_after_seize, stake_amount - seize_hdx);

		// Fee buffer is transferable post-seize.
		let recipient: AccountId = AccountId::from([42u8; 32]);
		assert_ok!(Balances::transfer_keep_alive(
			RuntimeOrigin::signed(alice.clone()),
			recipient.clone().into(),
			50 * UNITS,
		));

		// `unlock` re-derives from `VotingFor` — never above the original commitment.
		System::set_block_number(System::block_number() + 200 * DAYS);
		let _ = ConvictionVoting::unlock(RuntimeOrigin::signed(alice.clone()), 0u16, alice.clone().into());
		let pyconvot_after_unlock = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert!(pyconvot_after_unlock <= stake_amount);
	});
}

#[test]
fn force_release_vote_lock_should_only_touch_pyconvot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;

		let (alice, _bob, _alice_evm, _pool, _oracle, _hollar_addr) = liquidation_test_setup();

		let stake_amount = 5_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(2_000 * UNITS, Conviction::Locked1x),
		));

		let ghdxlock_pre = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"ghdxlock")
			.map(|l| l.amount)
			.unwrap_or(0);
		let pyconvot_pre = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(ghdxlock_pre, stake_amount);
		assert_eq!(pyconvot_pre, 2_000 * UNITS);

		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice,
			500 * UNITS,
		)
		.unwrap();

		let ghdxlock_post = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"ghdxlock")
			.map(|l| l.amount)
			.unwrap_or(0);
		let pyconvot_post = pallet_balances::Locks::<Runtime>::get(&alice)
			.iter()
			.find(|l| l.id == *b"pyconvot")
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(ghdxlock_post, ghdxlock_pre);
		assert_eq!(pyconvot_post, pyconvot_pre - 500 * UNITS);
	});
}

#[test]
fn force_release_vote_lock_should_remove_lock_when_amount_exceeds_balance() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		use pallet_gigahdx_rewards::traits::ClearConflictingVotes;

		let (alice, _bob, _alice_evm, _pool, _oracle, _hollar_addr) = liquidation_test_setup();

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), 5_000 * UNITS));
		let ref_index = begin_referendum_by_bob();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			ref_index,
			aye_with_conviction(1_000 * UNITS, Conviction::Locked1x),
		));

		<hydradx_runtime::gigahdx::GigaHdxVoteClearance as ClearConflictingVotes<AccountId>>::force_release_vote_lock(
			&alice,
			10_000 * UNITS, // way more than the 1k pyconvot lock
		)
		.unwrap();

		let locks = pallet_balances::Locks::<Runtime>::get(&alice);
		let pyconvot_post = locks.iter().find(|l| l.id == *b"pyconvot");
		assert!(pyconvot_post.is_none());
	});
}
