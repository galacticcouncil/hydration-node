use crate::polkadot_test_net::{hydradx_run_to_next_block, TestNet, ALICE, BOB};
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::traits::Time;
use hydradx_runtime::{Currencies, Runtime, RuntimeOrigin};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::v2::Solver as IceSolver;
use ice_support::Solution;
use orml_traits::MultiCurrency;
use pallet_omnipool::types::SlipFeeConfig;
use primitives::constants::time::MILLISECS_PER_BLOCK;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

// Asset IDs proven to work in existing solver tests
const HDX: u32 = 0;
const BNC: u32 = 14;

// Amounts from solver_execute_solution1 — known to work
const TRADE_AMOUNT: u128 = 10_000_000_000_000;
const MIN_OUT_BNC: u128 = 68_795_189_840;

const PERIOD: u32 = 5;

// 10% slippage — realistic user setting for recurring DCA trades.
// Oracle limit = estimated_out * 0.90, giving the solver enough room across periods
// as the oracle adjusts between blocks.
const DCA_SLIPPAGE: Permill = Permill::from_percent(10);

type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;
type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;

fn enable_slip_fees() {
	assert_ok!(hydradx_runtime::Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: Permill::from_percent(5),
		})
	));
}

fn run_solver_and_submit() -> Solution {
	let block = hydradx_runtime::System::block_number();
	let call = pallet_ice::Pallet::<Runtime>::run(
		block,
		|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
	)
	.expect("Solver should produce a solution");

	let pallet_ice::Call::submit_solution { solution, .. } = call else {
		panic!("Expected submit_solution call");
	};
	let solution_clone = solution.clone();

	hydradx_run_to_next_block();
	assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
		RuntimeOrigin::none(),
		solution,
	));

	solution_clone
}

fn advance_and_solve(n: u32) -> Solution {
	for _ in 0..n {
		hydradx_run_to_next_block();
	}
	run_solver_and_submit()
}

fn submit_dca_hdx_bnc(who: AccountId, budget: Option<u128>) {
	submit_dca_hdx_bnc_with_slippage(who, budget, DCA_SLIPPAGE);
}

fn submit_dca_hdx_bnc_with_slippage(who: AccountId, budget: Option<u128>, slippage: Permill) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who),
		pallet_intent::types::IntentInput {
			data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
				asset_in: HDX,
				asset_out: BNC,
				amount_in: TRADE_AMOUNT,
				amount_out: MIN_OUT_BNC,
				slippage,
				budget,
				period: PERIOD,
			}),
			deadline: None,
			on_resolved: None,
		}
	));
}

// === A. Basic Lifecycle ===

#[test]
fn dca_single_trade_execution() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			// 3% slippage — realistic user setting
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(3));

			let hdx_before = Currencies::total_balance(HDX, &alice);
			let bnc_before = Currencies::total_balance(BNC, &alice);

			assert_eq!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().len(),
				0,
				"Not yet eligible"
			);

			let _s = advance_and_solve(PERIOD);

			assert!(Currencies::total_balance(HDX, &alice) < hdx_before, "HDX decreased");
			assert!(Currencies::total_balance(BNC, &alice) > bnc_before, "BNC increased");

			let remaining: Vec<_> = pallet_intent::Intents::<Runtime>::iter().collect();
			assert_eq!(remaining.len(), 1, "DCA still active");
			match &remaining[0].1.data {
				ice_support::IntentData::Dca(dca) => {
					assert_eq!(dca.remaining_budget, budget - TRADE_AMOUNT);
				}
				_ => panic!("Expected DCA"),
			}

			// Account index still tracks the active DCA
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
		});
}

#[test]
fn dca_multi_period_completes() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 3 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(budget));

			let _s1 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 1");

			let _s2 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 2");

			let _s3 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0, "Completed");

			// Account index cleaned up after DCA completion
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

// Period eligibility is tested in unit tests (dca_intent::should_not_include_dca_before_period_elapsed).
// The snapshot-based integration tests use RelayChainBlockNumberProvider which behaves differently
// from the mock, making period timing assertions unreliable here.

