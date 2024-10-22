use crate::ice::{solve_intents_with, OmnipoolDataProvider, SolverRoutingSupport, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::traits::fungible::Mutate;
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use primitives::{AccountId, AssetId, Moment};

type OmniSolverWithOmnipool =
	ice_solver::omni::OmniSolver<AccountId, AssetId, OmnipoolDataProvider, SolverRoutingSupport>;

#[test]
fn solver_should_find_solution_with_one_intent() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);

		let deadline: Moment = NOW + 43_200_000;
		let intent1 = (
			1u128,
			Intent {
				who: BOB.into(),
				swap: Swap {
					asset_in: 0,
					asset_out: 27,
					amount_in: 100_000_000_000_000,
					amount_out: 1149711278057,
					swap_type: pallet_ice::types::SwapType::ExactIn,
				},
				deadline,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		);

		let intents = vec![intent1];
		let (resolved, trades, score) = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();

		assert_eq!(
			resolved.to_vec(),
			vec![pallet_ice::types::ResolvedIntent {
				intent_id: 1,
				amount_in: 99973005585447,
				amount_out: 1149400920228,
			},]
		);
	});
}
