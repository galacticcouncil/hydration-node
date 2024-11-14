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
use ndarray::{Array, Array1, Array2, Array3, ArrayBase, Ix1, Ix2, Ix3, OwnedRepr};
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
			let (amm_deltas, intent_deltas, x, obj, dual_obj, status) =
				find_good_solution_unrounded(&problem, true, true, true, true);

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

fn solve_inclusion_problem(
	p: &ICEProblem,
	x_real_list: Option<Array2<f64>>, // NLP solution
	upper_bound: Option<f64>,
	lower_bound: Option<f64>,
	old_A: Option<Array2<f64>>,
	old_A_upper: Option<Array1<f64>>,
	old_A_lower: Option<Array1<f64>>,
) -> (
	BTreeMap<AssetId, f64>,
	Vec<Option<f64>>,
	Vec<i32>,
	Array2<f64>,
	Array1<f64>,
	Array1<f64>,
	f64,
	bool,
	String,
) {
	let asset_list = &p.asset_list;
	let tkn_list = vec!["LRNA".to_string()]
		.into_iter()
		.chain(asset_list.iter().cloned())
		.collect::<Vec<_>>();
	let (n, m, r) = (p.n, p.m, p.r);
	let k = 4 * n + m + r;

	let scaling = p.get_scaling();
	let x_list = x_real_list.map(|x| x.map_axis(Axis(1), |row| p.get_scaled_x(row.to_vec())));

	let inf = f64::INFINITY;

	let upper_bound = upper_bound.unwrap_or(inf);
	let lower_bound = lower_bound.unwrap_or(-inf);

	let partial_intent_sell_amts = p.get_partial_sell_maxs_scaled();

	let mut max_lambda_d = BTreeMap::new();
	let mut max_lrna_lambda_d = BTreeMap::new();
	let mut max_y_d = BTreeMap::new();
	let mut min_y_d = BTreeMap::new();
	let mut max_x_d = BTreeMap::new();
	let mut min_x_d = BTreeMap::new();

	for tkn in asset_list {
		max_lambda_d.insert(tkn.clone(), p.omnipool.liquidity[tkn] / scaling[tkn] / 2.0);
		max_lrna_lambda_d.insert(tkn.clone(), p.omnipool.lrna[tkn] / scaling["LRNA"] / 2.0);
		max_y_d.insert(tkn.clone(), max_lrna_lambda_d[tkn]);
		min_y_d.insert(tkn.clone(), -max_lrna_lambda_d[tkn]);
		max_x_d.insert(tkn.clone(), max_lambda_d[tkn]);
		min_x_d.insert(tkn.clone(), -max_lambda_d[tkn]);
	}

	let max_in = p.get_max_in();
	let max_out = p.get_max_out();

	for tkn in asset_list {
		if tkn != &p.tkn_profit {
			max_x_d.insert(tkn.clone(), max_in[tkn] / scaling[tkn] * 2.0);
			min_x_d.insert(tkn.clone(), -max_out[tkn] / scaling[tkn] * 2.0);
			max_lambda_d.insert(tkn.clone(), -min_x_d[tkn]);
			let max_y_unscaled =
				max_out[tkn] * p.omnipool.lrna[tkn] / (p.omnipool.liquidity[tkn] - max_out[tkn]) + max_in["LRNA"];
			max_y_d.insert(tkn.clone(), max_y_unscaled / scaling["LRNA"]);
			min_y_d.insert(
				tkn.clone(),
				-max_in[tkn] * p.omnipool.lrna[tkn] / (p.omnipool.liquidity[tkn] + max_in[tkn]) / scaling["LRNA"],
			);
			max_lrna_lambda_d.insert(tkn.clone(), -min_y_d[tkn]);
		}
	}

	let (
		mut min_y,
		mut max_y,
		mut min_x,
		mut max_x,
		mut min_lrna_lambda,
		mut max_lrna_lambda,
		mut min_lambda,
		mut max_lambda,
	) = p.get_scaled_bounds();
	let profit_i = asset_list.iter().position(|tkn| tkn == &p.tkn_profit).unwrap();
	max_x[profit_i] = inf;
	max_y[profit_i] = inf;
	min_lambda[profit_i] = 0.0;
	min_lrna_lambda[profit_i] = 0.0;

	min_y = min_y - 1.1 * min_y.abs();
	min_x = min_x - 1.1 * min_x.abs();
	min_lrna_lambda = min_lrna_lambda - 1.1 * min_lrna_lambda.abs();
	min_lambda = min_lambda - 1.1 * min_lambda.abs();
	max_y = max_y + 1.1 * max_y.abs();
	max_x = max_x + 1.1 * max_x.abs();
	max_lrna_lambda = max_lrna_lambda + 1.1 * max_lrna_lambda.abs();
	max_lambda = max_lambda + 1.1 * max_lambda.abs();

	let lower = ndarray::concatenate![
		Axis(0),
		min_y.view(),
		min_x.view(),
		min_lrna_lambda.view(),
		min_lambda.view(),
		Array1::zeros(m + r).view()
	];
	let upper = ndarray::concatenate![
		Axis(0),
		max_y.view(),
		max_x.view(),
		max_lrna_lambda.view(),
		max_lambda.view(),
		partial_intent_sell_amts.view(),
		Array1::ones(r).view()
	];

	let mut S = Array2::<f64>::zeros((n, k));
	let mut S_upper = Array1::<f64>::zeros(n);

	for (i, tkn) in asset_list.iter().enumerate() {
		let lrna_c = p.get_amm_lrna_coefs();
		let asset_c = p.get_amm_asset_coefs();
		S[[i, i]] = -lrna_c[tkn];
		S[[i, n + i]] = -asset_c[tkn];
	}

	if let Some(x_list) = x_list {
		for x in x_list.outer_iter() {
			for (i, tkn) in asset_list.iter().enumerate() {
				if x[i] != 0.0 || x[n + i] != 0.0 {
					let mut S_row = Array2::<f64>::zeros((1, k));
					let mut S_row_upper = Array1::<f64>::zeros(1);
					let lrna_c = p.get_amm_lrna_coefs();
					let asset_c = p.get_amm_asset_coefs();
					let grads_yi = -lrna_c[tkn] - lrna_c[tkn] * asset_c[tkn] * x[n + i];
					let grads_xi = -asset_c[tkn] - lrna_c[tkn] * asset_c[tkn] * x[i];
					S_row[[0, i]] = grads_yi;
					S_row[[0, n + i]] = grads_xi;
					let grad_dot_x = grads_yi * x[i] + grads_xi * x[n + i];
					let g_neg =
						lrna_c[tkn] * x[i] + asset_c[tkn] * x[n + i] + lrna_c[tkn] * asset_c[tkn] * x[i] * x[n + i];
					S_row_upper[0] = grad_dot_x + g_neg;
					S = ndarray::stack![Axis(0), S.view(), S_row.view()];
					S_upper = ndarray::concatenate![Axis(0), S_upper.view(), S_row_upper.view()];
				}
			}
		}
	}

	let S_lower = Array1::<f64>::from_elem(S_upper.len(), -inf);

	let A3 = p.get_profit_A();
	let A3_upper = Array1::<f64>::from_elem(n + 1, inf);
	let A3_lower = Array1::<f64>::zeros(n + 1);

	let mut A5 = Array2::<f64>::zeros((2 * n, k));
	for i in 0..n {
		A5[[i, i]] = 1.0;
		A5[[i, 2 * n + i]] = 1.0;
		A5[[n + i, n + i]] = 1.0;
		A5[[n + i, 3 * n + i]] = 1.0;
	}
	let A5_upper = Array1::<f64>::from_elem(2 * n, inf);
	let A5_lower = Array1::<f64>::zeros(2 * n);

	let mut A8 = Array2::<f64>::zeros((1, k));
	let q = p.get_q();
	A8.row_mut(0).assign(&(-q));
	let A8_upper = Array1::from_elem(1, upper_bound / scaling[&p.tkn_profit]);
	let A8_lower = Array1::from_elem(1, lower_bound / scaling[&p.tkn_profit]);

	let old_A = old_A.unwrap_or_else(|| Array2::<f64>::zeros((0, k)));
	let old_A_upper = old_A_upper.unwrap_or_else(|| Array1::<f64>::zeros(0));
	let old_A_lower = old_A_lower.unwrap_or_else(|| Array1::<f64>::zeros(0));

	let A = ndarray::stack![Axis(0), old_A.view(), S.view(), A3.view(), A5.view(), A8.view()];
	let A_upper = ndarray::concatenate![
		Axis(0),
		old_A_upper.view(),
		S_upper.view(),
		A3_upper.view(),
		A5_upper.view(),
		A8_upper.view()
	];
	let A_lower = ndarray::concatenate![
		Axis(0),
		old_A_lower.view(),
		S_lower.view(),
		A3_lower.view(),
		A5_lower.view(),
		A8_lower.view()
	];

	let mut nonzeros = Vec::new();
	let mut start = Vec::with_capacity(A.shape()[0] + 1);
	let mut a = Vec::new();
	start.push(0);
	for i in 0..A.shape()[0] {
		let row_nonzeros = A
			.row(i)
			.indexed_iter()
			.filter(|(_, &v)| v != 0.0)
			.map(|(j, _)| j)
			.collect::<Vec<_>>();
		nonzeros.extend(&row_nonzeros);
		start.push(nonzeros.len());
		a.extend(row_nonzeros.iter().map(|&j| A[[i, j]]));
	}

	let mut h = highs::Highs::new();
	let mut lp = highs::Problem::new();

	lp.num_col = k;
	lp.num_row = A.shape()[0];

	lp.col_cost = -q.to_vec();
	lp.col_lower = lower.to_vec();
	lp.col_upper = upper.to_vec();
	lp.row_lower = A_lower.to_vec();
	lp.row_upper = A_upper.to_vec();

	lp.a_matrix.format = highs::MatrixFormat::Rowwise;
	lp.a_matrix.start = start;
	lp.a_matrix.index = nonzeros;
	lp.a_matrix.value = a;

	lp.integrality = vec![highs::VarType::Continuous; 4 * n + m]
		.into_iter()
		.chain(vec![highs::VarType::Integer; r])
		.collect();

	h.pass_model(&lp);
	let options = h.get_options();
	options.small_matrix_value = 1e-12;
	options.primal_feasibility_tolerance = 1e-10;
	options.dual_feasibility_tolerance = 1e-10;
	options.mip_feasibility_tolerance = 1e-10;
	h.pass_options(&options);
	h.run();
	let status = h.get_model_status();
	let solution = h.get_solution();
	let info = h.get_info();
	let basis = h.get_basis();

	let x_expanded = solution.col_value;

	let mut new_amm_deltas = BTreeMap::new();
	let mut exec_partial_intent_deltas = vec![None; m];

	for (i, tkn) in asset_list.iter().enumerate() {
		new_amm_deltas.insert(tkn.clone(), x_expanded[n + i] * scaling[tkn]);
	}

	for i in 0..m {
		exec_partial_intent_deltas[i] = Some(-x_expanded[4 * n + i] * scaling[&p.partial_intents[i].tkn_sell]);
	}

	let exec_full_intent_flags = (0..r)
		.map(|i| if x_expanded[4 * n + m + i] > 0.5 { 1 } else { 0 })
		.collect::<Vec<_>>();

	let save_A = old_A.clone();
	let save_A_upper = old_A_upper.clone();
	let save_A_lower = old_A_lower.clone();

	(
		new_amm_deltas,
		exec_partial_intent_deltas,
		exec_full_intent_flags,
		save_A,
		save_A_upper,
		save_A_lower,
		-q.dot(&x_expanded) * scaling[&p.tkn_profit],
		solution.value_valid,
		status.to_string(),
	)
}

