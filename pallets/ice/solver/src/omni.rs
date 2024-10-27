#![cfg_attr(not(feature = "std"), no_std)]
extern crate core;

use crate::{rational_to_f64, to_f64_by_decimals, SolverSolution};
use pallet_ice::traits::{OmnipoolAssetInfo, OmnipoolInfo, Routing, Solver};
use pallet_ice::types::{Balance, BoundedRoute, Intent, IntentId, ResolvedIntent, TradeInstruction};
use sp_runtime::{FixedU128, SaturatedConversion, Saturating};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec;
use sp_std::vec::Vec;

use clarabel::algebra::*;
use clarabel::solver::*;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;

fn calculate_scaling<AccountId, AssetId>(
	intents: &[(IntentId, Intent<AccountId, AssetId>)],
	intent_amounts: &[(f64, f64)],
	asset_ids: &[AssetId],
	omnipool_reserves: &[f64],
	omnipool_hub_reserves: &[f64],
) -> BTreeMap<AssetId, f64>
where
	AssetId: From<u32> + sp_std::hash::Hash + Copy + Clone + Eq + Ord,
{
	let mut scaling = BTreeMap::new();
	scaling.insert(1u32.into(), f64::INFINITY);

	for (idx, (_, intent)) in intents.iter().enumerate() {
		if intent.swap.asset_in != 1u32.into() {
			let a = intent.swap.asset_in;
			let sq = intent_amounts[idx].0;
			scaling.entry(a).and_modify(|v| *v = v.max(sq)).or_insert(sq);
		}
		if intent.swap.asset_out != 1u32.into() {
			let a = intent.swap.asset_out;
			let sq = intent_amounts[idx].1;
			scaling.entry(a).and_modify(|v| *v = v.max(sq)).or_insert(sq);
		}
	}

	for ((asset_id, reserve), hub_reserve) in asset_ids
		.iter()
		.zip(omnipool_reserves.iter())
		.zip(omnipool_hub_reserves.iter())
	{
		scaling
			.entry(*asset_id)
			.and_modify(|v| *v = v.min(*reserve))
			.or_insert(1.0);
		let scalar = (scaling.get(asset_id).unwrap() * *hub_reserve) / *reserve;
		scaling
			.entry(1u32.into())
			.and_modify(|v| *v = v.min(scalar))
			.or_insert(scalar);
	}

	scaling
}

fn calculate_tau_phi<AccountId, AssetId>(
	intents: &[(IntentId, Intent<AccountId, AssetId>)],
	asset_ids: &[AssetId],
	scaling: &BTreeMap<AssetId, f64>,
) -> (CscMatrix, CscMatrix)
where
	AssetId: From<u32> + sp_std::hash::Hash + Copy + Clone + Eq + Ord,
{
	let n = asset_ids.len();
	let m = intents.len();
	let mut tau = CscMatrix::zeros((n, m));
	let mut phi = CscMatrix::zeros((n, m));
	for (j, intent) in intents.iter().enumerate() {
		let sell_i = asset_ids.iter().position(|&tkn| tkn == intent.1.swap.asset_in).unwrap();
		let buy_i = asset_ids
			.iter()
			.position(|&tkn| tkn == intent.1.swap.asset_out)
			.unwrap();
		tau.set_entry((sell_i, j), 1.);
		let s = scaling.get(&intent.1.swap.asset_in).unwrap() / scaling.get(&intent.1.swap.asset_out).unwrap();
		phi.set_entry((buy_i, j), s);

		/*
		if let Some(scaling) = scaling.get(&intent.1.swap.asset_in) {
			tau.set_entry((sell_i, j), *scaling);
			phi.set_entry((buy_i, j), *scaling);
		} else {
			tau.set_entry((sell_i, j), 1.);
			phi.set_entry((buy_i, j), 1.);
		}

		 */
	}
	(tau, phi)
}

