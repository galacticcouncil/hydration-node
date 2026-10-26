//! Mainnet block 15_261_304, the state the 15_261_305 solution was built on: a
//! partial aUSDC→PAXG limit (#…432572) beside a PAXG→DOT DCA (#…888514). The
//! limit sat inside spot but outside spot-minus-pool-fee, so pair-by-pair
//! crossing dropped it and it filled one block later against the DCA's price
//! impact instead of against the DCA's PAXG.
//!
//! The snapshot is a gitignored scrape, so these tests are `#[ignore]`d:
//!
//! ```sh
//! ./target/release/scraper save-storage --uri wss://rpc.kril.hydration.cloud \
//!     --at 0xed70c6b628af8bd48666fe102788d6c6bf1a49b104df17f559fabf3a38f4ffa6 \
//!     --path integration-tests/snapshots/ice/xpair
//! cargo test -p runtime-integration-tests --locked ice::cross_pair -- --ignored --nocapture
//! ```

use crate::polkadot_test_net::hydradx_run_to_next_block;
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::pallet_prelude::{TransactionSource, ValidateUnsigned};
use hydradx_runtime::{Runtime, RuntimeOrigin};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::v4::Solver as IceSolver;
use ice_support::{Balance, Intent, IntentId, Solution};
use std::collections::BTreeMap;

const SNAPSHOT: &str = "snapshots/ice/xpair/SNAPSHOT";

const LIMIT: IntentId = 33035558264829864205910802432572;
const DCA: IntentId = 33035246810002923693841317888514;

type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;

fn solve_excluding(exclude: &[IntentId]) -> Solution {
	let (intents, _state, _eds, min_outs, fee, _mode) =
		pallet_ice::Pallet::<Runtime>::solver_input().expect("snapshot should yield solver input");
	let ids: Vec<IntentId> = intents.iter().map(|i| i.id).collect();
	assert!(
		ids.contains(&LIMIT) && ids.contains(&DCA),
		"both intents must be solver input: {ids:?}"
	);
	let intents: Vec<Intent> = intents.into_iter().filter(|i| !exclude.contains(&i.id)).collect();
	let min_outs: BTreeMap<IntentId, Balance> = min_outs.into_iter().collect();
	let state =
		<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::initial_state();
	let solution = Solver::solve_with_limits(intents, min_outs, state, fee).expect("solver should produce a solution");
	for ri in solution.resolved_intents.iter() {
		println!(
			"#{}: {} of {} -> {} of {}",
			ri.id,
			ri.data.amount_in(),
			ri.data.asset_in(),
			ri.data.amount_out(),
			ri.data.asset_out()
		);
	}
	for t in solution.trades.iter() {
		println!("trade in {} out {} route {:?}", t.amount_in, t.amount_out, t.route);
	}
	solution
}

fn resolved(solution: &Solution) -> Vec<IntentId> {
	solution.resolved_intents.iter().map(|r| r.id).collect()
}

#[test]
#[ignore = "needs the gitignored snapshots/ice/xpair scrape"]
fn limit_should_stay_unfilled_when_the_dca_is_absent() {
	crate::driver::HydrationTestDriver::with_snapshot(SNAPSHOT).execute(|| {
		assert!(!resolved(&solve_excluding(&[DCA])).contains(&LIMIT));
	});
}

#[test]
#[ignore = "needs the gitignored snapshots/ice/xpair scrape"]
fn limit_should_be_matched_against_the_dca_when_both_are_in_the_batch() {
	crate::driver::HydrationTestDriver::with_snapshot(SNAPSHOT).execute(|| {
		let solution = solve_excluding(&[]);
		let ids = resolved(&solution);
		assert!(ids.contains(&LIMIT) && ids.contains(&DCA), "resolved: {ids:?}");

		let call = pallet_ice::Call::<Runtime>::submit_solution {
			solution: solution.clone(),
		};
		assert_ok!(pallet_ice::Pallet::<Runtime>::validate_unsigned(
			TransactionSource::Local,
			&call
		));
		hydradx_run_to_next_block();
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			solution
		));
	});
}
