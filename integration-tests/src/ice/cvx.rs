use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{EmaOracle, ReferralsOraclePeriod, Router, RuntimeOrigin, ICE, LRNA as LRNAT};
use ice_solver::traits::ICESolver;
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use primitives::{AccountId, AssetId, Moment};
use xcm_emulator::TestExt;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

pub(crate) fn solve_intents(
	intents: Vec<(IntentId, pallet_ice::types::Intent<AccountId, AssetId>)>,
) -> Result<(BoundedResolvedIntents, BoundedTrades<AssetId>, u64), ()> {
	let solved = ice_solver::cvx::CVXSolver::<hydradx_runtime::Runtime, Router, Router, PriceP>::solve(intents)?;
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
				amount_in: 100,
				amount_out: 700,
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
				amount_in: 1500,
				amount_out: 100_000,
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
				amount_in: 400,
				amount_out: 50,
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
				amount_in: 100,
				amount_out: 100,
				swap_type: pallet_ice::types::SwapType::ExactIn,
			},
			deadline,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	);
	let result = solve_intents(vec![intent1, intent2, intent3, intent4]);
}
