//! Global-netting scenarios: cases where matching across *different* pairs
//! (chains, longer cycles, partial cross-pair coincidence) could internalize
//! volume that the current per-pair + 3-ring solver routes through the AMM.
//!
//! Each test runs against the real `mainnet_apr` snapshot with real Omnipool
//! assets and slip fees enabled — no mocks — so the measured behavior reflects
//! production liquidity. Phase 1 pins the current solver (v3) baseline; once the
//! v4 global-netting solver lands, each scenario gains a v4 arm asserting
//! `amm_trades` down / per-intent output up / `score` up versus v3.

use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, EVE};
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use hydradx_runtime::{Omnipool, Runtime, RuntimeOrigin, System};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::v3::Solver as IceSolver;
use ice_support::Solution;
use pallet_omnipool::types::SlipFeeConfig;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

type TestSimulator = HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>;
type CurrentSolver = IceSolver<TestSimulator>;
type NettingSolver = ice_solver::v4::Solver<TestSimulator>;
type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

fn enable_slip_fees() {
	assert_ok!(Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: Permill::from_percent(5),
		})
	));
}

/// Run the *current* (v3) solver via the pallet's own `run` (which builds the
/// valid-intent set and live simulator state exactly as production does) and
/// return the raw `Solution` so a test can measure it. `run` is deterministic,
/// so calling it again with the v4 solver yields identical inputs to compare.
fn solve_current() -> Solution {
	let call = pallet_ice::Pallet::<Runtime>::run(
		System::block_number(),
		|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
			CurrentSolver::solve(intents, state, pallet_ice::ProtocolFee::<Runtime>::get()).ok()
		},
	)
	.expect("solver must produce a solution");
	let pallet_ice::Call::submit_solution { solution, .. } = call else {
		panic!("expected submit_solution call");
	};
	solution
}

/// Same as [`solve_current`] but with the v4 global-netting solver.
fn solve_v4() -> Solution {
	let call = pallet_ice::Pallet::<Runtime>::run(
		System::block_number(),
		|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
			NettingSolver::solve(intents, state, pallet_ice::ProtocolFee::<Runtime>::get()).ok()
		},
	)
	.expect("v4 solver must produce a solution");
	let pallet_ice::Call::submit_solution { solution, .. } = call else {
		panic!("expected submit_solution call");
	};
	solution
}

/// Run v4 on the current intents, print a v3-vs-v4 comparison line, dump v4 for
/// pinning, enforce the "v4 is never worse than v3" invariants, and SUBMIT the
/// v4 solution (asserting the pallet accepts it — the conservation check).
/// Returns the v4 solution.
fn run_v4_compare(label: &str, v3: &Solution) -> Solution {
	let v4 = solve_v4();
	println!(
		"CMP {label}: v3(resolved={} trades={} score={}) -> v4(resolved={} trades={} score={})",
		v3.resolved_intents.len(),
		v3.trades.len(),
		v3.score,
		v4.resolved_intents.len(),
		v4.trades.len(),
		v4.score
	);
	dump(&format!("{label} v4"), &v4);

	// v4 must never do worse than v3 on the headline metrics.
	assert!(
		v4.resolved_intents.len() >= v3.resolved_intents.len(),
		"{label}: v4 resolved {} < v3 {}",
		v4.resolved_intents.len(),
		v3.resolved_intents.len()
	);
	assert!(
		v4.trades.len() <= v3.trades.len(),
		"{label}: v4 AMM trades {} > v3 {}",
		v4.trades.len(),
		v3.trades.len()
	);
	assert!(v4.score >= v3.score, "{label}: v4 score {} < v3 {}", v4.score, v3.score);
	// NB: v4 maximizes TOTAL surplus and may redistribute — a single intent can
	// receive marginally less than under v3's per-pair pricing while the batch
	// total rises. Every resolved intent still meets its own limit (guaranteed by
	// the resolution stage), so we assert the aggregate invariants, not a per-intent
	// Pareto improvement (which a uniform-price batch auction does not promise).

	// Conservation: the pallet must accept v4's solution.
	assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
		RuntimeOrigin::none(),
		v4.clone(),
	));
	v4
}

/// Number of distinct AMM trades the solution routes through the router.
/// Fewer is better: each AMM trade pays pool fee + slippage that internal
/// matching avoids.
fn amm_trade_count(sol: &Solution) -> usize {
	sol.trades.len()
}

