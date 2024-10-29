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
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use pallet_ice::Call::submit_intent;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::BlockNumberProvider;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type OmniSolverWithOmnipool = ice_solver::omni::OmniSolver<
	AccountId,
	AssetId,
	OmnipoolDataProvider<hydradx_runtime::Runtime>,
	IceRoutingSupport<Router, Router, PriceP, hydradx_runtime::RuntimeOrigin>,
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
		let resolved = solve_intents_with::<OmniSolverWithOmnipool>(intents).unwrap();

		let trades = BoundedTrades::new();
		let score = 0;

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

		assert_eq!(score, 1000000);
	});
}

#[test]
fn execute_solution_should_work_with_solved_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);
		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			LRNA,
			pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
		));

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
		let trades = BoundedTrades::new();
		let score = 0;

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved,
			trades,
			score,
			System::current_block_number()
		));

		//TODO: check balances, and check what is left in ice holding account
	});
}

#[test]
fn execute_two_intents_solution_should_work() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			27,
			111149711278057 as i128
		));

		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			LRNA,
			pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
		));

		let initial_hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		let initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));
		let alice_initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(ALICE));

		let deadline: Moment = Timestamp::now() + 43_200_000;

		let intent_ids = submit_intents(vec![
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
				partial: true,
				on_success: None,
				on_failure: None,
			},
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 27,
					asset_out: 0,
					amount_in: 1149711278057,
					amount_out: 100_000_000_000_000,
					swap_type: pallet_ice::types::SwapType::ExactIn,
				},
				deadline,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		]);

		let (intents, trades, score) = solve_intents_with::<OmniSolverWithOmnipool>(vec![
			(
				intent_ids[0],
				pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
			),
			(
				intent_ids[1],
				pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[1]).unwrap(),
			),
		])
		.unwrap();

		dbg!(&intents);

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			intents,
			trades,
			score,
			System::current_block_number()
		));

		//TODO: check balances, and check what is left in ice hodling account
		// also check the intent partial update

		let hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		assert_eq!(hdx_balance, initial_hdx_balance - 1_000_000_000_000u128);
		let dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));

		let received = dai_balance - initial_dai_balance;
		assert_eq!(received, 8_973_613_112_776_918u128);

		let alice_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(ALICE));
		let received = alice_dai_balance - alice_initial_dai_balance;
		assert_eq!(received, 8_973_613_112_776_918u128);
	});
}
