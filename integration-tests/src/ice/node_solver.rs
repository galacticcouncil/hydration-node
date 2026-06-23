use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::{TestNet, ALICE};
use amm_simulator::HydrationSimulator;
use codec::{Decode, Encode};
use hydradx_runtime::Runtime;
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::v4::Solver as IceSolver;
use primitives::AccountId;
use std::cell::RefCell;
use std::collections::BTreeMap;
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

// Runtime path: ED resolved from `AssetRegistry` storage.
type RuntimeSimulator = HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>;
type RuntimeSolver = IceSolver<RuntimeSimulator>;

thread_local! {
	static ED_TL: RefCell<BTreeMap<u32, u128>> = const { RefCell::new(BTreeMap::new()) };
}

// Node path: mirrors `node/src/ice_solver_worker.rs::NodeSimulatorConfig` — reuses
// the runtime simulators/route discovery, ED served from a per-solve thread-local
// seeded from the shipped ED map.
pub struct NodeSimulatorConfig;
impl SimulatorConfig for NodeSimulatorConfig {
	type Simulators = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators;
	type RouteDiscovery = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::RouteDiscovery;
	type PriceDenominator = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::PriceDenominator;

	fn existential_deposit(asset_id: u32) -> u128 {
		ED_TL.with(|m| m.borrow().get(&asset_id).copied().unwrap_or(0))
	}
}
type NodeSimulator = HydrationSimulator<NodeSimulatorConfig>;
type NodeSolver = IceSolver<NodeSimulator>;

/// Equivalence: the node path (real `solver_input` → encode→decode → thread-local
/// ED → solve with `NodeSimulatorConfig`) must produce a byte-identical solution to
/// the runtime path (`initial_state` → solve with `HydrationSimulatorConfig`, ED
/// from storage). Any divergence would be rejected on-chain as `ScoreMismatch`.
#[test]
fn node_path_solution_should_equal_runtime_path_solution() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let asset_a = 0u32; // HDX
	let asset_b = 14u32; // BNC
	let amount_in = 2_000_000_000_000_000u128;
	let min_amount_out = 20_000_000_000_000u128;

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, amount_in * 10)
		.submit_swap_intent(alice.clone(), asset_a, asset_b, amount_in, min_amount_out, Some(10))
		.execute(|| {
			// Node-path inputs come from the real runtime-API building function.
			let (intents, encoded_state, eds, fee) =
				pallet_ice::Pallet::<Runtime>::solver_input().expect("solver_input should be Some");
			assert_eq!(intents.len(), 1, "snapshot intent should be valid");

			// Runtime path: fresh snapshot, ED from storage.
			let runtime_state =
				<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators::initial_state();
			// Shipped snapshot bytes must equal a freshly built one (codec + determinism).
			assert_eq!(
				encoded_state,
				runtime_state.encode(),
				"shipped snapshot must match a fresh initial_state"
			);
			let runtime_solution =
				RuntimeSolver::solve(intents.clone(), runtime_state, fee).expect("runtime solve should succeed");

			// Node path: decode shipped snapshot, seed thread-local ED, solve.
			let node_state: CombinedSimulatorState =
				Decode::decode(&mut &encoded_state[..]).expect("snapshot should decode");
			ED_TL.with(|m| {
				let mut m = m.borrow_mut();
				m.clear();
				m.extend(eds.iter().copied());
			});
			let node_solution = NodeSolver::solve(intents.clone(), node_state, fee).expect("node solve should succeed");

			// Byte-identical solutions.
			assert_eq!(node_solution, runtime_solution);
			assert_eq!(node_solution.encode(), runtime_solution.encode());
			assert_eq!(node_solution.resolved_intents.len(), 1, "should resolve the intent");

			// Every shipped ED must match storage for the asset the solver may query.
			for (asset, ed) in &eds {
				assert_eq!(
					*ed,
					<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::existential_deposit(*asset),
					"shipped ED must equal storage ED for asset {asset}"
				);
			}
		});
}