fn round_solution(intents: &[(f64, f64)], intent_deltas: Vec<f64>, tolerance: f64) -> Vec<f64> {
	let mut deltas = Vec::new();
	for i in 0..intents.len() {
		// don't leave dust in intent due to rounding error
		if intents[i].0 + intent_deltas[i] < tolerance * intents[i].0 {
			deltas.push(-(intents[i].0));
		// don't trade dust amount due to rounding error
		} else if -intent_deltas[i] <= tolerance * intents[i].0 {
			deltas.push(0.);
		} else {
			deltas.push(intent_deltas[i]);
		}
	}
	deltas
}

fn add_buy_deltas(intent_prices: &[f64], sell_deltas: Vec<f64>) -> Vec<(f64, f64)> {
	let mut deltas = Vec::new();
	for i in 0..intent_prices.len() {
		let b = -sell_deltas[i] * intent_prices[i];
		deltas.push((sell_deltas[i], b));
	}
	deltas
}

fn diags(n: usize, m: usize, data: Vec<f64>) -> CscMatrix {
	let mut res = CscMatrix::zeros((n, m));
	for i in 0..n {
		res.set_entry((i, i), data[i]);
	}
	res
}

fn prepare_omnipool_data<AssetId>(
	info: Vec<OmnipoolAssetInfo<AssetId>>,
) -> (
	Vec<AssetId>,
	Vec<f64>,
	Vec<f64>,
	Vec<f64>,
	Vec<f64>,
	BTreeMap<AssetId, u8>,
)
where
	AssetId: sp_std::hash::Hash + Copy + Clone + Eq + Ord,
{
	let asset_ids = info.iter().map(|i| i.asset_id).collect::<Vec<_>>();
	let asset_reserves = info.iter().map(|i| i.reserve_as_f64()).collect::<Vec<_>>();
	let hub_reserves = info.iter().map(|i| i.hub_reserve_as_f64()).collect::<Vec<_>>();
	let fees = info.iter().map(|i| i.fee_as_f64()).collect::<Vec<_>>();
	let hub_fees = info.iter().map(|i| i.hub_fee_as_f64()).collect::<Vec<_>>();
	let decimals = info
		.iter()
		.map(|i| (i.asset_id, i.decimals))
		.collect::<BTreeMap<_, _>>();
	(asset_ids, asset_reserves, hub_reserves, fees, hub_fees, decimals)
}

fn prepare_intent_data<AccountId, AssetId>(
	intents: &[(IntentId, Intent<AccountId, AssetId>)],
) -> (Vec<AssetId>, Vec<f64>)
where
	AssetId: sp_std::hash::Hash + From<u32> + Copy + Clone + Eq + Ord,
{
	let mut asset_ids = BTreeSet::new();
	let mut intent_prices = Vec::new();
	for (_, intent) in intents {
		if intent.swap.asset_in != 1u32.into() {
			asset_ids.insert(intent.swap.asset_in);
		}
		if intent.swap.asset_out != 1u32.into() {
			//note: this should never happened, as it is not allowed to buy lrna!
			asset_ids.insert(intent.swap.asset_out);
		} else {
			debug_assert!(false, "It is not allowed to buy lrna!");
		}
		let amount_in = intent.swap.amount_in;
		let amount_out = intent.swap.amount_out;
		//let price = FixedU128::from_rational(amount_out , amount_in).to_float();
		let price = rational_to_f64!(amount_out, amount_in);
		intent_prices.push(price);
	}
	(asset_ids.iter().cloned().collect(), intent_prices)
}

pub struct OmniSolver<AccountId, AssetId, OI, R>(sp_std::marker::PhantomData<(AccountId, AssetId, OI, R)>);

