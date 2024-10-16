use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{EmaOracle, ReferralsOraclePeriod, Router, RuntimeOrigin, ICE, LRNA as LRNAT};
use ice_solver::traits::{ICESolver, OmnipoolAssetInfo, OmnipoolInfo};
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use primitives::{AccountId, AssetId, Moment};
use sp_runtime::Permill;
use xcm_emulator::TestExt;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

struct MockOmniInfo;

impl OmnipoolInfo<AssetId> for MockOmniInfo {
	fn assets() -> Vec<OmnipoolAssetInfo<AssetId>> {
		vec![
			OmnipoolAssetInfo {
				asset_id: 0,
				reserve: 100000000_000_000_000_000,
				hub_reserve: 1000000_000_000_000_000,
				decimals: 12,
				fee: Permill::from_float(0.0025),
				hub_fee: Permill::from_float(0.0005),
			},
			OmnipoolAssetInfo {
				asset_id: 2,
				reserve: 1333333_3333333333,
				hub_reserve: 10000000_000_000_000_000,
				decimals: 10,
				fee: Permill::from_float(0.0025),
				hub_fee: Permill::from_float(0.0005),
			},
			OmnipoolAssetInfo {
				asset_id: 20,
				reserve: 10000000_000_000_000_000,
				hub_reserve: 10000000_000_000_000_000,
				decimals: 12,
				fee: Permill::from_float(0.0025),
				hub_fee: Permill::from_float(0.0005),
			},
		]
	}
}

pub(crate) fn solve_intents(
	intents: Vec<(IntentId, pallet_ice::types::Intent<AccountId, AssetId>)>,
) -> Result<(BoundedResolvedIntents, BoundedTrades<AssetId>, u64), ()> {
	let solved =
		ice_solver::cvx::CVXSolver::<hydradx_runtime::Runtime, Router, Router, PriceP, MockOmniInfo>::solve(intents)?;
	let resolved_intents = BoundedResolvedIntents::try_from(solved.intents).unwrap();
	let trades = BoundedTrades::try_from(solved.trades).unwrap();
	Ok((resolved_intents, trades, solved.score))
}

#[test]
fn test_cvx() {
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
	let solved =
		ice_solver::cvx::CVXSolver::<hydradx_runtime::Runtime, Router, Router, PriceP, MockOmniInfo>::solve(intents)
			.unwrap();

	assert_eq!(
		solved.intents,
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
