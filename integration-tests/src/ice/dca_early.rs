//! Early DCA execution against the real `mainnet_apr` snapshot: a DCA may trade up to
//! `min(DcaEarlyWindow, period / 2)` blocks before its slot, only when the list holds
//! fillable reverse-pair flow covering it, and an early fill consumes the slot. Period 15
//! ⇒ window 7. `t0` is always read from storage after the first (due) fill.

use crate::polkadot_test_net::{hydradx_run_to_next_block, TestNet, ALICE, BOB};
use frame_support::assert_ok;
use frame_support::traits::Time;
use hydradx_runtime::{Currencies, Runtime, RuntimeOrigin, Timestamp};
use ice_support::{Balance, DcaData, DcaParams, IntentData, IntentDataInput, IntentId, SolverMode, SwapParams};
use orml_traits::MultiCurrency;
use primitives::constants::time::MILLISECS_PER_BLOCK;
use primitives::{AccountId, AssetId};
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::harness::{
	amm_in_for, amm_trade_count, enable_slip_fees, is_resolved, resolved, run_and_submit_as, solve_as, swap, V4Solver,
};
use super::PATH_TO_SNAPSHOT;

const HDX: AssetId = 0;
const BNC: AssetId = 14;
const HDX_UNIT: Balance = 1_000_000_000_000;
const BNC_UNIT: Balance = 1_000_000_000_000;

const TRADE: Balance = 10 * HDX_UNIT;
const MIN_OUT_BNC: Balance = 68_795_189_840;
const PERIOD: u32 = 15;
const WINDOW: u32 = 7;
const COUNTER_BNC: Balance = 10 * BNC_UNIT;
/// BOB's 10 BNC → HDX AMM output on this snapshot.
const BOB_MARKET_OUT: Balance = 147_407_011_313_812;
const BOB_DCA_TRADE: Balance = 400_000_000_000;

const PIN_FIRST_OUT: Balance = 674_325_708_682;
const PIN_EARLY_OUT: Balance = 676_286_282_545;
const PIN_EARLY_BOB_AMM_IN: Balance = 9_323_578_433_142;
const PIN_SLOT_OUT: Balance = 674_325_474_839;
const PIN_TOGETHER_ALICE_AMM_IN: Balance = 4_086_529_207_152;

fn step() {
	hydradx_run_to_next_block();
	hydradx_runtime::System::reset_events();
	Timestamp::set_timestamp(Timestamp::now() + MILLISECS_PER_BLOCK);
}

fn block() -> u32 {
	hydradx_runtime::System::block_number()
}

fn step_to(target: u32) {
	while block() < target {
		step();
	}
	assert_eq!(block(), target);
}

fn intent_of(owner: &AccountId) -> IntentId {
	pallet_intent::Intents::<Runtime>::iter_keys()
		.find(|id| pallet_intent::Pallet::<Runtime>::intent_owner(*id).as_ref() == Some(owner))
		.expect("owner has a stored intent")
}

fn in_list(id: IntentId) -> bool {
	pallet_intent::Pallet::<Runtime>::get_valid_intents()
		.iter()
		.any(|(i, _)| *i == id)
}

fn stored_dca(id: IntentId) -> DcaData {
	match pallet_intent::Pallet::<Runtime>::get_intent(id)
		.expect("intent stored")
		.data
	{
		IntentData::Dca(d) => d,
		_ => panic!("expected a DCA intent"),
	}
}

fn bnc_of(who: &AccountId) -> Balance {
	Currencies::total_balance(BNC, who)
}

fn submit(who: &AccountId, data: IntentDataInput) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who.clone()),
		pallet_intent::types::IntentInput {
			data,
			deadline: None,
			on_resolved: None,
		}
	));
}

