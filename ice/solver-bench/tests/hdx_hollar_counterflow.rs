//! Regression fixture: a fillable all-or-nothing HDX -> HOLLAR intent dropped by the v4 pair
//! crossing because unfillable HOLLAR -> HDX partials were still in the backward group while a
//! skewed reference price valued the forward side.
//!
//! The fixture is the raw `IceSolverApi_solver_input` result at lark0 block 33540 (spec 447).

use amm_simulator::HydrationSimulator;
use codec::Decode;
use hydradx_runtime::{HydrationSimulators, SimulatorPriceDenom, SmartRouteFinder};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::{IceSolver, SolverOptions};
use ice_support::{AssetId, Balance, IntentId, Solution};
use pallet_ice_runtime_api::SolverInput;
use std::cell::RefCell;
use std::collections::BTreeMap;

const INTENT: IntentId = 33052858506865904561474764803469;
const ABOVE_MARKET_COUNTERFLOW: [IntentId; 2] = [33039371538871293297001758721169, 33034441830985035156427898880401];

thread_local! {
	static ED: RefCell<BTreeMap<AssetId, Balance>> = const { RefCell::new(BTreeMap::new()) };
}

struct FixtureConfig;
impl SimulatorConfig for FixtureConfig {
	type Simulators = HydrationSimulators;
	type RouteDiscovery = SmartRouteFinder<HydrationSimulators>;
	type PriceDenominator = SimulatorPriceDenom;

	fn existential_deposit(asset_id: AssetId) -> Balance {
		ED.with(|m| m.borrow().get(&asset_id).copied().unwrap_or(0))
	}
}

type Solver = ice_solver::v4::Solver<HydrationSimulator<FixtureConfig>>;

fn load() -> SolverInput {
	let bytes = hex::decode(include_str!("fixtures/lark0_33540_hdx_hollar_counterflow.hex").trim()).unwrap();
	Option::<SolverInput>::decode(&mut &bytes[..]).unwrap().unwrap()
}

fn solve(input: SolverInput) -> Solution {
	ED.with(|m| *m.borrow_mut() = input.existential_deposits.iter().copied().collect());
	let state = <HydrationSimulators as SimulatorSet>::State::decode(&mut &input.state[..]).unwrap();
	let min_outs = input.min_amount_out.iter().copied().collect();
	Solver::solve_with_options(input.intents, min_outs, state, input.fee, &SolverOptions::default()).unwrap()
}

fn resolved(solution: &Solution, id: IntentId) -> Option<(Balance, Balance)> {
	solution
		.resolved_intents
		.iter()
		.find(|r| r.id == id)
		.map(|r| (r.data.amount_in(), r.data.amount_out()))
}

#[test]
fn solve_should_resolve_intent_when_above_market_counterflow_is_absent() {
	let mut input = load();
	input.intents.retain(|i| !ABOVE_MARKET_COUNTERFLOW.contains(&i.id));

	let solution = solve(input);

	assert_eq!(
		resolved(&solution, INTENT),
		Some((26_918_538_868_371_770, 205_858_765_823_271_870_236))
	);
}

#[test]
fn solve_should_resolve_intent_when_above_market_counterflow_is_present() {
	let solution = solve(load());

	assert_eq!(
		resolved(&solution, INTENT),
		Some((26_918_538_868_371_770, 205_858_765_823_271_870_236))
	);
}