// === B. Rolling Budget ===

#[test]
fn dca_rolling_budget_continues() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), None); // rolling

			for i in 1..=3 {
				let _s = advance_and_solve(PERIOD);
				assert_eq!(
					pallet_intent::Intents::<Runtime>::iter().count(),
					1,
					"Rolling after trade {i}"
				);
			}
		});
}

// === C. Direct Matching ===

#[test]
fn dca_matched_with_opposing_swap() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.endow_account(bob.clone(), BNC, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(5 * TRADE_AMOUNT));

			for _ in 0..PERIOD {
				hydradx_run_to_next_block();
			}

			// Bob opposing swap: BNC → HDX
			let ts = hydradx_runtime::Timestamp::now();
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(bob.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: BNC,
						asset_out: HDX,
						amount_in: TRADE_AMOUNT,
						amount_out: 1_000_000_000_000u128,
						partial: false,
					}),
					deadline: Some(MILLISECS_PER_BLOCK * 100u64 + ts),
					on_resolved: None,
				}
			));

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
			let solution = run_solver_and_submit();
			assert_eq!(solution.resolved_intents.len(), 2);
			assert!(solution.score > 0, "Surplus from direct matching");
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "DCA stays");

			// Alice's DCA still tracked, Bob's swap resolved and cleaned up
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&bob), 0);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&bob).count(), 0);
		});
}

// === D. Cancellation ===

#[test]
fn dca_cancel_mid_execution() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(5 * TRADE_AMOUNT));

			let _s1 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);

			let (id, _) = pallet_intent::Intents::<Runtime>::iter().next().unwrap();
			assert_ok!(hydradx_runtime::Intent::remove_intent(
				RuntimeOrigin::signed(alice.clone()),
				id
			));
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);

			// Account index cleaned up after cancellation
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

// === E. Multiple Users ===

#[test]
fn dca_multiple_users() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.endow_account(bob.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(3 * TRADE_AMOUNT));
			submit_dca_hdx_bnc(bob.clone(), Some(3 * TRADE_AMOUNT));

			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 2);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 2);

			// Each user has exactly 1 intent tracked
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&bob), 1);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&bob).count(), 1);
		});
}

// === F. Slippage Levels ===

#[test]
fn dca_with_3_percent_slippage() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 3 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(3));

			// Execute all 3 trades with tight slippage
			let _s1 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 1");

			let _s2 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 2");

			let _s3 = advance_and_solve(PERIOD);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0, "Completed");

			// Account index cleaned up
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

#[test]
fn dca_with_1_percent_slippage() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			// Very tight 1% slippage — single trade
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(5 * TRADE_AMOUNT), Permill::from_percent(1));

			let _s = advance_and_solve(PERIOD);

			// Should still work for a single trade on fresh snapshot state
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "DCA still active");
		});
}

