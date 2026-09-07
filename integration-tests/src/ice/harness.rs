//! Shared driving of the production solution path for the ICE suites.
//!
//! `run_and_submit_as` is the one entry point every scenario uses: it selects the
//! on-chain solver mode, builds a solution with the matching solver generation
//! through `pallet_ice::Pallet::run` (the same call the node worker makes), and
//! submits it, so the pallet's own validation is each scenario's oracle.

use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use hydradx_runtime::{Omnipool, Runtime, RuntimeOrigin, System};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::IceSolver;
use ice_support::{Solution, SolverMode};
use pallet_omnipool::types::SlipFeeConfig;
use sp_runtime::Permill;

pub(crate) type TestSimulator = HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>;
pub(crate) type V4Solver = ice_solver::v4::Solver<TestSimulator>;
pub(crate) type PassthroughSolver = ice_solver::passthrough::Solver<TestSimulator>;
pub(crate) type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

pub(crate) fn enable_slip_fees() {
	assert_ok!(Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: Permill::from_percent(5),
		})
	));
}

/// Flip the on-chain solver mode through the authority origin.
pub(crate) fn set_solver_mode(mode: SolverMode) {
	assert_ok!(pallet_ice::Pallet::<Runtime>::set_solver_mode(
		RuntimeOrigin::root(),
		mode
	));
}

/// Build a solution with solver generation `S` via the pallet's own `run`, which
/// assembles the valid-intent set, the admission floors and the live simulator
/// state exactly as production does — and rejects anything the active mode does
/// not accept. `None` when there is nothing to solve or the solution is refused.
pub(crate) fn solve_as<S: IceSolver<TestSimulator>>() -> Option<Solution> {
	let call = pallet_ice::Pallet::<Runtime>::run(
		System::block_number(),
		|intents: Vec<ice_support::Intent>,
		 limits: Vec<(ice_support::IntentId, ice_support::Balance)>,
		 state: CombinedSimulatorState| {
			S::solve_with_limits(
				intents,
				limits.into_iter().collect(),
				state,
				pallet_ice::ProtocolFee::<Runtime>::get(),
			)
			.ok()
		},
	)?;
	let pallet_ice::Call::submit_solution { solution, .. } = call else {
		panic!("expected submit_solution call");
	};
	Some(solution)
}

/// Solve with the builder that belongs to `mode` — the integration mirror of the
/// node worker's mode match.
pub(crate) fn solve_for_mode(mode: SolverMode) -> Option<Solution> {
	match mode {
		SolverMode::V4 => solve_as::<V4Solver>(),
		SolverMode::Passthrough => solve_as::<PassthroughSolver>(),
		SolverMode::Disabled => None,
	}
}

/// Select `mode`, solve with generation `S`, dump the pinnable numbers and submit.
/// Submission is the scenario's real oracle: the pallet re-checks everything the
/// active mode binds.
pub(crate) fn run_and_submit_as<S: IceSolver<TestSimulator>>(mode: SolverMode, label: &str) -> Solution {
	set_solver_mode(mode);
	let sol = solve_as::<S>().expect("solver must produce a solution");
	dump(label, &sol);
	assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
		RuntimeOrigin::none(),
		sol.clone(),
	));
	sol
}

/// `run_and_submit_as` with the builder picked from the mode.
pub(crate) fn run_and_submit_for_mode(mode: SolverMode, label: &str) -> Solution {
	match mode {
		SolverMode::V4 => run_and_submit_as::<V4Solver>(mode, label),
		SolverMode::Passthrough => run_and_submit_as::<PassthroughSolver>(mode, label),
		SolverMode::Disabled => panic!("no solution is accepted while the solver is disabled"),
	}
}

/// Number of distinct AMM trades the solution routes through the router.
/// Fewer is better: each AMM trade pays pool fee + slippage that internal
/// matching avoids.
pub(crate) fn amm_trade_count(sol: &Solution) -> usize {
	sol.trades.len()
}

/// Resolved intent by id (panics if the intent was not resolved).
pub(crate) fn resolved(sol: &Solution, id: u128) -> &ice_support::ResolvedIntent {
	sol.resolved_intents
		.iter()
		.find(|r| r.id == id)
		.expect("intent should be resolved")
}

/// Whether an intent appears in the solution's resolved set.
pub(crate) fn is_resolved(sol: &Solution, id: u128) -> bool {
	sol.resolved_intents.iter().any(|r| r.id == id)
}

/// The `SwapData` of a resolved swap intent.
pub(crate) fn swap(ri: &ice_support::ResolvedIntent) -> &ice_support::SwapData {
	match &ri.data {
		ice_support::IntentData::Swap(s) => s,
		_ => panic!("expected Swap"),
	}
}

/// `amount_in` summed over AMM trades routing `asset_in -> asset_out` (0 if none).
pub(crate) fn amm_in_for(sol: &Solution, asset_in: u32, asset_out: u32) -> u128 {
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
pub(crate) fn dump(label: &str, sol: &Solution) {
	println!("// === ICE DUMP BEGIN: {label} ===");
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
				"// resolved[{i}] id={} {}->{} in={} out={} partial={:?}",
				ri.id, s.asset_in, s.asset_out, s.amount_in, s.amount_out, s.partial
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
	println!("// === ICE DUMP END: {label} ===");
}
