mod cvx;
mod intents;
mod omni;
mod solution;

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
use ice_solver::traits::{ICESolver, IceSolution, OmnipoolAssetInfo, OmnipoolInfo, Routing};
use orml_traits::MultiCurrency;
use pallet_ice::types::{Balance, BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap};
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use xcm_emulator::TestExt;

const PATH_TO_SNAPSHOT: &str = "omnipool-snapshot/2024-10-18";

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

use hydradx_traits::registry::Inspect;
use hydradx_traits::router::{RouteProvider, RouterT, Trade};

pub(crate) struct OmnipoolDataProvider;

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

pub(crate) struct SolverRoutingSupport<R, RP, PP>(sp_std::marker::PhantomData<(R, RP, PP)>);

impl<R, RP, PP> Routing<AssetId> for SolverRoutingSupport<R, RP, PP>
where
	R: RouterT<
		RuntimeOrigin,
		AssetId,
		Balance,
		hydradx_traits::router::Trade<AssetId>,
		hydradx_traits::router::AmountInAndOut<Balance>,
	>,
	RP: RouteProvider<AssetId>,
	PP: PriceProvider<AssetId, Price = Ratio>,
{
	fn get_route(asset_a: AssetId, asset_b: AssetId) -> Vec<Trade<AssetId>> {
		RP::get_route(AssetPair::<AssetId>::new(asset_a, asset_b))
	}
	fn calculate_amount_out(route: &[Trade<AssetId>], amount_in: Balance) -> Result<Balance, ()> {
		let sold = R::calculate_sell_trade_amounts(&route, amount_in).unwrap();
		Ok(sold.last().unwrap().amount_out)
	}
	fn calculate_amount_in(route: &[Trade<AssetId>], amount_out: Balance) -> Result<Balance, ()> {
		let r = R::calculate_buy_trade_amounts(&route, amount_out).unwrap();
		Ok(r.last().unwrap().amount_in)
	}
	// should return price Hub/Asset
	fn hub_asset_price(asset_id: AssetId) -> Result<Ratio, ()> {
		PP::get_price(1u32.into(), asset_id).ok_or(())
	}
}

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