#[test]
fn ice_dca_driving() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 3 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.enable_slip_fees(Permill::from_percent(5))
		.new_block()
		.submit_dca_intent(
			alice.clone(),
			HDX,
			BNC,
			TRADE_AMOUNT,
			MIN_OUT_BNC,
			Permill::from_percent(3),
			Some(budget),
			PERIOD,
		)
		.advance(PERIOD)
		.run_solver()
		.execute(|| {
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 1");
		})
		.advance(PERIOD)
		.run_solver()
		.execute(|| {
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 2");
		})
		.advance(PERIOD)
		.run_solver()
		.execute(|| {
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0, "Completed");
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

// === Security reproducer: DCA period is NOT enforced at resolve time ===
//
// `validate_dca_intent_resolve` (pallets/intent/src/lib.rs:801-809) checks only
// `amount_in == dca.amount_in` and `amount_out >= dca.amount_out`. It does NOT
// check `current_block >= dca.last_execution_block + dca.period`.
//
// The period is enforced only in `get_valid_intents()` — an OCW-local helper.
// A block author runs their own OCW. They can skip the helper, read DCA
// intents directly from storage, transform them into Swap intents via
// `DcaData::to_swap_data()` (exactly what the helper itself does, minus the
// filter), hand that to the solver, and submit the resulting Solution via
// the normal unsigned extrinsic. `validate_resolve` — called from both the
// pool-entry `validate_unsigned_solution` and the dispatch-time
// `intent_resolved` — has no period check for DCA, so the solution is
// accepted and `resolve_dca_intent` drains one `amount_in` from the budget.
//
// Repeated across consecutive blocks, this drains the entire budget in
// `budget / amount_in` blocks instead of `budget / amount_in * period`
// blocks. Time-averaging and the oracle pre-filter are both bypassed.
//
// This reproducer uses only public APIs — no production code changes.
#[test]
fn dca_period_can_be_bypassed_at_resolve_time() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();

			// Loose 3% slippage — same as `dca_single_trade_execution`, known
			// to produce a feasible solver quote on this snapshot.
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(3));

			// Sanity: the honest OCW helper correctly filters the DCA out —
			// period has not elapsed.
			assert_eq!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().len(),
				0,
				"honest OCW correctly filters DCA before period elapses"
			);

			// Advance a SINGLE block — nowhere near PERIOD=5.
			hydradx_run_to_next_block();
			assert_eq!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().len(),
				0,
				"still not eligible"
			);

			let (intent_id, dca_before) = pallet_intent::Intents::<Runtime>::iter()
				.next()
				.map(|(id, intent)| match intent.data {
					ice_support::IntentData::Dca(dca) => (id, dca),
					_ => panic!("expected DCA"),
				})
				.unwrap();

			// --- bypass: craft an intent list that skips the period filter ---
			// Read Intents storage directly and apply the SAME `to_swap_data()`
			// transform the honest helper uses, but without the
			// `current_block >= last_execution_block + period` gate.
			let crafted_intents: Vec<ice_support::Intent> = pallet_intent::Intents::<Runtime>::iter()
				.map(|(id, intent)| {
					let data = match intent.data {
						ice_support::IntentData::Dca(ref dca) => ice_support::IntentData::Swap(dca.to_swap_data()),
						other => other,
					};
					ice_support::Intent { id, data }
				})
				.collect();
			assert_eq!(crafted_intents.len(), 1, "one crafted swap intent from the DCA");

			let state = <<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::initial_state();

			let solution = Solver::solve(crafted_intents, state)
				.expect("solver produces a fill for the crafted swap intent");
			assert_eq!(
				solution.resolved_intents.len(),
				1,
				"solver resolved the out-of-period DCA as a plain swap"
			);

			// Submit via the normal unsigned dispatch — same call shape a
			// malicious collator would emit from their OCW. `ensure_none`
			// inside the extrinsic matches, and `intent_resolved` →
			// `validate_resolve` → `validate_dca_intent_resolve` runs with
			// NO period check.
			hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			// --- proof of bypass ---
			// The intent is still in storage (budget > amount_in), but one
			// per-trade amount has been drained even though the period never
			// elapsed. A well-behaved DCA would have consumed zero budget
			// until block >= submit_block + PERIOD.
			let intent_after = pallet_intent::Intents::<Runtime>::get(intent_id)
				.expect("DCA still in storage (one trade consumed, budget > amount_in)");
			match intent_after.data {
				ice_support::IntentData::Dca(dca) => {
					assert_eq!(
						dca.remaining_budget,
						dca_before.remaining_budget - TRADE_AMOUNT,
						"one per-trade amount drained despite period not elapsed"
					);
					assert!(
						dca.last_execution_block < dca_before.last_execution_block.saturating_add(dca_before.period),
						"last_execution_block advanced before the period was due — bypass confirmed \
						 (last_execution_block={}, next_eligible_was={})",
						dca.last_execution_block,
						dca_before.last_execution_block.saturating_add(dca_before.period),
					);
				}
				_ => panic!("expected DCA"),
			}
		});
}
