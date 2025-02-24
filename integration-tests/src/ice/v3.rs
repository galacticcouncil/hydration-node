use crate::ice::{solve_current_intents, submit_intents, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::__private::serde;
use frame_support::assert_ok;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Time;
use frame_support::traits::UnfilteredDispatchable;
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, System, Timestamp, ICE,
	LRNA as LRNAT,
};
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::MultiCurrency;
use pallet_ice::types::{Intent, IntentId, Swap, SwapType};
use pallet_omnipool::types::Tradability;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{BlockNumberProvider, Dispatchable};
use std::collections::BTreeSet;

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type V3Solver = hydration_solver::v3::SolverV3;

fn load_from_file() -> Vec<Intent<AccountId32>> {
	let testdata = std::fs::read_to_string("../hydration-solver/testdata/success_1732737492.json").unwrap();
	let intents: Vec<TestEntry> = serde_json::from_str(&testdata).unwrap();
	let intents: Vec<Intent<AccountId32>> = intents
		.into_iter()
		.enumerate()
		.map(|(idx, entry)| {
			let mut who: [u8; 32] = [0u8; 32];
			let b = idx.to_be_bytes();
			who[..b.len()].copy_from_slice(&b);
			let mut e: Intent<AccountId32> = entry.into();
			e.who = who.into();
			e
		})
		.collect();
	intents
}

#[test]
fn simple_v3_scenario() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		//let intents = load_from_file();
		let intents: Vec<Intent<AccountId32>> = vec![Intent {
			who: BOB.into(),
			swap: Swap {
				asset_in: 0,
				asset_out: 27,
				amount_in: 100_000_000_000_000,
				amount_out: 6775923048819,
				swap_type: SwapType::ExactIn,
			},
			deadline: Timestamp::now() + 43_200_000,
			partial: false,
			on_success: None,
			on_failure: None,
		}];
		let now = Timestamp::now();
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128 * 1_000_000,
			));
		}
		let intents = submit_intents(intents);
		let submit_call = solve_current_intents().unwrap();
		hydradx_run_to_next_block();

		assert_ok!(submit_call.dispatch_bypass_filter(RuntimeOrigin::signed(BOB.into())));

		for (intent_id, intent) in intents {
			let balance = Currencies::free_balance(intent.swap.asset_out, &intent.who);
			assert_eq!(balance, intent.swap.amount_out);
		}
	});
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
struct TestEntry {
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	partial: bool,
}

impl From<Intent<AccountId32>> for TestEntry {
	fn from(value: Intent<AccountId32>) -> Self {
		Self {
			asset_in: value.swap.asset_in,
			asset_out: value.swap.asset_out,
			amount_in: value.swap.amount_in,
			amount_out: value.swap.amount_out,
			partial: value.partial,
		}
	}
}

impl Into<Intent<AccountId32>> for TestEntry {
	fn into(self) -> Intent<AccountId32> {
		let deadline: Moment = Timestamp::now() + 43_200_000;
		Intent {
			who: ALICE.into(),
			swap: Swap {
				asset_in: self.asset_in,
				asset_out: self.asset_out,
				amount_in: self.amount_in,
				amount_out: self.amount_out,
				swap_type: SwapType::ExactIn,
			},
			deadline,
			partial: self.partial,
			on_success: None,
			on_failure: None,
		}
	}
}
