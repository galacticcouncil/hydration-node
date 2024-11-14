#![allow(non_snake_case)]

use crate::{rational_to_f64, to_f64_by_decimals};
use pallet_ice::traits::{OmnipoolAssetInfo, OmnipoolInfo, Solver};
use pallet_ice::types::{Intent, IntentId, ResolvedIntent};
use primitives::{AccountId, AssetId, Balance};
use std::collections::{BTreeMap, BTreeSet};
use std::ptr::null;

use crate::data::process_omnipool_data;
use clarabel::algebra::*;
use clarabel::solver::*;
use highs::Problem;
use ndarray::{Array, Array1, Array2, Array3, ArrayBase, OwnedRepr, Ix1, Ix2, Ix3};
//use highs::
use crate::problem::{FloatType, ICEProblem, ProblemStatus, FLOAT_INF};

const ROUND_TOLERANCE: FloatType = 0.0001;

fn calculate_scaling<AccountId, AssetId>(
	intents: &[(IntentId, Intent<AccountId, AssetId>)],
	intent_amounts: &[(f64, f64)],
	asset_ids: &[AssetId],
	omnipool_reserves: &[f64],
	omnipool_hub_reserves: &[f64],
) -> BTreeMap<AssetId, f64>
where
	AssetId: From<u32> + std::hash::Hash + Copy + Clone + Eq + Ord,
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
	AssetId: From<u32> + std::hash::Hash + Copy + Clone + Eq + Ord,
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
	}
	(tau, phi)
}
fn convert_to_balance(a: f64, dec: u8) -> Balance {
	let factor = 10u128.pow(dec as u32);
	(a * factor as f64) as Balance
}

// note that intent_deltas are < 0
fn prepare_resolved_intents<AccountId, AssetId>(
	intents: &[(u128, Intent<AccountId, AssetId>)],
	asset_decimals: &BTreeMap<AssetId, u8>,
	converted_intent_amounts: &[(f64, f64)],
	intent_deltas: &[f64],
	intent_prices: &[f64],
	tolerance: f64,
) -> Vec<ResolvedIntent>
where
	AssetId: std::hash::Hash + Copy + Clone + Eq + Ord,
{
	let mut resolved_intents = Vec::new();

	for (idx, delta_in) in intent_deltas.iter().enumerate() {
		debug_assert!(converted_intent_amounts[idx].0 >= -delta_in, "delta in is too high!");
		let accepted_tolerance_amount = converted_intent_amounts[idx].0 * tolerance;
		let remainder = converted_intent_amounts[idx].0 + delta_in; // note that delta in is < 0
		let (amount_in, amount_out) = if remainder < accepted_tolerance_amount {
			// Do not leave dust, resolve the whole intent amount
			(intents[idx].1.swap.amount_in, intents[idx].1.swap.amount_out)
		} else if -delta_in <= accepted_tolerance_amount {
			// Do not trade dust
			(0u128, 0u128)
		} else {
			// just resolve solver amounts
			let amount_in = -delta_in;
			let amount_out = intent_prices[idx] * amount_in;
			(
				convert_to_balance(amount_in, *asset_decimals.get(&intents[idx].1.swap.asset_in).unwrap()),
				convert_to_balance(amount_out, *asset_decimals.get(&intents[idx].1.swap.asset_out).unwrap()),
			)
		};

		if amount_in == 0 || amount_out == 0 {
			continue;
		}
		let resolved_intent = ResolvedIntent {
			intent_id: intents[idx].0,
			amount_in,
			amount_out,
		};
		resolved_intents.push(resolved_intent);
	}

	resolved_intents
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
	AssetId: std::hash::Hash + Copy + Clone + Eq + Ord,
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
	AssetId: std::hash::Hash + From<u32> + Copy + Clone + Eq + Ord,
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
		let price = rational_to_f64!(amount_out, amount_in);
		intent_prices.push(price);
	}
	(asset_ids.iter().cloned().collect(), intent_prices)
}

pub struct SolverV3<OI>(std::marker::PhantomData<OI>);