fn plain(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> IntentDataInput {
	IntentDataInput::Swap(SwapParams {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		partial: false,
	})
}

fn dca(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	trades: u128,
) -> IntentDataInput {
	IntentDataInput::Dca(DcaParams {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		slippage: Permill::from_percent(10),
		budget: Some(trades * amount_in),
		period: PERIOD,
	})
}

fn bob_swap(bob: &AccountId, amount_out: Balance) -> IntentId {
	submit(bob, plain(BNC, HDX, COUNTER_BNC, amount_out));
	intent_of(bob)
}

fn alice_not_solved(alice_id: IntentId) {
	assert!(!in_list(alice_id));
	assert!(solve_as::<V4Solver>().is_none_or(|s| !is_resolved(&s, alice_id)));
}

/// ALICE's DCA HDX → BNC, due at creation and filled through the AMM. Returns `(id, t0)`.
fn fill_at_t0(alice: &AccountId, trades: u128) -> (IntentId, u32) {
	// Trading in the snapshot's own block trips the EMA oracle's integration on the next step.
	step();
	submit(alice, dca(HDX, BNC, TRADE, MIN_OUT_BNC, trades));
	let id = intent_of(alice);
	let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_first_fill");
	assert_eq!(amm_in_for(&sol, HDX, BNC), TRADE);
	let first_out = swap(resolved(&sol, id)).amount_out;
	assert_eq!(first_out, PIN_FIRST_OUT);
	let t0 = stored_dca(id).last_execution_block;
	assert_eq!(t0, block());
	(id, t0)
}

fn with_pair_endowed(execution: impl FnOnce(AccountId, AccountId)) {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, 2_000 * HDX_UNIT);
	driver.endow_account(bob.clone(), BNC, 500 * BNC_UNIT);
	driver.endow_account(bob.clone(), HDX, 100 * HDX_UNIT);
	driver.execute(|| {
		enable_slip_fees();
		execution(alice, bob);
	});
}

#[test]
fn dca_should_execute_early_when_opposite_flow_arrives_within_the_window() {
	with_pair_endowed(|alice, bob| {
		let (id, t0) = fill_at_t0(&alice, 3);

		step_to(t0 + PERIOD - WINDOW - 1);
		alice_not_solved(id);

		step_to(t0 + PERIOD - WINDOW);
		alice_not_solved(id);

		let bob_id = bob_swap(&bob, HDX_UNIT);
		assert!(in_list(id));

		let bnc_before = bnc_of(&alice);
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_headline");
		let early_out = bnc_of(&alice) - bnc_before;

		assert_eq!(sol.resolved_intents.len(), 2);
		assert!(is_resolved(&sol, id));
		assert!(is_resolved(&sol, bob_id));
		assert_eq!(amm_trade_count(&sol), 1);
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0, "ALICE's HDX never touches a pool");
		assert_eq!(amm_in_for(&sol, BNC, HDX), PIN_EARLY_BOB_AMM_IN);

		let stored = stored_dca(id);
		assert_eq!(stored.last_execution_block, t0 + PERIOD);
		assert!(stored.last_execution_block > block());
		assert_eq!(stored.remaining_budget, TRADE);
		assert_eq!(early_out, PIN_EARLY_OUT);
		assert!(early_out > PIN_FIRST_OUT, "matched early beats the AMM fill");
	});
}

#[test]
fn dca_should_not_execute_twice_in_one_period_when_opposite_flow_arrives_every_block() {
	with_pair_endowed(|alice, bob| {
		let (id, t0) = fill_at_t0(&alice, 5);

		step_to(t0 + PERIOD - WINDOW);
		bob_swap(&bob, HDX_UNIT);
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_every_block_first");
		assert!(is_resolved(&sol, id));
		assert_eq!(stored_dca(id).last_execution_block, t0 + PERIOD);

		for b in t0 + PERIOD - WINDOW + 1..=t0 + 2 * PERIOD - WINDOW - 1 {
			step();
			assert_eq!(block(), b);
			let bob_id = bob_swap(&bob, HDX_UNIT);
			assert!(!in_list(id));
			let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_every_block");
			assert_eq!(sol.resolved_intents.len(), 1);
			assert!(is_resolved(&sol, bob_id));
			assert!(!is_resolved(&sol, id));
			assert_eq!(stored_dca(id).last_execution_block, t0 + PERIOD);
		}

		step();
		assert_eq!(block(), t0 + 2 * PERIOD - WINDOW);
		bob_swap(&bob, HDX_UNIT);
		assert!(in_list(id));
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_every_block_second");
		assert!(is_resolved(&sol, id));
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0);
		let stored = stored_dca(id);
		assert_eq!(stored.last_execution_block, t0 + 2 * PERIOD);
		assert_eq!(stored.remaining_budget, 2 * TRADE);
	});
}