fn find_good_solution_unrounded(
	p: &ICEProblem,
	scale_trade_max: bool,
	approx_amm_eqs: bool,
	do_directional_run: bool,
	allow_loss: bool,
) -> (Vec<f64>, Vec<f64>, Array2<f64>, f64, f64, ProblemStatus) {
	let (n, m, r) = (p.n, p.m, p.r);
	if p.I.iter().sum::<f64>() + p.partial_sell_maxs.iter().sum::<f64>() == 0.0 {
		return (
			vec![0.0; p.asset_list.len()],
			vec![0.0; p.partial_intents.len()],
			Array2::zeros((4 * n + m, 1)),
			0.0,
			0.0,
			ProblemStatus::Solved,
		);
	}

	let (mut amm_deltas, mut intent_deltas, mut x, mut obj, mut dual_obj, mut status) =
		find_solution_unrounded(p, allow_loss);

	let mut trade_pcts: Vec<f64> = if scale_trade_max {
		p.partial_sell_maxs
			.iter()
			.enumerate()
			.map(|(i, &m)| if m > 0.0 { -intent_deltas[i] / m } else { 0.0 })
			.collect()
	} else {
		vec![1.0; p.partial_sell_maxs.len()]
	};
	trade_pcts.extend(vec![1.0; r]);

	let mut force_amm_approx: Option<BTreeMap<AssetId, &str>> = None;
	let mut approx_adjusted_ct = 0;

	if approx_amm_eqs && status != ProblemStatus::PrimalInfeasible && status != ProblemStatus::DualInfeasible {
		force_amm_approx = Some(p.asset_list.iter().map(|&tkn| (tkn, "full")).collect());
		let amm_pcts: BTreeMap<_, _> = p
			.asset_list
			.iter()
			.map(|&tkn| (tkn, (amm_deltas[tkn] / p.omnipool.liquidity[&tkn]).abs()))
			.collect();

		for &tkn in &p.asset_list {
			if let Some(force_amm_approx) = force_amm_approx.as_mut() {
				if amm_pcts[&tkn] <= 1e-6 {
					force_amm_approx.insert(tkn, "linear");
					approx_adjusted_ct += 1;
				} else if amm_pcts[&tkn] <= 1e-3 {
					force_amm_approx.insert(tkn, "quadratic");
					approx_adjusted_ct += 1;
				}
			}
		}
	}

	for _ in 0..100 {
		let trade_pcts_nonzero: Vec<_> = trade_pcts.iter().filter(|&&x| x > 0.0).collect();
		if (trade_pcts_nonzero.is_empty()
			|| trade_pcts_nonzero
				.iter()
				.cloned()
				.cloned()
				.min_by(|a, b| a.partial_cmp(b).unwrap())
				.unwrap() >= 0.1)
			&& approx_adjusted_ct == 0
		{
			break;
		}

		let (new_maxes, zero_ct) = if trade_pcts
			.iter()
			.cloned()
			.cloned()
			.min_by(|a, b| a.partial_cmp(b).unwrap())
			.unwrap() < 0.1
		{
			scale_down_partial_intents(p, &trade_pcts, 10)
		} else {
			(None, 0)
		};

		p.set_up_problem(new_maxes.as_ref(), false, force_amm_approx.as_ref());

		if zero_ct == m {
			break;
		}

		let (new_amm_deltas, new_intent_deltas, new_x, new_obj, new_dual_obj, new_status) =
			find_solution_unrounded(p, allow_loss);

		amm_deltas = new_amm_deltas;
		intent_deltas = new_intent_deltas;
		x = new_x;
		obj = new_obj;
		dual_obj = new_dual_obj;
		status = new_status;

		if scale_trade_max {
			trade_pcts = p
				.partial_sell_maxs
				.iter()
				.enumerate()
				.map(|(i, &m)| if m > 0.0 { -intent_deltas[i] / m } else { 0.0 })
				.collect();
			trade_pcts.extend(vec![1.0; r]);
		}

		if approx_amm_eqs && status != ProblemStatus::PrimalInfeasible && status != ProblemStatus::DualInfeasible {
			let amm_pcts: BTreeMap<_, _> = p
				.asset_list
				.iter()
				.map(|&tkn| (tkn, (amm_deltas[tkn] / p.omnipool.liquidity[&tkn]).abs()))
				.collect();

			approx_adjusted_ct = 0;
			for &tkn in &p.asset_list {
				if let Some(force_amm_approx) = force_amm_approx.as_mut() {
					match force_amm_approx[&tkn] {
						"linear" => {
							if amm_pcts[&tkn] > 1e-3 {
								force_amm_approx.insert(tkn, "full");
								approx_adjusted_ct += 1;
							} else if amm_pcts[&tkn] > 2e-6 {
								force_amm_approx.insert(tkn, "quadratic");
								approx_adjusted_ct += 1;
							}
						}
						"quadratic" => {
							if amm_pcts[&tkn] > 2e-3 {
								force_amm_approx.insert(tkn, "full");
								approx_adjusted_ct += 1;
							} else if amm_pcts[&tkn] <= 1e-6 {
								force_amm_approx.insert(tkn, "linear");
								approx_adjusted_ct += 1;
							}
						}
						_ => {
							if amm_pcts[&tkn] <= 1e-6 {
								force_amm_approx.insert(tkn, "linear");
								approx_adjusted_ct += 1;
							} else if amm_pcts[&tkn] <= 1e-3 {
								force_amm_approx.insert(tkn, "quadratic");
								approx_adjusted_ct += 1;
							}
						}
					}
				}
			}
		}
	}

	if do_directional_run {
		let flags = get_directional_flags(&amm_deltas);
		p.set_up_problem(Some(&flags), false, false, false);
		let (new_amm_deltas, new_intent_deltas, new_x, new_obj, new_dual_obj, new_status) =
			find_solution_unrounded(p, allow_loss);

		amm_deltas = new_amm_deltas;
		intent_deltas = new_intent_deltas;
		x = new_x;
		obj = new_obj;
		dual_obj = new_dual_obj;
		status = new_status;
	}

	if status == ProblemStatus::PrimalInfeasible || status == ProblemStatus::DualInfeasible {
		return (
			vec![0.0; n],
			vec![0.0; m],
			Array2::zeros((4 * n + m, 1)),
			0.0,
			0.0,
			status,
		);
	}

	let x_unscaled = p.get_real_x(&x);
	(amm_deltas, intent_deltas, x_unscaled, obj, dual_obj, status)
}

