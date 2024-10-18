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

pub(crate) fn solve_intents_with<S: ICESolver<(IntentId, Intent<sp_runtime::AccountId32, AssetId>)>>(
	intents: Vec<(IntentId, Intent<sp_runtime::AccountId32, AssetId>)>,
) -> Result<(BoundedResolvedIntents, BoundedTrades<AssetId>, u64), ()>
where
	S::Solution: IceSolution<AssetId>,
{
	let solution = S::solve(intents).map_err(|_| ())?;
	let score = solution.score();
	let resolved_intents = BoundedResolvedIntents::try_from(solution.resolved_intents()).unwrap();
	let trades = BoundedTrades::try_from(solution.trades()).unwrap();
	Ok((resolved_intents, trades, score))
}

use hydradx_traits::registry::Inspect;
struct OmnipoolDataProvider;

impl OmnipoolInfo<AssetId> for OmnipoolDataProvider {
	fn assets(filter: Option<Vec<AssetId>>) -> Vec<OmnipoolAssetInfo<AssetId>> {
		if let Some(filter_assets) = filter {
			let mut assets = vec![];

			for asset_id in filter_assets {
				let state = Omnipool::load_asset_state(asset_id).unwrap();
				let decimals = hydradx_runtime::AssetRegistry::decimals(asset_id).unwrap();
				let details = hydradx_runtime::AssetRegistry::assets(asset_id).unwrap();
				let symbol = details.symbol.unwrap();
				let s = String::from_utf8(symbol.to_vec()).unwrap();
				let fees = hydradx_runtime::DynamicFees::current_fees(asset_id).unwrap();
				assets.push(OmnipoolAssetInfo {
					asset_id,
					reserve: state.reserve,
					hub_reserve: state.hub_reserve,
					decimals,
					fee: fees.asset_fee,
					hub_fee: fees.protocol_fee,
					symbol: s,
				});
			}
			assets
		} else {
			let mut assets = vec![];
			for (asset_id, state) in Omnipool::omnipool_state() {
				let decimals = hydradx_runtime::AssetRegistry::decimals(asset_id).unwrap();
				let details = hydradx_runtime::AssetRegistry::assets(asset_id).unwrap();
				let symbol = details.symbol.unwrap();
				let s = String::from_utf8(symbol.to_vec()).unwrap();
				let fees = hydradx_runtime::DynamicFees::current_fees(asset_id).unwrap();
				assets.push(OmnipoolAssetInfo {
					asset_id,
					reserve: state.reserve,
					hub_reserve: state.hub_reserve,
					decimals,
					fee: fees.asset_fee,
					hub_fee: fees.protocol_fee,
					symbol: s,
				});
			}
			assets
		}
	}
}

fn print_json_str(assets: &[OmnipoolAssetInfo<AssetId>]) {
	// dump assets info in json format
	let mut json = String::from("[");
	for asset in assets {
		json.push_str(&format!(
			r#"{{"asset_id": {}, "reserve": {}, "hub_reserve": {}, "decimals": {}, "fee": {}, "hub_fee": {}, "symbol": "{}"}}"#,
			asset.asset_id,
			asset.reserve,
			asset.hub_reserve,
			asset.decimals,
			asset.fee.deconstruct(),
			asset.hub_fee.deconstruct(),
			asset.symbol
		));
		json.push_str(",");
	}
	json.pop();
	json.push_str("]");
	println!("{}", json);
}

/*
fn print_python(assets: &[OmnipoolAssetInfo<AssetId>]) {
	// print info in this format
	// liquidity = {'HDX': mpf(100000000), 'USDT': mpf(10000000), 'DOT': mpf(10000000 / 7.5)}
	// lrna = {'HDX': mpf(1000000), 'USDT': mpf(10000000), 'DOT': mpf(10000000)}
	let convert_to_f64 = |a: Balance, dec: u8| -> f64 {
		let factor = 10u128.pow(dec as u32);
		// FixedU128::from_rational(a, factor).to_float() -> this gives slightly different results but it should be more precise?!!
		a as f64 / factor as f64
	};
	let mut liquidity = String::from("liquidity = {");
	let mut lrna = String::from("lrna = {");
	for asset in assets {
		liquidity.push_str(&format!("'{}': mpf({}), ", asset.symbol, convert_to_f64(asset.reserve, asset.decimals)));
		lrna.push_str(&format!("'{}': mpf({}), ", asset.symbol, convert_to_f64(asset.hub_reserve, 12u8)));
	}
	liquidity.push_str("}");
	lrna.push_str("}");
	println!("{}", liquidity);
	println!("{}", lrna);
}
*/

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

const PATH_TO_SNAPSHOT: &str = "omnipool-snapshot/2024-10-18";

#[test]
fn test_omnipool_snapshot() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//let omnipool_account = hydradx_runtime::Omnipool::protocol_account();
		let buy_asset = 27;
		let initial_balance20 = Currencies::free_balance(buy_asset, &AccountId32::from(BOB));
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000_000);

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(BOB.into()),
			0,
			buy_asset,
			100_000_000_000_000_000,
			0,
			vec![]
		));
		let balance20 = Currencies::free_balance(buy_asset, &AccountId32::from(BOB));

		assert_eq!(balance20 - initial_balance20, 1249711278057);
	});
}

#[test]
fn solver_should_find_solution_with_matching_intents() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_runtime::Balances::set_balance(&BOB.into(), 200_000_000_000_000);

		let d = OmnipoolDataProvider::assets(None);
		print_json_str(&d);

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
