use crate::driver::HydrationTestDriver;
use crate::ice::{solve_current_intents, submit_intents, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::__private::serde;
use frame_support::assert_ok;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Time;
use frame_support::traits::UnfilteredDispatchable;
use hydradx_adapters::ice::GlobalAmmState;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, System, Timestamp, ICE,
	LRNA as LRNAT,
};
use hydradx_traits::ice::AmmState;
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::MultiCurrency;
use pallet_ice::types::{Intent, IntentId, Swap, SwapType};
use pallet_omnipool::types::Tradability;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{BlockNumberProvider, Dispatchable};
use std::any::Any;
use std::collections::BTreeSet;

#[test]
fn simple_v3_scenario() {
	let intents: Vec<Intent<AccountId32>> = vec![Intent {
		who: BOB.into(),
		swap: Swap {
			asset_in: 0,
			asset_out: 27,
			amount_in: 100_000_000_000_000,
			amount_out: 6775923048819,
			swap_type: SwapType::ExactIn,
		},
		//deadline: Timestamp::now() + 43_200_000,
		deadline: 43_200_000,
		partial: false,
		on_success: None,
		on_failure: None,
	}];

	HydrationTestDriver::default()
		.with_snapshot(PATH_TO_SNAPSHOT)
		.submit_intents(intents) // Submit given intents, it also updates balance of the intent's user
		.inspect_data::<Vec<(IntentId, Intent<AccountId32>)>>("intents", |intents| {
			// Easy way to inspect submitted intents if needed
			dbg!(intents);
		})
		.solve_intents() // Run Solver with currently submitted intents
		.new_block()
		.execute(|| {
			let balance = Currencies::free_balance(27, &BOB.into());
			assert_eq!(balance, 6775923048819);
		});
}
