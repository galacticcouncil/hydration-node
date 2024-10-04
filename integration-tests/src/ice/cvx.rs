
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
fn test_cvx(){
   let result = solve_intents(vec![]); 
}