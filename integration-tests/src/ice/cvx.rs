use crate::ice::{solve_intents_with, OmnipoolDataProvider, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, ICE, LRNA as LRNAT,
};
use ice_solver::traits::{ICESolver, IceSolution, OmnipoolAssetInfo, OmnipoolInfo};
use orml_traits::MultiCurrency;
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use sp_runtime::Permill;
use xcm_emulator::TestExt;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type CvxSolverWithMockOmnipool =
	ice_solver::cvx::CVXSolver<hydradx_runtime::Runtime, Router, Router, PriceP, MockOmniInfo>;
type CvxSolverWithOmnipool =
	ice_solver::cvx::CVXSolver<hydradx_runtime::Runtime, Router, Router, PriceP, OmnipoolDataProvider>;
type CvxSolver2WithOmnipool =
	ice_solver::cvx2::CVXSolver2<hydradx_runtime::Runtime, Router, Router, PriceP, OmnipoolDataProvider>;

// the following test has been used to compare results between python and rust implementation
struct MockOmniInfo;

impl OmnipoolInfo<AssetId> for MockOmniInfo {
	fn assets(filter: Option<Vec<AssetId>>) -> Vec<OmnipoolAssetInfo<AssetId>> {
		vec![
			OmnipoolAssetInfo {
				asset_id: 0,
				reserve: 100000000_000_000_000_000,
				hub_reserve: 1000000_000_000_000_000,
				decimals: 12,
				fee: Permill::from_float(0.0025),
				hub_fee: Permill::from_float(0.0005),
				symbol: String::from_utf8(b"HDX".to_vec()).unwrap(),
			},
			OmnipoolAssetInfo {
				asset_id: 2,
				reserve: 1333333_3333333333,
				hub_reserve: 10000000_000_000_000_000,
				decimals: 10,
				fee: Permill::from_float(0.0025),
				hub_fee: Permill::from_float(0.0005),
				symbol: String::from_utf8(b"HDX".to_vec()).unwrap(),
			},
			OmnipoolAssetInfo {
				asset_id: 20,
				reserve: 10000000_000_000_000_000,
				hub_reserve: 10000000_000_000_000_000,
				decimals: 12,
				fee: Permill::from_float(0.0025),
				hub_fee: Permill::from_float(0.0005),
				symbol: String::from_utf8(b"HDX".to_vec()).unwrap(),
			},
		]
	}
}

#[test]
fn test_specific_mock_scenario() {
	let deadline: Moment = NOW + 43_200_000;
	let intent1 = (
		1u128,
		Intent {
			who: BOB.into(),
			swap: Swap {
				asset_in: 2,
				asset_out: 20,
				amount_in: 100_000_000_000_0,
				amount_out: 700_000_000_000_000,
				swap_type: pallet_ice::types::SwapType::ExactIn,
			},
			deadline,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	);
	let intent2 = (
		2u128,
		Intent {
			who: BOB.into(),
			swap: Swap {
				asset_in: 20,
				asset_out: 0,
				amount_in: 1500_000_000_000_000,
				amount_out: 100_000_000_000_000_000,
				swap_type: pallet_ice::types::SwapType::ExactIn,
			},
			deadline,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	);
	let intent3 = (
		3u128,
		Intent {
			who: BOB.into(),
			swap: Swap {
				asset_in: 20,
				asset_out: 2,
				amount_in: 400_000_000_000_000,
				amount_out: 50_000_000_000_0,
				swap_type: pallet_ice::types::SwapType::ExactIn,
			},
			deadline,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	);
	let intent4 = (
		4u128,
		Intent {
			who: BOB.into(),
			swap: Swap {
				asset_in: 0,
				asset_out: 20,
				amount_in: 100_000_000_000_000,
				amount_out: 100_000_000_000_000,
				swap_type: pallet_ice::types::SwapType::ExactIn,
			},
			deadline,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	);

	let intents = vec![intent1, intent2, intent3, intent4];
	let (resolved, trades, score) = solve_intents_with::<CvxSolverWithMockOmnipool>(intents).unwrap();

	assert_eq!(
		resolved.to_vec(),
		vec![
			pallet_ice::types::ResolvedIntent {
				intent_id: 1,
				amount_in: 100_000_000_000_0,
				amount_out: 700_000_000_000_000,
			},
			pallet_ice::types::ResolvedIntent {
				intent_id: 2,
				amount_in: 1500_000_000_000_000,
				amount_out: 100_000_000_000_000_000,
			},
			pallet_ice::types::ResolvedIntent {
				intent_id: 3,
				amount_in: 400_000_000_000_000,
				amount_out: 50_000_000_000_0,
			},
		]
	);
}

#[test]
fn solver_should_find_solution_with_matching_intents() {
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
					amount_out: 1_149_000_000_000,
					swap_type: pallet_ice::types::SwapType::ExactIn,
				},
				deadline,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		);
		let intent2 = (
			2u128,
			Intent {
				who: BOB.into(),
				swap: Swap {
					asset_in: 27,
					asset_out: 0,
					amount_out: 100_000_000_000_000,
					amount_in: 1_150_000_000_000,
					swap_type: pallet_ice::types::SwapType::ExactIn,
				},
				deadline,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		);

		let intents = vec![intent1, intent2];
		let (resolved, trades, score) = solve_intents_with::<CvxSolver2WithOmnipool>(intents).unwrap();

		assert_eq!(
			resolved.to_vec(),
			vec![pallet_ice::types::ResolvedIntent {
				intent_id: 1,
				amount_in: 100_000_000_000_000_000,
				amount_out: 1149711278057,
			},]
		);
	});
}

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
					amount_in: 100_000_000_000_000_000,
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
		let (resolved, trades, score) = solve_intents_with::<CvxSolverWithOmnipool>(intents).unwrap();

		assert_eq!(
			resolved.to_vec(),
			vec![pallet_ice::types::ResolvedIntent {
				intent_id: 1,
				amount_in: 100_000_000_000_000,
				amount_out: 1149711278057,
			},]
		);
	});
}
//TODO: add such test
// Alice wants to buy 100 DOT for $800 total
// Bob wants to sell 100 DOT for $700 total
// Charlie wants to buy $100 with 0.0001 DOT
// it should not resolve charlie's intent