impl<OI> Solver<(IntentId, Intent<AccountId, AssetId>)> for SolverV3<OI>
where
	OI: OmnipoolInfo<AssetId>,
{
	type Metadata = ();
	type Error = ();

	fn solve(
		intents: Vec<(IntentId, Intent<AccountId, AssetId>)>,
	) -> Result<(Vec<ResolvedIntent>, Self::Metadata), Self::Error> {
		let omnipool_data = OI::assets(None); //TODO: get only needed assets, but the list is from the next line
		let data = process_omnipool_data(omnipool_data);
		let problem = ICEProblem::new(intents, data);

		let (n, m, r) = (problem.n, problem.m, problem.r);

		let inf = FLOAT_INF;

		let k_milp = 4 * n + m + r;
		let mut Z_L = -inf;
		let mut Z_U = inf;
		let mut best_status = ProblemStatus::NotSolved;

		let mut y_best: Vec<usize> = Vec::new();
		let mut best_intent_deltas: Vec<FloatType> = Vec::new(); // m size
		let mut best_amm_deltas: Vec<FloatType> = Vec::new(); // n size
		let milp_ob = -inf;

		// Force small 	trades to execute
		// note this comes from initial solution which we skip for now
		// so nothing is mandatory just yet, but let;s prepare

		let exec_indices: Vec<usize> = vec![];
		let mut mandatory_indicators = vec![0; r];
		for &i in &exec_indices {
			mandatory_indicators[i] = 1;
		}

		let bk: Vec<usize> = mandatory_indicators
			.iter()
			.enumerate()
			.filter(|&(_, &val)| val == 1)
			.map(|(idx, _)| idx + 4 * n + m)
			.collect();

		let mut new_a = Array2::<f64>::zeros((1, k_milp));
		for &i in &bk {
			new_a[[0, i]] = 1.0;
		}

		let new_a_upper = Array1::from_elem(1, inf);
		let new_a_lower = Array1::from_elem(1, bk.len() as f64);

		let mut Z_U_archive = vec![];
		let mut Z_L_archive = vec![];
		let indicators = problem.get_indicators();
		let mut x_list = Array2::<f64>::zeros((0, 4 * n + m));

		for _i in 0..5 {
			// Set up problem with current indicators
			problem.set_up_problem(Some(&indicators));
			let (amm_deltas, intent_deltas, x, obj, dual_obj, status) = find_good_solution_unrounded(&problem, true);

			if obj < Z_U && dual_obj <= 0.0 {
				Z_U = obj;
				y_best = indicators.clone();
				best_amm_deltas = amm_deltas.clone();
				best_intent_deltas = intent_deltas.clone();
				best_status = status;
			}

			//TODO: figure out this
			/*
			if status != "PrimalInfeasible" && status != "DualInfeasible" {
				x_list = ndarray::stack![ndarray::Axis(0), x_list, x.view()];
			}
			 */

			// Get new cone constraint from current indicators
			let BK: Vec<usize> = indicators
				.iter()
				.enumerate()
				.filter(|&(_, &val)| val == 1)
				.map(|(idx, _)| idx + 4 * n + m)
				.collect();
			let NK: Vec<usize> = indicators
				.iter()
				.enumerate()
				.filter(|&(_, &val)| val == 0)
				.map(|(idx, _)| idx + 4 * n + m)
				.collect();
			let mut IC_A = Array2::<f64>::zeros((1, k_milp));
			for &i in &BK {
				IC_A[[0, i]] = 1.0;
			}
			for &i in &NK {
				IC_A[[0, i]] = -1.0;
			}
			let IC_upper = Array1::from_elem(1, (BK.len() - 1) as f64);
			let IC_lower = Array1::from_elem(1, -FLOAT_INF);

			// Add cone constraint to A, A_upper, A_lower
			let A = ndarray::stack![ndarray::Axis(0), new_a.view(), IC_A.view()];
			let A_upper = ndarray::concatenate![ndarray::Axis(0), new_a_upper.view(), IC_upper.view()];
			let A_lower = ndarray::concatenate![ndarray::Axis(0), new_a_lower.view(), IC_lower.view()];

			// Do MILP solve
			problem.set_up_problem(None);
			let (
				amm_deltas,
				partial_intent_deltas,
				indicators,
				new_a,
				new_a_upper,
				new_a_lower,
				milp_obj,
				valid,
				milp_status,
			) = solve_inclusion_problem(&problem, &x_list, Z_U, -FLOAT_INF, &A, &A_upper, &A_lower);
			Z_L = Z_L.max(milp_obj);
			Z_U_archive.push(Z_U);
			Z_L_archive.push(Z_L);
			if !valid {
				break;
			}
		}
		if best_status != ProblemStatus::Solved {
			// no solution found
			return Err(());
		}

		/*

		let sell_deltas = round_solution(&problem.partial_intents, best_intent_deltas);

		let partial_deltas_with_buys = add_buy_deltas(&problem.partial_intents, sell_deltas);
		let full_deltas_with_buys = problem
			.full_intents
			.iter()
			.enumerate()
			.map(|(l, _)| {
				if y_best[l] == 1 {
					[-problem.full_intents[l].sell_quantity, problem.full_intents[l].buy_quantity]
				} else {
					[0., 0.]
				}
			})
			.collect::<Vec<_>>();
		let mut deltas = vec![None; m + r];
		for (i, delta) in problem.partial_indices.iter().enumerate() {
			deltas[problem.partial_indices[i]] = partial_deltas_with_buys[i];
		}
		for (i, delta) in problem.full_indices.iter().enumerate() {
			deltas[problem.full_indices[i]] = full_deltas_with_buys[i];
		}
		let (deltas_final, obj) = add_small_trades(&problem, deltas);


		 */

		Err(())
	}
}

fn  solve_inclusion_problem(
	p0: &ICEProblem,
	p1: &ArrayBase<OwnedRepr<f64>, Ix2>,
	p2: FloatType,
	p3: FloatType,
	p4: &Array<f64, Ix3>,
	p5: &Array<FloatType, Ix1>,
	p6: &Array<f64, Ix1>,
) -> (
	Vec<FloatType>,
	Vec<FloatType>,
	Vec<FloatType>,
	Array2<f64>,
	Array1<FloatType>,
	Array1<FloatType>,
	FloatType,
	bool,
	ProblemStatus,
) {
	todo!()
}

fn find_good_solution_unrounded(
	p0: &ICEProblem,
	p1: bool,
) -> (
	Vec<FloatType>,
	Vec<FloatType>,
	Array2<f64>,
	FloatType,
	FloatType,
	ProblemStatus,
) {
	todo!()
}