impl<AccountId, AssetId, OI, R> Solver<(IntentId, Intent<AccountId, AssetId>)> for OmniSolver<AccountId, AssetId, OI, R>
where
	AssetId: From<u32> + sp_std::hash::Hash + PartialEq + Eq + Ord + Clone + Copy + core::fmt::Debug,
	OI: OmnipoolInfo<AssetId>,
	R: Routing<AssetId>,
{
	type Error = ();

	fn solve(intents: Vec<(IntentId, Intent<AccountId, AssetId>)>) -> Result<Vec<ResolvedIntent>, Self::Error> {
		// Prepare intent and omnipool data

		let (intent_asset_ids, intent_prices) = prepare_intent_data::<AccountId, AssetId>(&intents);
		let omnipool_data = OI::assets(Some(intent_asset_ids));

		let (asset_ids, asset_reserves, hub_reserves, fees, lrna_fees, decimals) =
			prepare_omnipool_data::<AssetId>(omnipool_data);

		let mut converted_intent_amounts: Vec<(f64, f64)> = Vec::new();

		for (_, intent) in intents.iter() {
			let amount_in = to_f64_by_decimals!(intent.swap.amount_in, *decimals.get(&intent.swap.asset_in).unwrap());
			let amount_out =
				to_f64_by_decimals!(intent.swap.amount_out, *decimals.get(&intent.swap.asset_out).unwrap());
			converted_intent_amounts.push((amount_in, amount_out));
		}

		let mut tkns: Vec<AssetId> = vec![1u32.into()];
		tkns.extend(asset_ids.iter().cloned());

		let fee_match = 0.0005;

		let mut spot_prices = vec![1.];

		let omnipool_spot_price: Vec<f64> = asset_reserves
			.iter()
			.zip(hub_reserves.iter())
			.map(|(r, h)| h / r)
			.collect();
		spot_prices.extend(omnipool_spot_price.iter().cloned());

		let s = spot_prices[1];

		// Setup Variables

		let n = asset_ids.len();
		let m = intents.len();
		let k = 4 * n + m;

		//calculate scaling, tau, phi
		let scaling = calculate_scaling::<AccountId, AssetId>(
			&intents,
			&converted_intent_amounts,
			&asset_ids,
			&asset_reserves,
			&hub_reserves,
		);
		let (tau, phi) = calculate_tau_phi::<AccountId, AssetId>(&intents, &tkns, &scaling);

		//#----------------------------#
		//#          OBJECTIVE         #
		//#----------------------------#
		let P: CscMatrix<f64> = CscMatrix::zeros((k, k));

		let delta_lrna_coefs = vec![1.; n];
		let lambda_lrna_coefs = vec![-1.; n];
		let delta_ceofs = omnipool_spot_price.clone();
		let lambda_coefs: Vec<f64> = fees
			.iter()
			.zip(omnipool_spot_price.iter())
			.map(|(f, sp)| (*f - 1.) * *sp)
			.collect();
		let mut d_coefs = Vec::new();
		for j in 0..m {
			let mut v = 0.;
			for i in 0..n + 1 {
				let p = phi.get_entry((i, j)).unwrap_or(0.);
				let ip = intent_prices[j];
				let t = tau.get_entry((i, j)).unwrap_or(0.);
				let sp = spot_prices[i];

				let a = p * ip - t;
				let a = a * sp;

				v += a;
			}
			d_coefs.push(v);
		}

		let mut q = Vec::new();
		q.extend(delta_lrna_coefs);
		q.extend(lambda_lrna_coefs);
		q.extend(delta_ceofs);
		q.extend(lambda_coefs);
		q.extend(d_coefs);

		//#----------------------------#
		//#        CONSTRAINTS         #
		//#----------------------------#

		let mut A1: CscMatrix<f64> = CscMatrix::identity(k);
		A1.negate();
		let b1: Vec<f64> = vec![0.; k];
		let cone1 = NonnegativeConeT(k);

		// intents cannot sell more than they have
		let amm_coefs = CscMatrix::zeros((m, 4 * n));
		let d_coefs = CscMatrix::identity(m);
		let A2 = CscMatrix::hcat(&amm_coefs, &d_coefs);
		let b2 = intents
			.iter()
			.enumerate()
			.map(|(idx, intent)| (converted_intent_amounts[idx].0, intent.1.swap.asset_in))
			.map(|(amount, asset_in)| amount / scaling[&asset_in])
			.collect::<Vec<_>>();
		let cone2 = NonnegativeConeT(m);

		//leftover must be higher than required fees

		let delta_lrna_coefs = vec![1.; n];
		let lambda_lrna_coefs: Vec<f64> = lrna_fees.iter().map(|f| f - 1.).collect();
		let zero_ceofs = vec![0.; 2 * n];
		let mut d_coefs = Vec::new();
		for i in 0..m {
			let v = tau.get_entry((0, i)).unwrap_or(0.);
			d_coefs.push(-v);
		}
		let mut q1 = Vec::new();
		q1.extend(delta_lrna_coefs);
		q1.extend(lambda_lrna_coefs);
		q1.extend(zero_ceofs);
		q1.extend(d_coefs);

		let A30 = CscMatrix::from(&[q1.clone()]);
		let b30 = vec![0.];

		// other assets
		let lrna_coefs = CscMatrix::zeros((n, 2 * n));
		let delta_coefs = CscMatrix::identity(n);
		let lambda_coefs = diags(n, n, fees.iter().map(|f| f - fee_match - 1.).collect());

		let d = (1..n + 1)
			.map(|i| {
				(0..m)
					.map(|j| {
						let p = phi.get_entry((i, j)).unwrap_or(0.);
						let ip = intent_prices[j];
						let t = tau.get_entry((i, j)).unwrap_or(0.);
						(p * ip) / (1. - fee_match) - t
					})
					.collect::<Vec<_>>()
			})
			.collect::<Vec<_>>();
		let d_coefs = CscMatrix::from(&d);

		let A31 = CscMatrix::hcat(&lrna_coefs, &delta_coefs);
		let A31 = CscMatrix::hcat(&A31, &lambda_coefs);
		let A31 = CscMatrix::hcat(&A31, &d_coefs);
		let b31 = vec![0.; n];
		let A3 = CscMatrix::vcat(&A30, &A31);
		let cone3 = NonnegativeConeT(n + 1);

		// AMM invariants must not go down
		let mut A4 = CscMatrix::zeros((3 * n, k));
		let b4 = vec![1.; 3 * n];
		let mut cones4 = Vec::new();

		let lrna_scaling = scaling.get(&1u32.into()).cloned().unwrap();
		for i in 0..n {
			let reserve = asset_reserves[i];
			let hub_reserve = hub_reserves[i];
			let asset_scaling = scaling.get(&asset_ids[i]).cloned().unwrap();

			let v = -lrna_scaling / hub_reserve;
			let v1 = -asset_scaling / reserve;

			A4.set_entry((3 * i, i), v);
			A4.set_entry((3 * i, n + i), -v);
			A4.set_entry((3 * i + 1, 2 * n + i), v1);
			A4.set_entry((3 * i + 1, 3 * n + i), -v1);
			cones4.push(PowerConeT(0.5));
		}
		let A = CscMatrix::vcat(&A1, &A2);
		let A = CscMatrix::vcat(&A, &A3);
		let A = CscMatrix::vcat(&A, &A4);

		let mut b = Vec::new();
		b.extend(b1);
		b.extend(b2);
		b.extend(b30);
		b.extend(b31);
		b.extend(b4);

		let mut cones = vec![cone1, cone2, cone3];
		cones.extend(cones4);

		let settings = DefaultSettingsBuilder::default()
			.verbose(false)
			.time_limit(f64::INFINITY)
			.max_iter(200)
			.build()
			.unwrap();
		let mut solver = DefaultSolver::new(&P, &q, &A, &b, &cones, settings);
		solver.solve();

		let status = solver.solution.status;
		let solve_time = solver.solution.solve_time;
		let x = solver.solution.x;

		let mut new_amm_deltas = BTreeMap::new();
		let mut exec_intent_deltas = vec![0.; intents.len()];
		for i in 0..n {
			let tkn = asset_ids[i];
			new_amm_deltas.insert(tkn, (x[2 * n + i] - x[3 * n + i]) * scaling.get(&tkn).unwrap());
		}
		for i in 0..intents.len() {
			exec_intent_deltas[i] = -x[4 * n + i] * scaling[&intents[i].1.swap.asset_in];
		}

		let sell_deltas = round_solution(&converted_intent_amounts, exec_intent_deltas, 0.0001);

		let intent_deltas = add_buy_deltas(&intent_prices, sell_deltas);

		// Construct the solution
		let mut resolved_intents = Vec::new();

		let convert_to_balance = |a: f64, dec: u8| -> Balance {
			let factor = 10u128.pow(dec as u32);
			(a * factor as f64) as Balance
		};

		// figure out trades and score
		// this could be probably extracted from here and simplified

		let mut amounts_in: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let mut amounts_out: BTreeMap<AssetId, Balance> = BTreeMap::new();

		for (i, intent) in intents.iter().enumerate() {
			if intent_deltas[i].0 != 0. && intent_deltas[i].1 != 0. {
				let resolved_intent = ResolvedIntent {
					intent_id: intent.0,
					amount_in: convert_to_balance(
						-intent_deltas[i].0,
						decimals.get(&intent.1.swap.asset_in).unwrap().clone(),
					),
					amount_out: convert_to_balance(
						intent_deltas[i].1,
						decimals.get(&intent.1.swap.asset_out).unwrap().clone(),
					),
				};
				amounts_in
					.entry(intent.1.swap.asset_in)
					.and_modify(|e| *e += resolved_intent.amount_in)
					.or_insert(resolved_intent.amount_in);
				amounts_out
					.entry(intent.1.swap.asset_out)
					.and_modify(|e| *e += resolved_intent.amount_out)
					.or_insert(resolved_intent.amount_out);
				resolved_intents.push(resolved_intent);
			}
		}

		let mut lrna_aquired = 0u128;

		let mut matched_amounts = Vec::new();
		let mut trades_instructions = Vec::new();

		// Sell all for lrna
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = *amounts_out.get(asset_id).unwrap_or(&0u128);

			matched_amounts.push((*asset_id, (*amount).min(amount_out)));

			if *amount > amount_out {
				let route = R::get_route(*asset_id, 1u32.into());
				let diff = amount.saturating_sub(amount_out);

				let lrna_bought = R::calculate_amount_out(&route, diff)?;
				lrna_aquired.saturating_accrue(lrna_bought);
				trades_instructions.push(TradeInstruction::SwapExactIn {
					asset_in: *asset_id,
					asset_out: 1u32.into(),                       // LRNA
					amount_in: amount.saturating_sub(amount_out), //Swap only difference
					amount_out: lrna_bought,
					route: BoundedRoute::try_from(route).unwrap(),
				});
			}
		}

		let mut lrna_sold = 0u128;

		for (asset_id, amount) in amounts_out {
			let amount_in = *amounts_in.get(&asset_id).unwrap_or(&0u128);

			if amount > amount_in {
				let route = R::get_route(1u32.into(), asset_id);
				let diff = amount.saturating_sub(amount_in);
				let lrna_in = R::calculate_amount_in(&route, diff)?;
				lrna_sold.saturating_accrue(lrna_in);
				trades_instructions.push(TradeInstruction::SwapExactOut {
					asset_in: 1u32.into(), // LRNA
					asset_out: asset_id,
					amount_in: lrna_in,
					amount_out: amount.saturating_sub(amount_in), //Swap only difference
					route: BoundedRoute::try_from(route).unwrap(),
				});
			}
		}
		assert!(
			lrna_aquired >= lrna_sold,
			"lrna_aquired < lrna_sold ({} < {})",
			lrna_aquired,
			lrna_sold
		);

		let mut score = resolved_intents.len() as u128 * 1_000_000_000_000;
		for (asset_id, amount) in matched_amounts {
			let price = R::hub_asset_price(asset_id)?;
			let h = multiply_by_rational_with_rounding(amount, price.n, price.d, sp_runtime::Rounding::Up).unwrap();
			score.saturating_accrue(h);
		}
		let score = (score / 1_000_000) as u64;

		/*
		let solution = SolverSolution {
			intents: resolved_intents,
			trades: trades_instructions,
			score,
		};

		 */

		Ok(resolved_intents)
	}
}
