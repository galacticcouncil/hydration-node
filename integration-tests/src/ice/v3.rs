use crate::ice::generator::generate_random_intents;
use crate::ice::{solve_intents_with, submit_intents, PATH_TO_SNAPSHOT};
use crate::polkadot_test_net::*;
use frame_support::__private::serde;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydradx_adapters::ice::{IceRoutingSupport, OmnipoolDataProvider};
use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::{
	Currencies, EmaOracle, Omnipool, ReferralsOraclePeriod, Router, RuntimeOrigin, System, Timestamp, ICE,
	LRNA as LRNAT,
};
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::MultiCurrency;
use pallet_ice::traits::OmnipoolInfo;
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades, Intent, IntentId, Swap, SwapType};
use std::collections::BTreeSet;
//use pallet_ice::Call::submit_intent;
use frame_support::dispatch::GetDispatchInfo;
use pallet_omnipool::types::Tradability;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{BlockNumberProvider, Dispatchable};

type PriceP =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNAT>, ReferralsOraclePeriod>;

type V3Solver = hydration_solver::v3::SolverV3<OmnipoolDataProvider<hydradx_runtime::Runtime>>;

fn load_from_file() -> Vec<Intent<AccountId32, AssetId>> {
	let testdata = std::fs::read_to_string("../hydration-solver/testdata/success_1732737492.json").unwrap();
	let intents: Vec<TestEntry> = serde_json::from_str(&testdata).unwrap();
	let intents: Vec<Intent<AccountId32, AssetId>> = intents
		.into_iter()
		.enumerate()
		.map(|(idx, entry)| {
			let mut who: [u8; 32] = [0u8; 32];
			let b = idx.to_be_bytes();
			who[..b.len()].copy_from_slice(&b);
			let mut e: Intent<AccountId32, AssetId> = entry.into();
			e.who = who.into();
			e
		})
		.collect();
	intents
}

#[test]
fn v3_scenario() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let intents = load_from_file();
		for intent in intents.iter() {
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_in,
				intent.swap.amount_in as i128 * 1_000_000,
			));
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				intent.who.clone().into(),
				intent.swap.asset_out,
				intent.swap.amount_out as i128 * 1_000_000,
			));
		}
		let intents = submit_intents(intents);

		let resolved = solve_intents_with::<V3Solver>(intents).unwrap();

		let mut assets = BTreeSet::new();
		let mut balances = Vec::new();
		for resolved_intent in resolved.iter() {
			let intent = pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(resolved_intent.intent_id).unwrap();
			let asset_in = intent.swap.asset_in;
			let asset_out = intent.swap.asset_out;
			let asset_in_balances = Currencies::total_balance(asset_in, &intent.who);
			let asset_out_balances = Currencies::total_balance(asset_out, &intent.who);
			let resolved_amount_in = resolved_intent.amount_in;
			let resolved_amount_out = resolved_intent.amount_out;
			balances.push((
				asset_in_balances,
				asset_out_balances,
				resolved_amount_in,
				resolved_amount_out,
				intent,
			));
			assets.insert(asset_in);
			assets.insert(asset_out);
		}

		let (trades, score) =
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::calculate_trades_and_score(&resolved.to_vec()).unwrap();
		let holding_acc = hydradx_runtime::ICE::holding_account();
		for asset in assets.iter() {
			let balance = Currencies::free_balance(*asset, &holding_acc.clone().into());
			println!("{:?} balance: {:?}", asset, balance);
			//assert_eq!(balance, 0, "{:?} balance is not 0", asset);
		}

		dbg!(trades.len());
		dbg!(score);

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved,
			BoundedTrades::try_from(trades).unwrap(),
			score,
			System::current_block_number()
		));
		// verify the balances after execution
		for (asset_in_balances, asset_out_balances, resolved_amount_in, resolved_amount_out, intent) in balances.iter()
		{
			let asset_in_balances_after = Currencies::total_balance(intent.swap.asset_in, &intent.who);
			let asset_out_balances_after = Currencies::total_balance(intent.swap.asset_out, &intent.who);
			assert_eq!(asset_in_balances_after, asset_in_balances - resolved_amount_in);
			assert_eq!(asset_out_balances_after, asset_out_balances + resolved_amount_out);
		}

		// check leftover
		let holding_acc = hydradx_runtime::ICE::holding_account();
		for asset in assets.iter() {
			let balance = Currencies::free_balance(*asset, &holding_acc.clone().into());
			println!("{:?} balance: {:?}", asset, balance);
			//assert_eq!(balance, 0, "{:?} balance is not 0", asset);
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

impl From<Intent<AccountId32, AssetId>> for TestEntry {
	fn from(value: Intent<AccountId32, AssetId>) -> Self {
		Self {
			asset_in: value.swap.asset_in,
			asset_out: value.swap.asset_out,
			amount_in: value.swap.amount_in,
			amount_out: value.swap.amount_out,
			partial: value.partial,
		}
	}
}

impl Into<Intent<AccountId32, AssetId>> for TestEntry {
	fn into(self) -> Intent<AccountId32, AssetId> {
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
