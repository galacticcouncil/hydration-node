use crate::ice::generator::generate_random_intents;
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
use orml_traits::MultiCurrency;
use pallet_ice::traits::OmnipoolInfo;
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap, SwapType};
//use pallet_ice::Call::submit_intent;
use frame_support::dispatch::GetDispatchInfo;
use pallet_omnipool::types::Tradability;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{BlockNumberProvider, Dispatchable};

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type OmniSolverWithOmnipool =
	ice_solver::omni::OmniSolver<AccountId, AssetId, OmnipoolDataProvider<hydradx_runtime::Runtime>>;

#[test]
fn solver_should_find_solution_with_one_intent() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);

		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents = submit_intents(vec![Intent {
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
		let intents = submit_intents(vec![Intent {
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
		let intents = submit_intents(vec![
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
fn execute_solution_should_work_with_multiple_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents = generate_random_intents(
			10_000,
			OmnipoolDataProvider::<hydradx_runtime::Runtime>::assets(None),
			deadline,
		);
		dbg!(intents.len());
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128,
			));
		}
		let intents = submit_intents(intents);
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();
		dbg!(&resolved.len());

		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();

		let c = hydradx_runtime::RuntimeCall::ICE(pallet_ice::Call::<hydradx_runtime::Runtime>::submit_solution {
			intents: resolved,
			trades: BoundedTrades::try_from(trades).unwrap(),
			score,
			block: System::current_block_number(),
		});
		let info = c.get_dispatch_info();
		dbg!(info);

		//assert_ok!(c.dispatch());
	});
}

#[test]
fn solve_should_not_return_solution_when_intent_at_exact_spot_price() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents: Vec<Intent<AccountId, AssetId>> = vec![Intent {
			who: ALICE.into(),
			swap: Swap {
				asset_in: 16,
				asset_out: 28,
				amount_in: 1001497604662274886037302,
				amount_out: 1081639587746551400027,
				swap_type: SwapType::ExactIn,
			},
			deadline: 43200000,
			partial: true,
			on_success: None,
			on_failure: None,
		}];
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128,
			));
		}
		let intents = submit_intents(intents);
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();
		assert_eq!(resolved.len(), 0);
	});
}

#[test]
fn execute_solution_should_work_when_transfer_are_below_existential_deposit() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents: Vec<Intent<AccountId, AssetId>> = vec![
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 5,
					asset_out: 8,
					amount_in: 4821630410495467,
					amount_out: 4300252617313999658,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: BOB.into(),
				swap: Swap {
					asset_in: 100,
					asset_out: 14,
					amount_in: 81565235644454869738270,
					amount_out: 380462588393307031,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: CHARLIE.into(),
				swap: Swap {
					asset_in: 31,
					asset_out: 5,
					amount_in: 2503466695997857626345467,
					amount_out: 9807415192088,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		];
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128,
			));
		}
		let intents = submit_intents(intents);
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();
		//dbg!(&resolved);

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
fn execute_solution_should_work_with_three_not_matched_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents: Vec<Intent<AccountId, AssetId>> = vec![
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 33,
					asset_out: 27,
					amount_in: 132653831770276356107540,
					amount_out: 20603223703468376,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: BOB.into(),
				swap: Swap {
					asset_in: 9,
					asset_out: 31,
					amount_in: 3717269212068780311876590,
					amount_out: 23878885199132385026397556,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: CHARLIE.into(),
				swap: Swap {
					asset_in: 8,
					asset_out: 12,
					amount_in: 377054246311395353,
					amount_out: 24475091286281977,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		];
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128,
			));
		}
		let intents = submit_intents(intents);
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();
		//dbg!(&resolved);

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
fn execute_solution_should_work_with_three_matched_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents: Vec<Intent<AccountId, AssetId>> = vec![
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 13,
					asset_out: 100,
					amount_in: 326409469329847147541743,
					amount_out: 91211533416359082413821,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: BOB.into(),
				swap: Swap {
					asset_in: 16,
					asset_out: 100,
					amount_in: 859672196380158283722402,
					amount_out: 116544522744433552955005,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: CHARLIE.into(),
				swap: Swap {
					asset_in: 100,
					asset_out: 16,
					amount_in: 118196975642964996908053,
					amount_out: 601990851295077318054073,
					swap_type: SwapType::ExactIn,
				},
				deadline: 43200000,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		];
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128,
			));
		}
		let intents2 = submit_intents(intents);

		let intent_balances = intents2
			.clone()
			.into_iter()
			.map(|(intent_id, intent)| {
				(
					intent_id,
					(intent.swap.amount_in, 0u128),
					(intent.swap.asset_in.clone(), intent.swap.asset_out.clone()),
					intent.who.clone(),
				)
			})
			.collect::<Vec<_>>();
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents2).unwrap();
		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved.clone(),
			BoundedTrades::try_from(trades).unwrap(),
			score,
			System::current_block_number()
		));

		// Check that the balances are correct
		for resolved_intent in resolved.iter() {
			let (initial_balance_in, initial_balance_out, asset_in, asset_out, who) = intent_balances
				.iter()
				.find(|(intent_id, a, b, c)| *intent_id == resolved_intent.intent_id)
				.map(|(_, (a, b), (x, y), c)| (*a, *b, *x, *y, c.clone()))
				.unwrap();
			assert_eq!(
				Currencies::total_balance(asset_in, &who),
				initial_balance_in - resolved_intent.amount_in
			);
			assert_eq!(
				Currencies::total_balance(asset_out, &who),
				initial_balance_out + resolved_intent.amount_out
			);
		}
	});
}

#[test]
fn haha() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		/*
		let deadline: Moment = Timestamp::now() + 43_200_000;
		let intents = generate_random_intents(
			129,
			OmnipoolDataProvider::<hydradx_runtime::Runtime>::assets(None),
			deadline,
		);
		//dbg!(&intents);
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128,
			));
		}
		let intents = submit_intents(intents);
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();
		dbg!(&resolved.len());

		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();

		let c = hydradx_runtime::RuntimeCall::ICE(pallet_ice::Call::<hydradx_runtime::Runtime>::submit_solution{
			intents: resolved,
			trades: BoundedTrades::try_from(trades).unwrap(),
			score,
			block: System::current_block_number()
		});
		let info = c.get_dispatch_info();
		dbg!(info);

		//assert_ok!(c.dispatch());S

		 */
		let c = hydradx_runtime::RuntimeCall::Router(pallet_route_executor::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: 0,
			asset_out: 20,
			amount_in: 100_000_000_000_000,
			min_amount_out: 0,
			route: vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: 0,
				asset_out: 20,
			}],
		});
		let info = c.get_dispatch_info();
		dbg!(info);
		let c = hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: 0,
			asset_out: 20,
			amount: 100_000_000_000_000,
			min_buy_amount: 0,
		});
		let info = c.get_dispatch_info();
		dbg!(info);
	});
}