fn find_solution_unrounded(
	p: &ICEProblem,
	allow_loss: bool,
) -> (BTreeMap<AssetId, f64>, Vec<f64>, Array2<f64>, f64, f64, ProblemStatus) {
	if p.I.is_none() {
		panic!("I is None");
	}
	let I = p.I.as_ref().unwrap();
	if I.iter().sum::<f64>() + p.partial_sell_maxs.iter().sum::<f64>() == 0.0 {
		return (
			p.asset_list.iter().map(|&tkn| (tkn, 0.0)).collect(),
			vec![0.0; p.partial_intents.len()],
			Array2::zeros((4 * p.n + p.m, 1)),
			0.0,
			0.0,
			ProblemStatus::Solved,
		);
	}

	let full_intents = &p.full_intents;
	let partial_intents = &p.partial_intents;
	let state = &p.omnipool;
	let asset_list = &p.asset_list;
	let (n, m, r) = (p.n, p.m, p.r);

	if partial_intents.len() + I.iter().sum::<f64>() as usize == 0 {
		return (
			asset_list.iter().map(|&tkn| (tkn, 0.0)).collect(),
			vec![],
			Array2::zeros((4 * n, 1)),
			0.0,
			0.0,
			ProblemStatus::Solved,
		);
	}

	let directions = p.get_omnipool_directions();
	let k = 4 * n + m;
	let mut indices_to_keep: Vec<usize> = (0..k).collect();

	for &tkn in directions.keys() {
		if directions[&tkn] == "sell" || directions[&tkn] == "neither" {
			indices_to_keep.retain(|&i| i != 2 * n + asset_list.iter().position(|&x| x == tkn).unwrap());
		}
		if directions[&tkn] == "buy" || directions[&tkn] == "neither" {
			indices_to_keep.retain(|&i| i != 3 * n + asset_list.iter().position(|&x| x == tkn).unwrap());
		}
		if directions[&tkn] == "neither" {
			indices_to_keep.retain(|&i| i != asset_list.iter().position(|&x| x == tkn).unwrap());
			indices_to_keep.retain(|&i| i != n + asset_list.iter().position(|&x| x == tkn).unwrap());
		}
	}

	let k_real = indices_to_keep.len();
	let P_trimmed = CscMatrix::zeros((k_real, k_real));
	let q_all = p.get_q();
	let objective_I_coefs = -q_all.slice(s![4 * n + m..]);
	let q = -q_all.slice(s![..4 * n + m]);
	let q_trimmed: Vec<f64> = indices_to_keep.iter().map(|&i| q[i]).collect();

	let diff_coefs = Array2::<f64>::zeros((2 * n + m, 2 * n));
	let nonzero_coefs = -Array2::<f64>::eye(2 * n + m);
	let A1 = concatenate![Axis(1), diff_coefs, nonzero_coefs];
	let rows_to_keep: Vec<usize> = (0..2 * n + m)
		.filter(|&i| indices_to_keep.contains(&(2 * n + i)))
		.collect();
	let A1_trimmed = A1.select(Axis(0), &rows_to_keep).select(Axis(1), &indices_to_keep);
	let b1 = Array1::<f64>::zeros(A1_trimmed.shape()[0]);
	let cone1 = NonnegativeConeT::new(A1_trimmed.shape()[0]);

	let amm_coefs = Array2::<f64>::zeros((m, 4 * n));
	let d_coefs = Array2::<f64>::eye(m);
	let A2 = concatenate![Axis(1), amm_coefs, d_coefs];
	let b2 = Array1::from(p.get_partial_sell_maxs_scaled());
	let A2_trimmed = A2.select(Axis(1), &indices_to_keep);
	let cone2 = NonnegativeConeT::new(A2_trimmed.shape()[0]);

	let profit_A = p.get_profit_A();
	let mut A3 = -profit_A.slice(s![.., ..4 * n + m]).to_owned();
	let mut I_coefs = -profit_A.slice(s![.., 4 * n + m..]).to_owned();
	if allow_loss {
		let profit_i = p.asset_list.iter().position(|&x| x == p.tkn_profit).unwrap() + 1;
		A3.remove_index(Axis(0), profit_i);
		I_coefs.remove_index(Axis(0), profit_i);
	}
	let A3_trimmed = A3.select(Axis(1), &indices_to_keep);
	let b3 = if r == 0 {
		Array1::<f64>::zeros(A3_trimmed.shape()[0])
	} else {
		-I_coefs.dot(I)
	};
	let cone3 = NonnegativeConeT::new(A3_trimmed.shape()[0]);

	let mut A4 = Array2::<f64>::zeros((0, k));
	let mut b4 = Array1::<f64>::zeros(0);
	let mut cones4 = vec![];
	let epsilon_tkn = p.get_epsilon_tkn();

	for i in 0..n {
		let tkn = asset_list[i];
		let approx = p.get_amm_approx(tkn);
		let approx = if approx == "none" && epsilon_tkn[&tkn] <= 1e-6 && tkn != p.tkn_profit {
			"linear"
		} else if approx == "none" && epsilon_tkn[&tkn] <= 1e-3 {
			"quadratic"
		} else {
			approx
		};

		let (A4i, b4i, cone) = match approx {
			"linear" => {
				if !directions.contains_key(&tkn) {
					let c1 = 1.0 / (1.0 + epsilon_tkn[&tkn]);
					let c2 = 1.0 / (1.0 - epsilon_tkn[&tkn]);
					let mut A4i = Array2::<f64>::zeros((2, k));
					A4i[[0, i]] = -p.get_amm_lrna_coefs()[&tkn];
					A4i[[0, n + i]] = -p.get_amm_asset_coefs()[&tkn] * c1;
					A4i[[1, i]] = -p.get_amm_lrna_coefs()[&tkn];
					A4i[[1, n + i]] = -p.get_amm_asset_coefs()[&tkn] * c2;
					(A4i, Array1::<f64>::zeros(2), NonnegativeConeT::new(2))
				} else {
					let c = if directions[&tkn] == "sell" {
						1.0 / (1.0 - epsilon_tkn[&tkn])
					} else {
						1.0 / (1.0 + epsilon_tkn[&tkn])
					};
					let mut A4i = Array2::<f64>::zeros((1, k));
					A4i[[0, i]] = -p.get_amm_lrna_coefs()[&tkn];
					A4i[[0, n + i]] = -p.get_amm_asset_coefs()[&tkn] * c;
					(A4i, Array1::<f64>::zeros(1), ZeroConeT::new(1))
				}
			}
			"quadratic" => {
				let mut A4i = Array2::<f64>::zeros((3, k));
				A4i[[1, i]] = -p.get_amm_lrna_coefs()[&tkn];
				A4i[[1, n + i]] = -p.get_amm_asset_coefs()[&tkn];
				A4i[[2, n + i]] = -p.get_amm_asset_coefs()[&tkn];
				(A4i, array![1.0, 0.0, 0.0], PowerConeT::new(0.5))
			}
			_ => {
				let mut A4i = Array2::<f64>::zeros((3, k));
				A4i[[0, i]] = -p.get_amm_lrna_coefs()[&tkn];
				A4i[[1, n + i]] = -p.get_amm_asset_coefs()[&tkn];
				(A4i, Array1::<f64>::ones(3), PowerConeT::new(0.5))
			}
		};

		A4 = concatenate![Axis(0), A4, A4i];
		b4 = concatenate![Axis(0), b4, b4i];
		cones4.push(cone);
	}

	let A4_trimmed = A4.select(Axis(1), &indices_to_keep);

	let mut A5 = Array2::<f64>::zeros((0, k));
	let mut A6 = Array2::<f64>::zeros((0, k));
	let mut A7 = Array2::<f64>::zeros((0, k));

	for i in 0..n {
		let tkn = asset_list[i];
		if !directions.contains_key(&tkn) {
			let mut A5i = Array2::<f64>::zeros((2, k));
			A5i[[0, i]] = -1.0;
			A5i[[0, 2 * n + i]] = -1.0;
			A5i[[1, n + i]] = -1.0;
			A5i[[1, 3 * n + i]] = -1.0;
			A5 = concatenate![Axis(0), A5, A5i];
		} else {
			let mut A6i = Array2::<f64>::zeros((2, k));
			let mut A7i = Array2::<f64>::zeros((1, k));
			if directions[&tkn] == "sell" {
				A6i[[0, i]] = -1.0;
				A6i[[1, n + i]] = 1.0;
				A7i[[0, n + i]] = 1.0;
				A7i[[0, 3 * n + i]] = 1.0;
			} else {
				A6i[[0, i]] = 1.0;
				A6i[[1, n + i]] = -1.0;
				A7i[[0, i]] = 1.0;
				A7i[[0, 2 * n + i]] = 1.0;
			}
			A6 = concatenate![Axis(0), A6, A6i];
			A7 = concatenate![Axis(0), A7, A7i];
		}
	}

	let A5_trimmed = A5.select(Axis(1), &indices_to_keep);
	let A6_trimmed = A6.select(Axis(1), &indices_to_keep);
	let A7_trimmed = A7.select(Axis(1), &indices_to_keep);

	let b5 = Array1::<f64>::zeros(A5.shape()[0]);
	let b6 = Array1::<f64>::zeros(A6.shape()[0]);
	let b7 = Array1::<f64>::zeros(A7.shape()[0]);
	let cone5 = NonnegativeConeT::new(A5.shape()[0]);
	let cone6 = NonnegativeConeT::new(A6.shape()[0]);
	let cone7 = ZeroConeT::new(A7.shape()[0]);

	let A = concatenate![
		Axis(0),
		A1_trimmed,
		A2_trimmed,
		A3_trimmed,
		A4_trimmed,
		A5_trimmed,
		A6_trimmed,
		A7_trimmed
	];
	let A_sparse = CscMatrix::from(&A);
	let b = concatenate![Axis(0), b1, b2, b3, b4, b5, b6, b7];
	let cones = vec![cone1, cone2, cone3]
		.into_iter()
		.chain(cones4.into_iter())
		.chain(vec![cone5, cone6, cone7].into_iter())
		.collect::<Vec<_>>();

	let settings = DefaultSettings::default();
	let mut solver = DefaultSolver::new(P_trimmed, q_trimmed, A_sparse, b, cones, settings);
	let solution = solver.solve().unwrap();
	let x = solution.x;
	let z = solution.z;
	let s = solution.s;

	let mut new_amm_deltas = BTreeMap::new();
	let mut exec_intent_deltas = vec![0.0; partial_intents.len()];
	let mut x_expanded = vec![0.0; k];
	for (i, &index) in indices_to_keep.iter().enumerate() {
		x_expanded[index] = x[i];
	}
	let x_scaled = p.get_real_x(&x_expanded);
	for i in 0..n {
		let tkn = asset_list[i];
		new_amm_deltas.insert(tkn, x_scaled[n + i]);
	}
	for j in 0..partial_intents.len() {
		exec_intent_deltas[j] = -x_scaled[4 * n + j];
	}

	let obj_offset = if let Some(I) = I { objective_I_coefs.dot(I) } else { 0.0 };
	(
		new_amm_deltas,
		exec_intent_deltas,
		Array2::from_shape_vec((k, 1), x_expanded).unwrap(),
		p.scale_obj_amt(solution.obj_val + obj_offset),
		p.scale_obj_amt(solution.obj_val_dual + obj_offset),
		solution.status.into(),
	)
}

