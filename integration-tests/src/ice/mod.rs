mod intents;
mod solution;
mod cvx;

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

pub(crate) fn submit_intents(intents: Vec<(AccountId, Swap<AssetId>, Moment)>) -> Vec<IntentId> {
	let mut intent_ids = Vec::new();
	for (who, swap, deadline) in intents {
		let increment_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::next_incremental_id();
		assert_ok!(ICE::submit_intent(
			RuntimeOrigin::signed(who.clone()),
			Intent {
				who,
				swap,
				deadline,
				partial: false,
				on_success: None,
				on_failure: None,
			}
		));
		let intent_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent_id(deadline, increment_id);
		intent_ids.push(intent_id);
	}

	intent_ids
}

pub(crate) fn solve_intents(
	intents: Vec<(IntentId, pallet_ice::types::Intent<AccountId, AssetId>)>,
) -> Result<(BoundedResolvedIntents, BoundedTrades<AssetId>, u64), ()> {
	let solved = ice_solver::SimpleSolver::<hydradx_runtime::Runtime, Router, Router, PriceP>::solve(intents)?;
	let resolved_intents = BoundedResolvedIntents::try_from(solved.intents).unwrap();
	let trades = BoundedTrades::try_from(solved.trades).unwrap();
	Ok((resolved_intents, trades, solved.score))
}
