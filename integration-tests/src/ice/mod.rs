//pub(crate) mod generator;
mod intents;
//mod omni;
mod v3;

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydra_dx_math::ratio::Ratio;
use hydradx_adapters::ice::GlobalAmmState;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Intents, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, ICE, LRNA as LRNAT,
};
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::AssetPair;
use orml_traits::MultiCurrency;
use pallet_ice::types::{Balance, Intent, IntentId, ResolvedIntent as RI, Swap};
use pallet_intent::types::BoundedResolvedIntents;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use xcm_emulator::TestExt;

const PATH_TO_SNAPSHOT: &str = "omnipool-snapshot/2025-02-24";

pub(crate) fn solve_intents_with(
	intents: Vec<(IntentId, Intent<sp_runtime::AccountId32>)>,
) -> Result<BoundedResolvedIntents, ()> {
	let solution = pallet_ice::Pallet::<hydradx_runtime::Runtime>::run(0, |i, d| vec![]);

	Ok(BoundedResolvedIntents::default())
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

pub(crate) fn submit_intents(intents: Vec<Intent<AccountId>>) -> Vec<(IntentId, Intent<AccountId>)> {
	let mut intent_ids = Vec::new();
	for intent in intents {
		let deadline = intent.deadline;
		let increment_id = pallet_intent::Pallet::<hydradx_runtime::Runtime>::next_incremental_id();
		assert_ok!(Intents::submit_intent(
			RuntimeOrigin::signed(intent.who.clone()),
			intent.clone()
		));
		let intent_id = pallet_intent::Pallet::<hydradx_runtime::Runtime>::get_intent_id(deadline, increment_id);
		intent_ids.push((intent_id, intent));
	}

	intent_ids
}