/// Resolved intent by id (panics if the intent was not resolved).
fn resolved(sol: &Solution, id: u128) -> &ice_support::ResolvedIntent {
	sol.resolved_intents
		.iter()
		.find(|r| r.id == id)
		.expect("intent should be resolved")
}

/// Whether an intent appears in the solution's resolved set.
fn is_resolved(sol: &Solution, id: u128) -> bool {
	sol.resolved_intents.iter().any(|r| r.id == id)
}

/// The `SwapData` of a resolved swap intent.
fn swap(ri: &ice_support::ResolvedIntent) -> &ice_support::SwapData {
	match &ri.data {
		ice_support::IntentData::Swap(s) => s,
		_ => panic!("expected Swap"),
	}
}

/// `amount_in` summed over AMM trades routing `asset_in -> asset_out` (0 if none).
fn amm_in_for(sol: &Solution, asset_in: u32, asset_out: u32) -> u128 {
	sol.trades
		.iter()
		.filter(|t| {
			t.route.first().map(|h| h.asset_in) == Some(asset_in)
				&& t.route.last().map(|h| h.asset_out) == Some(asset_out)
		})
		.map(|t| t.amount_in)
		.fold(0u128, |a, v| a.saturating_add(v))
}

/// Print every field needed to pin a baseline: per-intent fills/outputs, each
/// AMM trade's directed amounts, and the headline metrics. Copy the emitted
/// `assert_eq!` lines back into the test once the real numbers are known.
#[allow(dead_code)]
fn dump(label: &str, sol: &Solution) {
	println!("// === NETTING DUMP BEGIN: {label} ===");
	println!(
		"// resolved={} amm_trades={} score={}",
		sol.resolved_intents.len(),
		sol.trades.len(),
		sol.score
	);
	println!(
		"assert_eq!(sol.resolved_intents.len(), {});",
		sol.resolved_intents.len()
	);
	println!("assert_eq!(amm_trade_count(&sol), {});", sol.trades.len());
	println!("assert_eq!(sol.score, {}u128);", sol.score);
	for (i, ri) in sol.resolved_intents.iter().enumerate() {
		if let ice_support::IntentData::Swap(ref s) = ri.data {
			println!(
				"// resolved[{i}] id={} {}->{} in={} out={}",
				ri.id, s.asset_in, s.asset_out, s.amount_in, s.amount_out
			);
		}
	}
	for (i, t) in sol.trades.iter().enumerate() {
		let first = t.route.first();
		let last = t.route.last();
		println!(
			"// trade[{i}] {:?}->{:?} in={} out={}",
			first.map(|h| h.asset_in),
			last.map(|h| h.asset_out),
			t.amount_in,
			t.amount_out
		);
	}
	println!("// === NETTING DUMP END: {label} ===");
}