#[test]
fn dca_should_execute_at_its_slot_without_a_counterparty() {
	with_pair_endowed(|alice, _bob| {
		let (id, t0) = fill_at_t0(&alice, 3);

		while block() < t0 + PERIOD - 1 {
			step();
			alice_not_solved(id);
		}

		step();
		assert_eq!(block(), t0 + PERIOD);
		assert!(in_list(id));
		let bnc_before = bnc_of(&alice);
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_at_slot");
		let slot_out = bnc_of(&alice) - bnc_before;

		assert_eq!(amm_in_for(&sol, HDX, BNC), TRADE);
		assert_eq!(stored_dca(id).last_execution_block, t0 + PERIOD);
		assert_eq!(slot_out, PIN_SLOT_OUT);
	});
}

#[test]
fn dca_should_not_execute_early_when_the_only_counterparty_is_above_market() {
	with_pair_endowed(|alice, bob| {
		let (id, t0) = fill_at_t0(&alice, 3);

		step_to(t0 + PERIOD - WINDOW);
		let bob_id = bob_swap(&bob, 3 * BOB_MARKET_OUT);
		loop {
			assert!(in_list(bob_id));
			alice_not_solved(id);
			if block() == t0 + PERIOD - 1 {
				break;
			}
			step();
		}

		step();
		assert_eq!(block(), t0 + PERIOD);
		assert!(in_list(id));
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_above_market");
		assert!(is_resolved(&sol, id));
		assert!(!is_resolved(&sol, bob_id));
		assert_eq!(amm_in_for(&sol, HDX, BNC), TRADE);
		assert!(pallet_intent::Pallet::<Runtime>::get_intent(bob_id).is_some());
		assert_eq!(stored_dca(id).last_execution_block, t0 + PERIOD);
	});
}

#[test]
fn opposite_dcas_should_execute_together_when_one_is_within_its_window() {
	with_pair_endowed(|alice, bob| {
		let (alice_id, t0) = fill_at_t0(&alice, 3);

		step_to(t0 + 4);
		submit(&bob, dca(BNC, HDX, BOB_DCA_TRADE, HDX_UNIT, 3));
		let bob_id = intent_of(&bob);
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_bob_first_fill");
		assert!(is_resolved(&sol, bob_id));
		assert_eq!(stored_dca(bob_id).last_execution_block, t0 + 4);

		// BOB's ≈ 5.9 HDX of counter-flow leaves ALICE's 10 HDX short by more than ED, so
		// ALICE is withheld while early, and an uncovered BOB is withheld with her.
		while block() < t0 + PERIOD - 1 {
			step();
			assert!(!in_list(alice_id));
			assert!(!in_list(bob_id));
		}

		step();
		assert_eq!(block(), t0 + PERIOD);
		assert!(in_list(alice_id));
		assert!(in_list(bob_id));
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "dca_early_together");

		assert_eq!(sol.resolved_intents.len(), 2);
		assert_eq!(amm_in_for(&sol, BNC, HDX), 0, "BOB's BNC never touches a pool");
		assert_eq!(amm_in_for(&sol, HDX, BNC), PIN_TOGETHER_ALICE_AMM_IN);
		assert_eq!(stored_dca(alice_id).last_execution_block, t0 + PERIOD);
		assert_eq!(stored_dca(bob_id).last_execution_block, t0 + 4 + PERIOD);
		assert!(stored_dca(bob_id).last_execution_block > block());
	});
}
