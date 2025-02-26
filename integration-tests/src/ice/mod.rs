//pub(crate) mod generator;
mod intents;
//mod omni;
mod v3;

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::UnfilteredDispatchable;
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
use pallet_ice::api::{DataRepr, IntentRepr};
use pallet_ice::types::{Balance, Intent, IntentId, ResolvedIntent as RI, Swap};
use pallet_intent::types::BoundedResolvedIntents;
use primitives::{AccountId, AssetId, Moment};
use sp_core::crypto::AccountId32;
use xcm_emulator::TestExt;

const PATH_TO_SNAPSHOT: &str = "omnipool-snapshot/2025-02-26";

fn convert_to_solver_types(
	intents: Vec<IntentRepr>,
	data: Vec<DataRepr>,
) -> (
	Vec<hydration_solver::types::Intent>,
	Vec<hydration_solver::types::Asset>,
) {
	let data: Vec<hydration_solver::types::Asset> = data
		.into_iter()
		.map(|v| {
			let (c, asset_id, reserve, hub_reserve, decimals, fee, hub_fee, pool_id) = v;
			match c {
				0 => hydration_solver::types::Asset::Omnipool(hydration_solver::types::OmnipoolAsset {
					asset_id,
					decimals,
					reserve,
					hub_reserve,
					fee,
					hub_fee,
				}),
				1 => hydration_solver::types::Asset::StableSwap(hydration_solver::types::StableSwapAsset {
					pool_id,
					asset_id,
					decimals,
					reserve,
					fee,
				}),
				_ => {
					panic!("unsupported pool asset!")
				}
			}
		})
		.collect();

	// map to solver intents
	let intents: Vec<hydration_solver::types::Intent> = intents
		.into_iter()
		.map(|v| {
			let (intent_id, asset_in, asset_out, amount_in, amount_out, partial) = v;
			hydration_solver::types::Intent {
				intent_id,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				partial,
			}
		})
		.collect();

	(intents, data)
}

pub(crate) fn solve_current_intents() -> Result<pallet_ice::Call<hydradx_runtime::Runtime>, ()> {
	let b = hydradx_runtime::System::block_number();
	let solution = pallet_ice::Pallet::<hydradx_runtime::Runtime>::run(b, |i, d| {
		let (intents, data) = convert_to_solver_types(i, d);
		let s = hydration_solver::v3::SolverV3::solve(intents, data).ok()?;
		let resolved = s
			.resolved_intents
			.iter()
			.map(|v| pallet_ice::types::ResolvedIntent {
				intent_id: v.intent_id,
				amount_in: v.amount_in,
				amount_out: v.amount_out,
			})
			.collect();
		Some(resolved)
	});

	if let Some(c) = solution {
		Ok(c)
	} else {
		Err(())
	}
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

		assert_eq!(balance20 - initial_balance20, 6775923048819);
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
