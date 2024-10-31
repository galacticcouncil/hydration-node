use crate::ice::{solve_intents_with, submit_intents, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydradx_adapters::ice::{IceRoutingSupport, OmnipoolDataProvider};
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, System, Timestamp, ICE,
	LRNA as LRNAT,
};
use hydradx_traits::router::{PoolType, Trade};
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use pallet_ice::Call::submit_intent;
use pallet_omnipool::types::Tradability;
use primitives::{AccountId, AssetId, Moment};
use sp_runtime::traits::BlockNumberProvider;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type OmniSolverWithOmnipool =
	ice_solver::omni::OmniSolver<AccountId, AssetId, OmnipoolDataProvider<hydradx_runtime::Runtime>>;

#[test]
fn solver_should_find_solution_with_one_intent() {
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
			partial: true,
			on_success: None,
			on_failure: None,
		}]);

		let intents = vec![(
			intent_ids[0],
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
		)];

		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();

		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();

		assert_eq!(
			resolved.to_vec(),
			vec![pallet_ice::types::ResolvedIntent {
				intent_id: 796899343984252629811200000,
				amount_in: 99853645824127,
				amount_out: 1148028627591,
			},]
		);

		assert_eq!(
			trades,
			vec![pallet_ice::types::TradeInstruction::SwapExactIn {
				asset_in: 0,
				asset_out: 27,
				amount_in: 99853645824127,
				amount_out: 1148028627591,
				route: pallet_ice::types::BoundedRoute::try_from(vec![Trade {
					pool: PoolType::Omnipool,
					asset_in: 0,
					asset_out: 27,
				}])
				.unwrap()
			},]
		);

		assert_eq!(score, 1000000);
	});
}

#[test]
fn execute_solution_should_work_with_one_intent() {
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
			partial: true,
			on_success: None,
			on_failure: None,
		}]);

		let intents = vec![(
			intent_ids[0],
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
		)];
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();

		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved,
			BoundedTrades::try_from(trades).unwrap(),
			score,
			System::current_block_number()
		));
	});
}

#[test]
fn execute_solution_should_work_with_two_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);
		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			27,
			2_149_711_278_057,
		));

		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intent_ids = submit_intents(vec![
			Intent {
				who: BOB.into(),
				swap: Swap {
					asset_in: 0,
					asset_out: 27,
					amount_in: 100_000_000_000_000,
					amount_out: 1_149_711_278_057,
					swap_type: pallet_ice::types::SwapType::ExactIn,
				},
				deadline,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 27,
					asset_out: 0,
					amount_in: 1_149_711_278_057,
					amount_out: 100_000_000_000_000,
					swap_type: pallet_ice::types::SwapType::ExactIn,
				},
				deadline,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		]);

		let intents = vec![
			(
				intent_ids[0],
				pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
			),
			(
				intent_ids[1],
				pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[1]).unwrap(),
			),
		];
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();

		dbg!(&resolved);

		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved,
			BoundedTrades::try_from(trades).unwrap(),
			score,
			System::current_block_number()
		));
	});
}