fn scale_down_partial_intents(p: &ICEProblem, trade_pcts: &[f64], scale: f64) -> (Vec<f64>, usize) {
	let mut zero_ct = 0;
	let mut intent_sell_maxs = p.partial_sell_maxs.clone();

	for (i, &m) in p.partial_sell_maxs.iter().enumerate() {
		let old_sell_quantity = m * trade_pcts[i];
		let mut new_sell_max = m / scale;

		if old_sell_quantity < new_sell_max {
			let tkn = p.partial_intents[i].tkn_sell;
			let sell_amt_lrna_value = new_sell_max * p.omnipool.price(tkn);

			if sell_amt_lrna_value < p.min_partial {
				new_sell_max = 0.0;
				zero_ct += 1;
			}
			intent_sell_maxs[i] = new_sell_max;
		}
	}

	(intent_sell_maxs, zero_ct)
}

fn get_directional_flags(amm_deltas: &BTreeMap<AssetId, f64>) -> BTreeMap<AssetId, i32> {
	let mut flags = BTreeMap::new();
	for (&tkn, &delta) in amm_deltas.iter() {
		let flag = if delta > 0.0 {
			1
		} else if delta < 0.0 {
			-1
		} else {
			0
		};
		flags.insert(tkn, flag);
	}
	flags
}
