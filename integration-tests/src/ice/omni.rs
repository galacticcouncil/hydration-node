use crate::ice::{solve_intents_with, submit_intents, OmnipoolDataProvider, SolverRoutingSupport, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, System, Timestamp, ICE,
	LRNA as LRNAT,
};
use hydradx_traits::router::{PoolType, Trade};
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use pallet_ice::Call::submit_intent;
use primitives::{AccountId, AssetId, Moment};
use sp_runtime::traits::BlockNumberProvider;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type OmniSolverWithOmnipool = ice_solver2::omni::OmniSolver<
	AccountId,
	AssetId,
	OmnipoolDataProvider,
	SolverRoutingSupport<Router, Router, PriceP>,
>;

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

		assert_eq!(
			trades.to_vec(),
			vec![
				pallet_ice::types::TradeInstruction::SwapExactIn {
					asset_in: 0,
					asset_out: 1,
					amount_in: 99973005585447,
					amount_out: 17588098635,
					route: pallet_ice::types::BoundedRoute::try_from(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: 0,
						asset_out: 1,
					}])
					.unwrap()
				},
				pallet_ice::types::TradeInstruction::SwapExactOut {
					asset_in: 1,
					asset_out: 27,
					amount_in: 16180720855,
					amount_out: 1149400920228,
					route: pallet_ice::types::BoundedRoute::try_from(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: 1,
						asset_out: 27,
					}])
					.unwrap()
				}
			]
		);

		assert_eq!(score, 0);
	});
}

#[test]
fn execute_solution_should_work_with_solved_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);

		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intent_ids = submit_intents(vec![Intent {
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
		}]);

		let intents = vec![(
			intent_ids[0],
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
		)];
		let (resolved, trades, score) = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved,
			trades,
			score,
			System::current_block_number()
		));
	});
}
