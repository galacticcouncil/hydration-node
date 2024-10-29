mod intents;
mod omni;

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydra_dx_math::ratio::Ratio;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, ICE, LRNA as LRNAT,
};
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::AssetPair;
use orml_traits::MultiCurrency;
use pallet_ice::traits::Solver;
use pallet_ice::types::{Balance, BoundedResolvedIntents, BoundedTrades, Intent, IntentId, ResolvedIntent as RI, Swap};
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use xcm_emulator::TestExt;

const PATH_TO_SNAPSHOT: &str = "omnipool-snapshot/2024-10-18";

pub(crate) fn solve_intents_with<S: Solver<(IntentId, Intent<sp_runtime::AccountId32, AssetId>)>>(
	intents: Vec<(IntentId, Intent<sp_runtime::AccountId32, AssetId>)>,
) -> Result<BoundedResolvedIntents, ()> {
	let (result, metadata) = S::solve(intents).map_err(|_| ())?;
	let resolved_intents = BoundedResolvedIntents::try_from(result).unwrap();
	//let trades = BoundedTrades::try_from(solution.trades()).unwrap();
	//let score = solution.score();
	Ok(resolved_intents)
}

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
			100_000_000_000_000,
			0,
			vec![]
		));
		let balance20 = Currencies::free_balance(buy_asset, &AccountId32::from(BOB));

		assert_eq!(balance20 - initial_balance20, 1249711278057);
	});
}

pub(crate) fn submit_intents(intents: Vec<Intent<AccountId, AssetId>>) -> Vec<IntentId> {
	let mut intent_ids = Vec::new();
	for intent in intents {
		let deadline = intent.deadline;
		let increment_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::next_incremental_id();
		assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(intent.who.clone()), intent,));
		let intent_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent_id(deadline, increment_id);
		intent_ids.push(intent_id);
	}

	intent_ids
}

/*

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

 */