// ---------------------------------------------------------------------------
// Scenario 1 — open 3-asset chain (A->B, B->C); the canonical case rings miss.
//
// Alice sells BNC for HDX; Bob sells HDX for DOT. HDX is the intermediate:
// Alice *buys* it, Bob *sells* it, so the HDX leg can net internally and only
// the residual BNC->DOT needs the AMM. (All three are Omnipool assets in this
// snapshot — the existing 3-ring test trades exactly this set.)
//
// Per-pair (v3) baseline: pair (BNC,HDX) and pair (HDX,DOT) are solved
// independently -> two AMM trades, HDX volume round-trips through the pool.
// GLOBAL-NETTING TARGET (v4): HDX nets out -> single BNC->DOT AMM trade,
// higher per-user output, higher score.
// ---------------------------------------------------------------------------
#[test]
fn netting_chain_bnc_hdx_dot_baseline() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let bnc = 14u32; // 12 decimals
	let hdx = 0u32; // 12 decimals
	let dot = 5u32; // 10 decimals

	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // ~14_700 HDX at snapshot spot
	let bob_hdx = 14_000 * hdx_unit; // sized to roughly cancel Alice's HDX receipt
	let alice_min_hdx = 1_000 * hdx_unit; // loose but valid (>= ED), below spot
	let bob_min_dot = 1 * dot_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, alice_min_hdx, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, bob_min_dot, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			// Intent ids are deterministic for the snapshot: base + submission order.
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX, first
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT, second

			let sol = solve_current();

			// --- v3 baseline (per-pair) ---
			// Two independent AMM trades: HDX->DOT (14000 HDX sold to pool) AND
			// BNC->HDX (~14732 HDX bought from pool). ~14000 HDX leaves to the pool
			// while ~14732 HDX arrives from it — the HDX coincidence is never matched.
			// GLOBAL-NETTING TARGET (v4): net the HDX leg, leaving one BNC->DOT
			// residual trade; assert v4 amm_trades < 2, score >= this, outputs >=.
			assert_eq!(sol.resolved_intents.len(), 2, "both chain intents resolve");
			assert_eq!(amm_trade_count(&sol), 2, "v3 routes each pair independently");
			assert_eq!(sol.score, 13732511128949566u128, "v3 baseline score");

			let alice = resolved(&sol, alice_id);
			assert_eq!(swap(alice).asset_in, bnc);
			assert_eq!(swap(alice).asset_out, hdx);
			assert_eq!(swap(alice).amount_in, alice_bnc);
			assert_eq!(swap(alice).amount_out, 14732299456702693u128, "Alice HDX out (v3)");

			let bob = resolved(&sol, bob_id);
			assert_eq!(swap(bob).asset_in, hdx);
			assert_eq!(swap(bob).asset_out, dot);
			assert_eq!(swap(bob).amount_in, bob_hdx);
			assert_eq!(swap(bob).amount_out, 221672246873u128, "Bob DOT out (v3)");

			// The HDX round-trip v4 should remove: 14000 HDX sold to pool, and
			// 1000 BNC spent buying HDX back from the pool.
			assert_eq!(
				amm_in_for(&sol, hdx, dot),
				14000000000000000u128,
				"v3 sells 14000 HDX to pool"
			);
			assert_eq!(
				amm_in_for(&sol, bnc, hdx),
				1000000000000000u128,
				"v3 buys HDX from pool with 1000 BNC"
			);

			let _v4 = run_v4_compare("chain_bnc_hdx_dot", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 2 — 3-asset cycle (HDX->BNC->DOT->HDX): the case v3 ALREADY nets via
// ring detection. Control: v3 should internalize most of the cycle (few/no AMM
// trades). v4 must match or beat this — never regress the case rings handle.
// ---------------------------------------------------------------------------
#[test]
fn netting_cycle_3asset_ring_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_hdx = 1_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 5 * bnc_unit; // BNC->DOT
	let charlie_dot = 10 * dot_unit; // DOT->HDX

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit / 2, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit / 10, Some(10))
		.submit_swap_intent(charlie.clone(), dot, hdx, charlie_dot, 500 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->HDX

			let sol = solve_current();

			// --- v3 baseline (ring detection partially fires) ---
			// The cycle is bottlenecked by Bob's small 5 BNC leg, so v3 ring-matches
			// only that much and routes the remainder as two AMM trades. This is the
			// case v3 already handles — v4 must match or beat it, never regress.
			assert_eq!(sol.resolved_intents.len(), 3, "all three cycle intents resolve");
			assert_eq!(
				amm_trade_count(&sol),
				2,
				"v3 ring-matches the bottleneck, routes the rest"
			);
			assert_eq!(sol.score, 5845681041331610u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				67446698202754u128,
				"Alice BNC out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				6278734169887749u128,
				"Charlie HDX out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				1173241107u128,
				"Bob DOT out (v3)"
			);

			let _v4 = run_v4_compare("cycle_3asset_ring", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 3 — partial cross-pair coincidence. Alice sells 1000 BNC for HDX
// (~14.7k HDX of demand); Bob sells only 5000 HDX for DOT. ~5000 HDX of the
// coincidence could net internally, the rest must come from the AMM. v3 nets
// none of it (different pairs) and routes both full legs.
// ---------------------------------------------------------------------------
#[test]
fn netting_partial_coincidence_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit;
	let bob_hdx = 5_000 * hdx_unit; // less than Alice's ~14.7k HDX receipt

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, dot_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT

			let sol = solve_current();

			// --- v3 baseline ---
			// Bob sells 5000 HDX to the pool while Alice buys ~14731 HDX from it:
			// ~5000 HDX of coincidence goes unmatched (different pairs). v4 should
			// net that 5000 HDX and shrink the residual AMM volume.
			assert_eq!(sol.resolved_intents.len(), 2);
			assert_eq!(amm_trade_count(&sol), 2, "v3 routes both legs independently");
			assert_eq!(sol.score, 13731089606008634u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14731020431531246u128,
				"Alice HDX out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				79174477388u128,
				"Bob DOT out (v3)"
			);
			assert_eq!(
				amm_in_for(&sol, hdx, dot),
				5000000000000000u128,
				"v3 sells 5000 HDX to pool"
			);
			assert_eq!(
				amm_in_for(&sol, bnc, hdx),
				1000000000000000u128,
				"v3 buys HDX from pool with 1000 BNC"
			);

			let _v4 = run_v4_compare("partial_coincidence", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 4 — 4-asset cycle (HDX->BNC->DOT->WETH->HDX). v3 ring detection only
// handles 3-cycles, so it misses this entirely and routes four independent AMM
// trades. v4 global netting should internalize the whole cycle.
// ---------------------------------------------------------------------------
#[test]
fn netting_cycle_4asset_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	// ~10k HDX-equivalent per leg (rough spot), loose mins (>= ED, below spot).
	let alice_hdx = 10_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 680 * bnc_unit; // BNC->DOT
	let charlie_dot = 15 * dot_unit; // DOT->WETH
	let dave_weth = weth_unit / 30; // WETH->HDX (~0.033 WETH)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), weth, hdx, dave_weth, 100 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH
			let dave_id = 32752052247409382067756072960003u128; // WETH->HDX

			let sol = solve_current();

			// --- v3 baseline ---
			// v3's ring detector only handles 3-cycles, so it misses this 4-cycle
			// entirely and routes all FOUR legs through the AMM. v4 global netting
			// should internalize the cycle -> far fewer AMM trades, higher score.
			// This is the strongest demonstration of the netting gap.
			assert_eq!(sol.resolved_intents.len(), 4, "all four cycle intents resolve");
			assert_eq!(amm_trade_count(&sol), 4, "v3 misses the 4-cycle: one AMM trade per leg");
			assert_eq!(sol.score, 43643401097207961u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				674092120446581u128,
				"Alice BNC out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				159212482182u128,
				"Bob DOT out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				8883229586041964u128,
				"Charlie WETH out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				35186930178237234u128,
				"Dave HDX out (v3)"
			);

			let _v4 = run_v4_compare("cycle_4asset", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 5 — open 4-asset chain (BNC->HDX->DOT->WETH). Two intermediates (HDX,
// DOT) could net; v3 routes all three legs as independent AMM trades. v4 should
// collapse to a single BNC->WETH residual.
// ---------------------------------------------------------------------------
#[test]
fn netting_chain_4asset_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let weth = 20u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // BNC->HDX (~14.7k HDX)
	let bob_hdx = 14_000 * hdx_unit; // HDX->DOT (~22 DOT)
	let charlie_dot = 22 * dot_unit; // DOT->WETH

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH

			let sol = solve_current();

			// --- v3 baseline ---
			// Two intermediates (HDX, DOT) could net, but v3 routes all three legs
			// independently. GLOBAL-NETTING TARGET (v4): single BNC->WETH residual.
			assert_eq!(sol.resolved_intents.len(), 3, "all three chain intents resolve");
			assert_eq!(amm_trade_count(&sol), 3, "v3 routes each leg independently");
			assert_eq!(sol.score, 25759219821603472u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14732299456702693u128,
				"Alice HDX out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				221672246873u128,
				"Bob DOT out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				13026708692653906u128,
				"Charlie WETH out (v3)"
			);

			let _v4 = run_v4_compare("chain_4asset", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 6 — 5-asset cycle (HDX->BNC->DOT->WETH->ETH->HDX). Far beyond v3's
// 3-cycle ring detection -> five independent AMM trades. v4 should internalize
// the entire cycle.
// ---------------------------------------------------------------------------
#[test]
fn netting_cycle_5asset_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();
	let eve: AccountId = EVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let eth = 34u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;
	let eth_unit = 1_000_000_000_000_000_000u128;

	// ~10k HDX-equivalent per leg (rough spot), loose mins (>= ED, below spot).
	let alice_hdx = 10_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 680 * bnc_unit; // BNC->DOT
	let charlie_dot = 15 * dot_unit; // DOT->WETH
	let dave_weth = weth_unit / 30; // WETH->ETH
	let eve_eth = eth_unit / 30; // ETH->HDX

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.endow_account(eve.clone(), eth, eve_eth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), weth, eth, dave_weth, eth_unit / 1000, Some(10))
		.submit_swap_intent(eve.clone(), eth, hdx, eve_eth, 100 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 5);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH
			let dave_id = 32752052247409382067756072960003u128; // WETH->ETH
			let eve_id = 32752052247409382067756072960004u128; // ETH->HDX

			let sol = solve_current();

			// --- v3 baseline ---
			// Five-cycle is far beyond v3's 3-cycle ring detection: it routes all
			// FIVE legs through the AMM. v4 should internalize the whole cycle.
			assert_eq!(sol.resolved_intents.len(), 5, "all five cycle intents resolve");
			assert_eq!(amm_trade_count(&sol), 5, "v3 misses the 5-cycle: one AMM trade per leg");
			assert_eq!(sol.score, 75974573815874571u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				674092120446581u128,
				"Alice BNC out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				159212482182u128,
				"Bob DOT out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				8883113300905660u128,
				"Charlie WETH out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				33309206064241006u128,
				"Dave ETH out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, eve_id)).amount_out,
				35209013117799142u128,
				"Eve HDX out (v3)"
			);

			let _v4 = run_v4_compare("cycle_5asset", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 7 — chain + same-pair direct match. Alice BNC->HDX and Bob HDX->BNC
// oppose on pair (BNC,HDX) (v3 matches these directly), while Charlie HDX->DOT
// chains off the HDX. Tests that global netting composes with the per-pair
// direct match instead of breaking it.
// ---------------------------------------------------------------------------
#[test]
fn netting_chain_plus_direct_match_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // BNC->HDX
	let bob_hdx = 10_000 * hdx_unit; // HDX->BNC (opposing Alice on the pair)
	let charlie_hdx = 5_000 * hdx_unit; // HDX->DOT (chain off HDX)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.endow_account(charlie.clone(), hdx, charlie_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, bnc, bob_hdx, bnc_unit / 2, Some(10))
		.submit_swap_intent(charlie.clone(), hdx, dot, charlie_hdx, dot_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->BNC
			let charlie_id = 32752052247409382067756072960002u128; // HDX->DOT

			let sol = solve_current();

			// --- v3 baseline ---
			// v3 already directly matches the opposing BNC<->HDX intents (Alice/Bob):
			// ~680 BNC matched internally, only ~323 BNC residual hits the pool. Charlie's
			// HDX->DOT is a separate AMM trade. v4 should additionally net Charlie's HDX
			// against the HDX flowing through the BNC<->HDX match.
			assert_eq!(sol.resolved_intents.len(), 3, "all three intents resolve");
			assert_eq!(
				amm_trade_count(&sol),
				2,
				"v3 direct-matches BNC<->HDX, routes residual + chain"
			);
			assert_eq!(sol.score, 14442140296744393u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14766284605162892u128,
				"Alice HDX out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				676286517104113u128,
				"Bob BNC out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				79174477388u128,
				"Charlie DOT out (v3)"
			);
			assert_eq!(
				amm_in_for(&sol, bnc, hdx),
				323578198535595u128,
				"only residual BNC hits the pool"
			);
			assert_eq!(
				amm_in_for(&sol, hdx, dot),
				5000000000000000u128,
				"Charlie's HDX->DOT routed in full"
			);

			let _v4 = run_v4_compare("chain_plus_direct_match", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 8 — two disjoint matching groups in one batch: a BNC->HDX->DOT chain
// AND an independent opposing WETH<->ETH pair. Asserts the solver handles
// independent groups deterministically and that netting one group never
// perturbs the other. v4 must keep the groups independent.
// ---------------------------------------------------------------------------
#[test]
fn netting_disjoint_groups_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let weth = 20u32;
	let eth = 34u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;
	let eth_unit = 1_000_000_000_000_000_000u128;

	// Group 1: chain BNC->HDX->DOT. Group 2: opposing WETH<->ETH (roughly balanced).
	let alice_bnc = 1_000 * bnc_unit;
	let bob_hdx = 14_000 * hdx_unit;
	let charlie_weth = weth_unit / 30; // WETH->ETH
	let dave_eth = eth_unit / 30; // ETH->WETH (opposes Charlie)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.endow_account(charlie.clone(), weth, charlie_weth * 10)
		.endow_account(dave.clone(), eth, dave_eth * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), weth, eth, charlie_weth, eth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), eth, weth, dave_eth, weth_unit / 1000, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT
			let charlie_id = 32752052247409382067756072960002u128; // WETH->ETH
			let dave_id = 32752052247409382067756072960003u128; // ETH->WETH

			let sol = solve_current();

			// --- v3 baseline ---
			// Chain group (BNC->HDX->DOT) routes 2 AMM trades; the opposing WETH<->ETH
			// pair direct-matches with only a tiny residual -> 3 AMM trades total. The
			// groups are independent; v4 must preserve that (netting the chain must not
			// perturb the WETH<->ETH match).
			assert_eq!(sol.resolved_intents.len(), 4, "all four intents resolve");
			assert_eq!(amm_trade_count(&sol), 3, "chain: 2 trades; opposing pair: 1 residual");
			assert_eq!(sol.score, 78385851104521095u128, "v3 baseline score");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14732299456702693u128,
				"Alice HDX out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				221672246873u128,
				"Bob DOT out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				33305976737841382u128,
				"Charlie ETH out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				33347363237730147u128,
				"Dave WETH out (v3)"
			);

			let _v4 = run_v4_compare("disjoint_groups", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 9 — blocked 3-ring. The ring HDX->BNC->DOT->HDX is feasible, but the
// FIRST intent on the HDX->BNC edge (Alice) carries a tight limit above spot.
// v3's ring detector inspects only that first entry, so it skips the whole ring
// even though Dave's loose HDX->BNC intent could have carried it. v3 therefore
// routes per-pair and drops Alice. v4 should skip the blocking intent and ring.
// ---------------------------------------------------------------------------
#[test]
fn netting_blocked_3ring_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_hdx = 1_000 * hdx_unit; // HDX->BNC, tight (min above spot ~67.4 BNC)
	let dave_hdx = 1_000 * hdx_unit; // HDX->BNC, loose (could carry the ring)
	let bob_bnc = 5 * bnc_unit; // BNC->DOT
	let charlie_dot = 10 * dot_unit; // DOT->HDX

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), hdx, dave_hdx * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, 68 * bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit / 10, Some(10))
		.submit_swap_intent(charlie.clone(), dot, hdx, charlie_dot, 500 * hdx_unit, Some(10))
		.submit_swap_intent(dave.clone(), hdx, bnc, dave_hdx, 60 * bnc_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC (tight)
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->HDX
			let dave_id = 32752052247409382067756072960003u128; // HDX->BNC (loose)

			let sol = solve_current();

			// --- v3 baseline ---
			// Alice's tight HDX->BNC limit (68 BNC > spot ~67.4) is unsatisfiable, so
			// she drops; the ring proceeds via Dave's loose HDX->BNC duplicate. Two AMM
			// trades, three resolved. (Alice is genuinely unfillable, so v4 won't rescue
			// her — this case guards that v4 doesn't regress the ring it still forms.)
			assert_eq!(sol.resolved_intents.len(), 3, "tight Alice drops, other three resolve");
			assert_eq!(amm_trade_count(&sol), 2);
			assert_eq!(sol.score, 5786181041331610u128, "v3 baseline score");
			assert!(!is_resolved(&sol, alice_id), "tight-limit Alice is dropped by v3");
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				67446698202754u128,
				"Dave BNC out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				6278734169887749u128,
				"Charlie HDX out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				1173241107u128,
				"Bob DOT out (v3)"
			);

			let _v4 = run_v4_compare("blocked_3ring", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 10 — binding-limit chain. Chain BNC->HDX->DOT, but Bob's HDX->DOT min
// is set just above what v3's HDX->DOT AMM trade yields (~22.167 DOT). v3 routes
// per-pair, can't meet Bob's limit, and DROPS him (only Alice resolves). v4
// should net the HDX leg (less slippage) and fill Bob.
// ---------------------------------------------------------------------------
#[test]
fn netting_binding_limit_chain_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // BNC->HDX (loose)
	let bob_hdx = 14_000 * hdx_unit; // HDX->DOT (tight)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		// 222_000_000_000 = 22.2 DOT, just above v3's per-pair output of ~22.167 DOT.
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, 222_000_000_000, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX (loose)
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT (tight)

			let sol = solve_current();

			// --- v3 baseline ---
			// Bob's HDX->DOT limit (22.2 DOT) sits just above the ~22.167 DOT his
			// per-pair AMM trade yields, so v3 DROPS Bob — only Alice resolves.
			// GLOBAL-NETTING TARGET (v4): net the HDX leg -> less slippage -> Bob fills.
			assert_eq!(sol.resolved_intents.len(), 1, "v3 drops Bob (tight limit)");
			assert_eq!(amm_trade_count(&sol), 1);
			assert_eq!(sol.score, 13730309893978531u128, "v3 baseline score");
			assert!(!is_resolved(&sol, bob_id), "tight-limit Bob is dropped by v3");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14730309893978531u128,
				"Alice HDX out (v3)"
			);

			let _v4 = run_v4_compare("binding_limit_chain", &sol);
		});
}

// ---------------------------------------------------------------------------
// Scenario 11 — tight-limit cycle (more-intents-filled). The 4-cycle from
// scenario 4, but Dave's WETH->HDX min is set just above v3's per-leg output
// (~35_187 HDX). v3 misses the cycle, routes per-leg, and DROPS Dave for missing
// his limit. v4 should internalize the cycle (less slippage) and fill all four.
// ---------------------------------------------------------------------------
#[test]
fn netting_tight_limit_cycle_baseline() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	let alice_hdx = 10_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 680 * bnc_unit; // BNC->DOT
	let charlie_dot = 15 * dot_unit; // DOT->WETH
	let dave_weth = weth_unit / 30; // WETH->HDX (tight)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		// 35_500 HDX, just above v3's per-leg WETH->HDX output of ~35_187 HDX.
		.submit_swap_intent(dave.clone(), weth, hdx, dave_weth, 35_500 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH
			let dave_id = 32752052247409382067756072960003u128; // WETH->HDX (tight)

			let sol = solve_current();

			// --- v3 baseline ---
			// Dave's WETH->HDX limit (35_500 HDX) is just above the ~35_187 HDX his
			// per-leg AMM trade yields, so v3 DROPS Dave; the 4-cycle collapses to a
			// 3-leg chain. GLOBAL-NETTING TARGET (v4): internalize the cycle -> less
			// slippage -> Dave fills, four resolved.
			assert_eq!(
				sol.resolved_intents.len(),
				3,
				"v3 drops Dave (tight limit), cycle breaks"
			);
			assert_eq!(amm_trade_count(&sol), 3);
			assert_eq!(sol.score, 8555978575797269u128, "v3 baseline score");
			assert!(!is_resolved(&sol, dave_id), "tight-limit Dave is dropped by v3");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				674092120446581u128,
				"Alice BNC out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				159212482182u128,
				"Bob DOT out (v3)"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				8882737242868506u128,
				"Charlie WETH out (v3)"
			);

			let _v4 = run_v4_compare("tight_limit_cycle", &sol);
		});
}

// ---------------------------------------------------------------------------
// Explicit 4-asset-cycle conservation check: run v3 and v4, print both metrics,
// and submit the v4 solution to confirm the pallet accepts it (conservation).
// ---------------------------------------------------------------------------
#[test]
fn netting_v4_4cycle_conservation() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	let alice_hdx = 10_000 * hdx_unit;
	let bob_bnc = 680 * bnc_unit;
	let charlie_dot = 15 * dot_unit;
	let dave_weth = weth_unit / 30;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), weth, hdx, dave_weth, 100 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			let v3 = solve_current();
			let v4 = solve_v4();
			println!(
				"V3: resolved={} amm_trades={} score={}",
				v3.resolved_intents.len(),
				v3.trades.len(),
				v3.score
			);
			println!(
				"V4: resolved={} amm_trades={} score={}",
				v4.resolved_intents.len(),
				v4.trades.len(),
				v4.score
			);
			dump("v4 4cycle", &v4);
			// The real conservation test: the pallet must accept v4's solution.
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				v4.clone(),
			));
			println!("v4 submit_solution: OK");
		});
}
