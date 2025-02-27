#![allow(non_snake_case)]

use crate::data::process_data;
use crate::problem::{AmmApprox, Direction, ICEProblem, ProblemStatus, SetupParams, FLOAT_INF};
use crate::types::{AssetId, Balance, FloatType, Intent, ResolvedIntent};
use clarabel::algebra::*;
use clarabel::solver::*;
use highs::{HighsModelStatus, Sense};
use ndarray::{s, Array1, Array2, Axis};
use std::collections::BTreeMap;
use std::ops::Neg;

const ROUND_TOLERANCE: FloatType = 0.0001;
const LRNA: AssetId = 1;

fn calculate_scaling(
	intents: &[Intent],
	intent_amounts: &[(f64, f64)],
	asset_ids: &[AssetId],
	omnipool_reserves: &[f64],
	omnipool_hub_reserves: &[f64],
) -> BTreeMap<AssetId, f64> {
	let mut scaling = BTreeMap::new();
	scaling.insert(1u32.into(), f64::INFINITY);

	for (idx, intent) in intents.iter().enumerate() {
		if intent.asset_in != 1u32 {
			let a = intent.asset_in;
			let sq = intent_amounts[idx].0;
			scaling.entry(a).and_modify(|v| *v = v.max(sq)).or_insert(sq);
		}
		if intent.asset_out != 1u32 {
			let a = intent.asset_out;
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
		scaling.entry(1u32).and_modify(|v| *v = v.min(scalar)).or_insert(scalar);
	}

	scaling
}

fn calculate_tau_phi(
	intents: &[Intent],
	asset_ids: &[AssetId],
	scaling: &BTreeMap<AssetId, f64>,
) -> (CscMatrix, CscMatrix) {
	let n = asset_ids.len();
	let m = intents.len();
	let mut tau = CscMatrix::zeros((n, m));
	let mut phi = CscMatrix::zeros((n, m));
	for (j, intent) in intents.iter().enumerate() {
		let sell_i = asset_ids.iter().position(|&tkn| tkn == intent.asset_in).unwrap();
		let buy_i = asset_ids.iter().position(|&tkn| tkn == intent.asset_out).unwrap();
		tau.set_entry((sell_i, j), 1.);
		let s = scaling.get(&intent.asset_in).unwrap() / scaling.get(&intent.asset_out).unwrap();
		phi.set_entry((buy_i, j), s);
	}
	(tau, phi)
}
fn convert_to_balance(a: f64, dec: u8) -> Balance {
	let factor = 10u128.pow(dec as u32);
	(a * factor as f64) as Balance
}

// note that intent_deltas are < 0
fn prepare_resolved_intents(
	intents: &[Intent],
	asset_decimals: &BTreeMap<AssetId, u8>,
	converted_intent_amounts: &[(f64, f64)],
	intent_deltas: &[f64],
	intent_prices: &[f64],
	tolerance: f64,
) -> Vec<ResolvedIntent> {
	let mut resolved_intents = Vec::new();

	for (idx, delta_in) in intent_deltas.iter().enumerate() {
		debug_assert!(converted_intent_amounts[idx].0 >= -delta_in, "delta in is too high!");
		let accepted_tolerance_amount = converted_intent_amounts[idx].0 * tolerance;
		let remainder = converted_intent_amounts[idx].0 + delta_in; // note that delta in is < 0
		let (amount_in, amount_out) = if remainder < accepted_tolerance_amount {
			// Do not leave dust, resolve the whole intent amount
			(intents[idx].amount_in, intents[idx].amount_out)
		} else if -delta_in <= accepted_tolerance_amount {
			// Do not trade dust
			(0u128, 0u128)
		} else {
			// just resolve solver amounts
			let amount_in = -delta_in;
			let amount_out = intent_prices[idx] * amount_in;
			(
				convert_to_balance(amount_in, *asset_decimals.get(&intents[idx].asset_in).unwrap()),
				convert_to_balance(amount_out, *asset_decimals.get(&intents[idx].asset_out).unwrap()),
			)
		};

		if amount_in == 0 || amount_out == 0 {
			continue;
		}
		let resolved_intent = ResolvedIntent {
			intent_id: intents[idx].intent_id,
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

fn add_buy_deltas(intents: Vec<(FloatType, FloatType)>, sell_deltas: Vec<FloatType>) -> Vec<(FloatType, FloatType)> {
	let mut deltas = Vec::new();
	for (i, (amount_in, amount_out)) in intents.iter().enumerate() {
		let sell_delta = sell_deltas[i];
		let buy_delta = -sell_delta * amount_out / amount_in;
		deltas.push((sell_delta, buy_delta));
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

pub struct SolverResult {
	pub resolved_intents: Vec<ResolvedIntent>,
}

pub struct SolverV3;

impl SolverV3 {
	pub fn solve(intents: Vec<Intent>, pool_data: Vec<crate::types::Asset>) -> Result<SolverResult, ()> {
		if intents.is_empty() {
			return Ok(SolverResult {
				resolved_intents: vec![],
			});
		}
		let store = process_data(pool_data);
		let mut problem = ICEProblem::new().with_intents(intents).with_amm_store(store);
		problem.prepare();

		let (n, m, r) = (problem.n, problem.m, problem.r);

		let inf = FLOAT_INF;

		let k_milp = 4 * n + m + r;
		let mut Z_L = -inf;
		let mut Z_U = inf;
		let mut best_status = ProblemStatus::NotSolved;

		let mut y_best: Vec<usize> = Vec::new();
		let mut best_intent_deltas: Vec<FloatType> = Vec::new(); // m size
		let mut best_amm_deltas: BTreeMap<AssetId, FloatType> = BTreeMap::new(); // n size
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

		let mut new_a_upper = Array1::from_elem(1, inf);
		let mut new_a_lower = Array1::from_elem(1, bk.len() as f64);

		let mut Z_U_archive = vec![];
		let mut Z_L_archive = vec![];
		let indicators = problem.get_indicators().unwrap_or(vec![0; r]);
		let mut x_list = Array2::<f64>::zeros((0, 4 * n + m));

		let mut iter_indicators = indicators.clone();

		for _i in 0..5 {
			let params = SetupParams::new().with_indicators(iter_indicators.clone());
			problem.set_up_problem(params);
			let (amm_deltas, intent_deltas, x, obj, dual_obj, status) =
				find_good_solution_unrounded(&problem, true, true, true, true);

			if obj < Z_U && dual_obj <= 0.0 {
				Z_U = obj;
				y_best = iter_indicators.clone();
				best_amm_deltas = amm_deltas.clone();
				best_intent_deltas = intent_deltas.clone();
				best_status = status;
			}

			if status != ProblemStatus::PrimalInfeasible && status != ProblemStatus::DualInfeasible {
				//TODO: verify if this is correct
				let x2 = Array2::from_shape_vec((1, 4 * n + m), x).unwrap();
				x_list = ndarray::concatenate![Axis(0), x_list, x2];
			}

			// Get new cone constraint from current indicators
			let BK: Vec<usize> = iter_indicators
				.iter()
				.enumerate()
				.filter(|&(_, &val)| val == 1)
				.map(|(idx, _)| idx + 4 * n + m)
				.collect();
			let NK: Vec<usize> = iter_indicators
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
			let IC_upper = Array1::from_elem(1, BK.len() as f64 - 1.);
			let IC_lower = Array1::from_elem(1, -FLOAT_INF);

			// Add cone constraint to A, A_upper, A_lower
			let A = ndarray::concatenate![ndarray::Axis(0), new_a.view(), IC_A.view()];
			let A_upper = ndarray::concatenate![ndarray::Axis(0), new_a_upper.view(), IC_upper.view()];
			let A_lower = ndarray::concatenate![ndarray::Axis(0), new_a_lower.view(), IC_lower.view()];

			problem.set_up_problem(SetupParams::new());
			let (amm_deltas, partial_intent_deltas, indicators, s_new_a, s_new_a_upper, s_new_a_lower, milp_obj, valid) =
				solve_inclusion_problem(
					&problem,
					Some(x_list.clone()),
					Some(Z_U),
					Some(-FLOAT_INF),
					Some(A),
					Some(A_upper),
					Some(A_lower),
				);

			if !valid {
				break;
			}
			iter_indicators = indicators;
			new_a = s_new_a;
			new_a_upper = s_new_a_upper;
			new_a_lower = s_new_a_lower;
			Z_L = Z_L.max(milp_obj);
			Z_U_archive.push(Z_U);
			Z_L_archive.push(Z_L);
		}
		if best_status != ProblemStatus::Solved {
			// no solution found
			return Err(());
		}

		let sell_deltas = round_solution(
			&problem.get_partial_intents_amounts(),
			best_intent_deltas,
			ROUND_TOLERANCE,
		);
		let partial_deltas_with_buys = add_buy_deltas(problem.get_partial_intents_amounts(), sell_deltas);

		let full_deltas_with_buys = problem
			.get_full_intents_amounts()
			.iter()
			.enumerate()
			.map(|(l, (amount_in, amount_out))| {
				if y_best[l] == 1 {
					(-amount_in, *amount_out)
				} else {
					(0., 0.)
				}
			})
			.collect::<Vec<_>>();

		let mut deltas = vec![None; m + r];
		for (i, delta) in problem.partial_indices.iter().enumerate() {
			deltas[problem.partial_indices[i]] = Some(partial_deltas_with_buys[i]);
		}
		for (i, delta) in problem.full_indices.iter().enumerate() {
			deltas[problem.full_indices[i]] = Some(full_deltas_with_buys[i]);
		}

		//TODO: add this
		//let (deltas_final, obj) = add_small_trades(&problem, deltas);

		// Construct resolved intents
		let mut resolved_intents = Vec::new();

		for (idx, intent_delta) in deltas.iter().enumerate() {
			if let Some((delta_in, delta_out)) = intent_delta {
				let intent = &problem.intents[idx];
				let converted_intent_amount = problem.intent_amounts[idx];
				debug_assert!(converted_intent_amount.0 >= -delta_in, "delta in is too high!");

				let accepted_tolerance_amount = converted_intent_amount.0 * ROUND_TOLERANCE;
				let remainder = converted_intent_amount.0 + delta_in; // note that delta in is < 0
				let (amount_in, amount_out) = if remainder < accepted_tolerance_amount {
					// Do not leave dust, resolve the whole intent amount
					(intent.amount_in, intent.amount_out)
				} else if -delta_in <= accepted_tolerance_amount {
					// Do not trade dust
					(0u128, 0u128)
				} else {
					// just resolve solver amounts
					let amount_in = -delta_in;
					let amount_out = *delta_out;
					(
						convert_to_balance(amount_in, problem.get_asset_pool_data(intent.asset_in).decimals),
						convert_to_balance(amount_out, problem.get_asset_pool_data(intent.asset_out).decimals),
					)
				};

				if amount_in == 0 || amount_out == 0 {
					continue;
				}
				let resolved_intent = ResolvedIntent {
					intent_id: problem.intent_ids[idx],
					amount_in,
					amount_out,
				};
				resolved_intents.push(resolved_intent);
			}
		}

		Ok(SolverResult { resolved_intents })
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
	Vec<usize>,
	Array2<f64>,
	Array1<f64>,
	Array1<f64>,
	f64,
	bool,
) {
	let asset_list = p.asset_ids.clone();
	let tkn_list = vec![1u32]
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

	for tkn in asset_list.iter() {
		max_lambda_d.insert(
			tkn.clone(),
			p.get_asset_pool_data(*tkn).reserve / scaling.get(tkn).unwrap() / 2.0,
		);
		max_lrna_lambda_d.insert(
			tkn.clone(),
			p.get_asset_pool_data(*tkn).hub_reserve / scaling.get(&LRNA).unwrap() / 2.0,
		);
		max_y_d.insert(tkn.clone(), *max_lrna_lambda_d.get(tkn).unwrap());
		min_y_d.insert(tkn.clone(), -max_lrna_lambda_d.get(tkn).unwrap());
		max_x_d.insert(tkn.clone(), *max_lambda_d.get(tkn).unwrap());
		min_x_d.insert(tkn.clone(), -max_lambda_d.get(tkn).unwrap());
	}

	let max_in = p.get_max_in();
	let max_out = p.get_max_out();

	for tkn in asset_list.iter() {
		if *tkn != p.tkn_profit {
			max_x_d.insert(
				tkn.clone(),
				max_in.get(&tkn).unwrap() / scaling.get(&tkn).unwrap() * 2.0,
			);
			min_x_d.insert(
				tkn.clone(),
				-max_out.get(&tkn).unwrap() / scaling.get(&tkn).unwrap() * 2.0,
			);
			max_lambda_d.insert(tkn.clone(), -min_x_d.get(&tkn).unwrap());
			let max_y_unscaled = max_out.get(&tkn).unwrap() * p.get_asset_pool_data(*tkn).hub_reserve
				/ (p.get_asset_pool_data(*tkn).reserve - max_out.get(&tkn).unwrap())
				+ max_in.get(&LRNA).unwrap();
			max_y_d.insert(tkn.clone(), max_y_unscaled / scaling.get(&LRNA).unwrap());
			min_y_d.insert(
				tkn.clone(),
				-max_in.get(&tkn).unwrap() * p.get_asset_pool_data(*tkn).hub_reserve
					/ (p.get_asset_pool_data(*tkn).reserve + max_in.get(&tkn).unwrap())
					/ scaling.get(&LRNA).unwrap(),
			);
			max_lrna_lambda_d.insert(tkn.clone(), -min_y_d.get(&tkn).unwrap());
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

	min_y = min_y.clone() - 1.1 * min_y.abs();
	min_x = min_x.clone() - 1.1 * min_x.abs();
	min_lrna_lambda = min_lrna_lambda.clone() - 1.1 * min_lrna_lambda.abs();
	min_lambda = min_lambda.clone() - 1.1 * min_lambda.abs();
	max_y = max_y.clone() + 1.1 * max_y.abs();
	max_x = max_x.clone() + 1.1 * max_x.abs();
	max_lrna_lambda = max_lrna_lambda.clone() + 1.1 * max_lrna_lambda.abs();
	max_lambda = max_lambda.clone() + 1.1 * max_lambda.abs();

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
		partial_intent_sell_amts,
		Array1::ones(r).view()
	];

	let mut S = Array2::<f64>::zeros((n, k));
	let mut S_upper = Array1::<f64>::zeros(n);

	for (i, tkn) in asset_list.iter().enumerate() {
		let lrna_c = p.get_amm_lrna_coefs();
		let asset_c = p.get_amm_asset_coefs();
		S[[i, i]] = -lrna_c.get(&tkn).unwrap();
		S[[i, n + i]] = -asset_c.get(&tkn).unwrap();
	}

	if let Some(x_list) = x_list {
		for x in x_list.iter() {
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
					S = ndarray::concatenate![Axis(0), S.view(), S_row.view()];
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
	let q_a = ndarray::Array1::from(q.clone());
	A8.row_mut(0).assign(&(-q_a));
	let A8_upper = Array1::from_elem(1, upper_bound / scaling[&p.tkn_profit]);
	let A8_lower = Array1::from_elem(1, lower_bound / scaling[&p.tkn_profit]);

	let old_A = old_A.unwrap_or_else(|| Array2::<f64>::zeros((0, k)));
	let old_A_upper = old_A_upper.unwrap_or_else(|| Array1::<f64>::zeros(0));
	let old_A_lower = old_A_lower.unwrap_or_else(|| Array1::<f64>::zeros(0));

	let A = ndarray::concatenate![Axis(0), old_A.view(), S.view(), A3.view(), A5.view(), A8.view()];
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

	let mut pb = highs::RowProblem::new();

	let mut col_cost = vec![];
	for (idx, &v) in q.iter().enumerate() {
		let lower_bound = lower[idx];
		let upper_bound = upper[idx];
		let x = pb.add_column(-v, lower_bound..upper_bound);
		col_cost.push(x);
	}

	for (idx, row) in A.outer_iter().enumerate() {
		let v = row.to_vec();
		// now zip v with col_cost
		let v = v.iter().zip(col_cost.iter()).map(|(a, b)| (*b, *a)).collect::<Vec<_>>();
		let lower_bound = A_lower[idx];
		let upper_bound = A_upper[idx];
		pb.add_row(lower_bound..upper_bound, v);
	}
	let mut model = pb.optimise(Sense::Minimise);
	model.set_option("small_matrix_value", 1e-12);
	model.set_option("primal_feasibility_tolerance", 1e-10);
	model.set_option("dual_feasibility_tolerance", 1e-10);
	model.set_option("mip_feasibility_tolerance", 1e-10);

	let solved = model.solve();
	let status = solved.status();
	let solution = solved.get_solution();
	let x_expanded = solution.columns().to_vec();
	let value_valid = status != HighsModelStatus::Infeasible;

	/*

	//TODO: should we integrality and options ?!! seems to work without that
	lp.integrality = vec![highs::VarType::Continuous; 4 * n + m]
		.into_iter()
		.chain(vec![highs::VarType::Integer; r])
		.collect();
	let options = h.get_options();
	options.small_matrix_value = 1e-12;
	options.primal_feasibility_tolerance = 1e-10;
	options.dual_feasibility_tolerance = 1e-10;
	options.mip_feasibility_tolerance = 1e-10;
	let status = h.get_model_status();
	let solution = h.get_solution();
	let info = h.get_info();
	let basis = h.get_basis();
	let value_valid = solution.value_valid,
	let status  = status.to_string(),
	let x_expanded = solution.col_value;
	 */

	let mut new_amm_deltas = BTreeMap::new();
	let mut exec_partial_intent_deltas = vec![None; m];

	for i in 0..n {
		let tkn = tkn_list[i + 1];
		new_amm_deltas.insert(tkn, x_expanded[n + i] * scaling[&tkn]);
	}

	for i in 0..m {
		exec_partial_intent_deltas[i] =
			Some(-x_expanded[4 * n + i] * scaling[&p.get_intent(p.partial_indices[i]).asset_in]);
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
		-q.clone().dot(&x_expanded) * scaling[&p.tkn_profit],
		value_valid,
	)
}

fn find_good_solution_unrounded(
	problem: &ICEProblem,
	scale_trade_max: bool,
	approx_amm_eqs: bool,
	do_directional_run: bool,
	allow_loss: bool,
) -> (BTreeMap<AssetId, f64>, Vec<f64>, Vec<f64>, f64, f64, ProblemStatus) {
	let mut p: ICEProblem = problem.clone();
	let (n, m, r) = (p.n, p.m, p.r);
	if p.get_indicators_len() as f64 + p.partial_sell_maxs.iter().sum::<f64>() == 0.0 {
		// nothing to execute
		return (
			BTreeMap::new(),
			vec![0.0; p.partial_indices.len()],
			vec![0.; 4 * n + m],
			0.0,
			0.0,
			ProblemStatus::Solved,
		);
	}

	let (mut amm_deltas, mut intent_deltas, mut x, mut obj, mut dual_obj, mut status) =
		find_solution_unrounded(&p, allow_loss);

	// if partial trade size is much higher than executed trade, lower trade max
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

	// adjust AMM constraint approximation based on percent of Omnipool liquidity traded with AMM
	let mut force_amm_approx: Option<BTreeMap<AssetId, AmmApprox>> = None;
	let mut approx_adjusted_ct = 0;

	if approx_amm_eqs && status != ProblemStatus::PrimalInfeasible && status != ProblemStatus::DualInfeasible {
		force_amm_approx = Some(p.asset_ids.iter().map(|&tkn| (tkn, AmmApprox::Full)).collect());
		let amm_pcts: BTreeMap<_, _> = p
			.asset_ids
			.iter()
			.map(|&tkn| {
				(
					tkn,
					(amm_deltas.get(&tkn).unwrap() / p.get_asset_pool_data(tkn).reserve).abs(),
				)
			})
			.collect();

		for &tkn in &p.asset_ids {
			if let Some(force_amm_approx) = force_amm_approx.as_mut() {
				if amm_pcts[&tkn] <= 1e-6 {
					force_amm_approx.insert(tkn, AmmApprox::Linear);
					approx_adjusted_ct += 1;
				} else if amm_pcts[&tkn] <= 1e-3 {
					force_amm_approx.insert(tkn, AmmApprox::Quadratic);
					approx_adjusted_ct += 1;
				}
			}
		}
	}

	for iteration in 0..100 {
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
			.min_by(|a, b| a.partial_cmp(b).unwrap())
			.unwrap() < 0.1
		{
			scale_down_partial_intents(&p, &trade_pcts, 10.)
		} else {
			(None, 0)
		};

		if zero_ct == m {
			// all partial intents have been eliminated from execution
			break;
		}

		let params = SetupParams::new().with_clear_indicators(false);
		let params = if let Some(force_amm_approx) = force_amm_approx.as_ref() {
			params.with_force_amm_approx(force_amm_approx.clone())
		} else {
			params
		};
		let params = if let Some(nm) = new_maxes {
			params.with_sell_maxes(nm)
		} else {
			params
		};
		p.set_up_problem(params);

		let (new_amm_deltas, new_intent_deltas, new_x, new_obj, new_dual_obj, new_status) =
			find_solution_unrounded(&p, allow_loss);

		// need to check if amm_deltas stayed within their reasonable approximation bounds
		// if not, we may want to discard the "solution"

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
				.asset_ids
				.iter()
				.map(|&tkn| {
					(
						tkn,
						(amm_deltas.get(&tkn).unwrap() / p.get_asset_pool_data(tkn).reserve).abs(),
					)
				})
				.collect();

			approx_adjusted_ct = 0;
			for &tkn in &p.asset_ids {
				if let Some(force_amm_approx) = force_amm_approx.as_mut() {
					match force_amm_approx[&tkn] {
						AmmApprox::Linear => {
							if amm_pcts[&tkn] > 1e-3 {
								force_amm_approx.insert(tkn, AmmApprox::Full);
								approx_adjusted_ct += 1;
							} else if amm_pcts[&tkn] > 2e-6 {
								force_amm_approx.insert(tkn, AmmApprox::Quadratic);
								approx_adjusted_ct += 1;
							}
						}
						AmmApprox::Quadratic => {
							if amm_pcts[&tkn] > 2e-3 {
								force_amm_approx.insert(tkn, AmmApprox::Full);
								approx_adjusted_ct += 1;
							} else if amm_pcts[&tkn] <= 1e-6 {
								force_amm_approx.insert(tkn, AmmApprox::Linear);
								approx_adjusted_ct += 1;
							}
						}
						_ => {
							if amm_pcts[&tkn] <= 1e-6 {
								force_amm_approx.insert(tkn, AmmApprox::Linear);
								approx_adjusted_ct += 1;
							} else if amm_pcts[&tkn] <= 1e-3 {
								force_amm_approx.insert(tkn, AmmApprox::Quadratic);
								approx_adjusted_ct += 1;
							}
						}
					}
				}
			}
		}
	}

	// once solution is found, re-run with directional flags
	if do_directional_run {
		let flags = get_directional_flags(&amm_deltas);
		let params = SetupParams::new()
			.with_flags(flags)
			.with_clear_indicators(false)
			.with_clear_sell_maxes(false)
			.with_clear_amm_approx(false);
		p.set_up_problem(params);
		let (new_amm_deltas, new_intent_deltas, new_x, new_obj, new_dual_obj, new_status) =
			find_solution_unrounded(&p, allow_loss);

		amm_deltas = new_amm_deltas;
		intent_deltas = new_intent_deltas;
		x = new_x;
		obj = new_obj;
		dual_obj = new_dual_obj;
		status = new_status;
	}

	if status == ProblemStatus::PrimalInfeasible || status == ProblemStatus::DualInfeasible {
		return (BTreeMap::new(), vec![0.0; m], vec![], 0.0, 0.0, status);
	}

	let x_unscaled = p.get_real_x(x.iter().cloned().collect());
	(amm_deltas, intent_deltas, x_unscaled, obj, dual_obj, status)
}

fn find_solution_unrounded(
	p: &ICEProblem,
	allow_loss: bool,
) -> (BTreeMap<AssetId, f64>, Vec<f64>, Array2<f64>, f64, f64, ProblemStatus) {
	if p.get_indicators_len() as f64 + p.partial_sell_maxs.iter().sum::<f64>() == 0.0 {
		return (
			p.asset_ids.iter().map(|&tkn| (tkn, 0.0)).collect(),
			vec![0.0; p.partial_indices.len()],
			Array2::zeros((4 * p.n + p.m, 1)),
			0.0,
			0.0,
			ProblemStatus::Solved,
		);
	}

	//let full_intents = &p.full_intents;
	let partial_intents_len = p.partial_indices.len();
	let asset_list = &p.asset_ids;
	let (n, m, r) = (p.n, p.m, p.r);

	if partial_intents_len + p.get_indicators_len() == 0 {
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
		if directions[&tkn] == Direction::Sell || directions[&tkn] == Direction::Neither {
			indices_to_keep.retain(|&i| i != 2 * n + asset_list.iter().position(|&x| x == tkn).unwrap());
		}
		if directions[&tkn] == Direction::Buy || directions[&tkn] == Direction::Neither {
			indices_to_keep.retain(|&i| i != 3 * n + asset_list.iter().position(|&x| x == tkn).unwrap());
		}
		if directions[&tkn] == Direction::Neither {
			indices_to_keep.retain(|&i| i != asset_list.iter().position(|&x| x == tkn).unwrap());
			indices_to_keep.retain(|&i| i != n + asset_list.iter().position(|&x| x == tkn).unwrap());
		}
	}

	let k_real = indices_to_keep.len();
	let P_trimmed = CscMatrix::zeros((k_real, k_real));
	let q_all = ndarray::Array::from(p.get_q());

	let objective_I_coefs = q_all.slice(s![4 * n + m..]);
	let objective_I_coefs = objective_I_coefs.neg();
	let q = q_all.slice(s![..4 * n + m]);
	let q = q.neg();
	let q_trimmed: Vec<f64> = indices_to_keep.iter().map(|&i| q[i]).collect();

	let diff_coefs = Array2::<f64>::zeros((2 * n + m, 2 * n));
	let nonzero_coefs = -Array2::<f64>::eye(2 * n + m);
	let A1 = ndarray::concatenate![Axis(1), diff_coefs, nonzero_coefs];
	let rows_to_keep: Vec<usize> = (0..2 * n + m)
		.filter(|&i| indices_to_keep.contains(&(2 * n + i)))
		.collect();
	let A1_trimmed = A1.select(Axis(0), &rows_to_keep).select(Axis(1), &indices_to_keep);
	let b1 = Array1::<f64>::zeros(A1_trimmed.shape()[0]);
	let cone1 = NonnegativeConeT(A1_trimmed.shape()[0]);

	let amm_coefs = Array2::<f64>::zeros((m, 4 * n));
	let d_coefs = Array2::<f64>::eye(m);
	let A2 = ndarray::concatenate![Axis(1), amm_coefs, d_coefs];
	let b2 = Array1::from(p.get_partial_sell_maxs_scaled());
	let A2_trimmed = A2.select(Axis(1), &indices_to_keep);
	let cone2 = NonnegativeConeT(A2_trimmed.shape()[0]);

	let profit_A = p.get_profit_A();

	let A3 = profit_A.slice(s![.., ..4 * n + m]).to_owned();
	let mut A3 = A3.neg();
	let I_coefs = profit_A.slice(s![.., 4 * n + m..]).to_owned();
	let mut I_coefs = I_coefs.neg();

	if allow_loss {
		let profit_i = p.asset_ids.iter().position(|&x| x == p.tkn_profit).unwrap() + 1;
		A3.remove_index(Axis(0), profit_i);
		I_coefs.remove_index(Axis(0), profit_i);
	}
	let A3_trimmed = A3.select(Axis(1), &indices_to_keep);

	let b3 = if r == 0 {
		Array1::<f64>::zeros(A3_trimmed.shape()[0])
	} else {
		let indicators = if let Some(inds) = p.get_indicators() {
			inds
		} else {
			vec![0; r]
		};
		//TODO: this is trange to convert indicators to f64 - verify if we should use f64 for indicators
		let r_inds: ndarray::Array1<FloatType> = ndarray::Array::from(indicators).iter().map(|v| *v as f64).collect();
		-I_coefs.dot(&r_inds)
	};
	let cone3 = NonnegativeConeT(A3_trimmed.shape()[0]);
	let mut A4 = Array2::<f64>::zeros((0, k));
	let mut b4 = Array1::<f64>::zeros(0);
	let mut cones4 = vec![];
	let epsilon_tkn = p.get_epsilon_tkn();

	for i in 0..n {
		let tkn = asset_list[i];
		let approx = p.get_amm_approx(tkn); //TODO: this initial approx doesn not match
		let approx = if approx == AmmApprox::None && epsilon_tkn[&tkn] <= 1e-6 && tkn != p.tkn_profit {
			AmmApprox::Linear
		} else if approx == AmmApprox::None && epsilon_tkn[&tkn] <= 1e-3 {
			AmmApprox::Quadratic
		} else {
			approx
		};

		let (A4i, b4i, cone) = match approx {
			AmmApprox::Linear => {
				if !directions.contains_key(&tkn) {
					let c1 = 1.0 / (1.0 + epsilon_tkn[&tkn]);
					let c2 = 1.0 / (1.0 - epsilon_tkn[&tkn]);
					let mut A4i = Array2::<f64>::zeros((2, k));
					A4i[[0, i]] = -p.get_amm_lrna_coefs()[&tkn];
					A4i[[0, n + i]] = -p.get_amm_asset_coefs()[&tkn] * c1;
					A4i[[1, i]] = -p.get_amm_lrna_coefs()[&tkn];
					A4i[[1, n + i]] = -p.get_amm_asset_coefs()[&tkn] * c2;
					(A4i, Array1::<f64>::zeros(2), NonnegativeConeT(2))
				} else {
					let c = if directions[&tkn] == Direction::Sell {
						1.0 / (1.0 - epsilon_tkn[&tkn])
					} else {
						1.0 / (1.0 + epsilon_tkn[&tkn])
					};
					let mut A4i = Array2::<f64>::zeros((1, k));
					A4i[[0, i]] = -p.get_amm_lrna_coefs()[&tkn];
					A4i[[0, n + i]] = -p.get_amm_asset_coefs()[&tkn] * c;
					(A4i, Array1::<f64>::zeros(1), ZeroConeT(1))
				}
			}
			AmmApprox::Quadratic => {
				let mut A4i = Array2::<f64>::zeros((3, k));
				A4i[[1, i]] = -p.get_amm_lrna_coefs()[&tkn];
				A4i[[1, n + i]] = -p.get_amm_asset_coefs()[&tkn];
				A4i[[2, n + i]] = -p.get_amm_asset_coefs()[&tkn];
				(A4i, ndarray::array![1.0, 0.0, 0.0], PowerConeT(0.5))
			}
			_ => {
				let mut A4i = Array2::<f64>::zeros((3, k));
				A4i[[0, i]] = -p.get_amm_lrna_coefs()[&tkn];
				A4i[[1, n + i]] = -p.get_amm_asset_coefs()[&tkn];
				(A4i, Array1::<f64>::ones(3), PowerConeT(0.5))
			}
		};

		A4 = ndarray::concatenate![Axis(0), A4, A4i];
		//b4.append(Axis(0),(&b4i).into());
		b4 = ndarray::concatenate![Axis(0), b4, b4i];
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
			A5 = ndarray::concatenate![Axis(0), A5, A5i];
		} else {
			let mut A6i = Array2::<f64>::zeros((2, k));
			let mut A7i = Array2::<f64>::zeros((1, k));
			if directions[&tkn] == Direction::Sell {
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
			A6 = ndarray::concatenate![Axis(0), A6, A6i];
			A7 = ndarray::concatenate![Axis(0), A7, A7i];
		}
	}

	let A5_trimmed = A5.select(Axis(1), &indices_to_keep);
	let A6_trimmed = A6.select(Axis(1), &indices_to_keep);
	let A7_trimmed = A7.select(Axis(1), &indices_to_keep);

	let b5 = Array1::<f64>::zeros(A5.shape()[0]);
	let b6 = Array1::<f64>::zeros(A6.shape()[0]);
	let b7 = Array1::<f64>::zeros(A7.shape()[0]);
	let cone5 = NonnegativeConeT(A5.shape()[0]);
	let cone6 = NonnegativeConeT(A6.shape()[0]);
	let cone7 = ZeroConeT(A7.shape()[0]);

	/*
	let A = ndarray::concatenate![
		Axis(0),
		A1_trimmed,
		A2_trimmed,
		A3_trimmed,
		A4_trimmed,
		A5_trimmed,
		A6_trimmed,
		A7_trimmed
	];

	 */
	// convert a1_trimmed to vec of vec<f64>, note that to_vec does not exist
	let shape = A1_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a1_q = A1_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a1_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A1_trimmed = a_vec;
	let A1_trimmed = CscMatrix::from(&A1_trimmed);

	// convert a2 trimmed
	let shape = A2_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a2_q = A2_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a2_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A2_trimmed = a_vec;
	let A2_trimmed = CscMatrix::from(&A2_trimmed);

	// convert a3 trimmed
	let shape = A3_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a3_q = A3_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a3_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A3_trimmed = a_vec;
	let A3_trimmed = CscMatrix::from(&A3_trimmed);

	// convert a4 trimmed
	let shape = A4_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a4_q = A4_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a4_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A4_trimmed = a_vec;
	let A4_trimmed = CscMatrix::from(&A4_trimmed);

	// Convert a5 trimmed
	let shape = A5_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a5_q = A5_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a5_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A5_trimmed = a_vec;
	let A5_trimmed = CscMatrix::from(&A5_trimmed);

	// Convert a6 trimmed
	let shape = A6_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a6_q = A6_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a6_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A6_trimmed = a_vec;
	let A6_trimmed = CscMatrix::from(&A6_trimmed);

	// Convert a7 trimmed
	let shape = A7_trimmed.shape();
	let mut a_vec = Vec::new();
	for idx in 0..shape[0] {
		let a7_q = A7_trimmed.select(Axis(0), &[idx]);
		let (v, _) = a7_q.into_raw_vec_and_offset();
		a_vec.push(v);
	}
	let A7_trimmed = a_vec;
	let A7_trimmed = CscMatrix::from(&A7_trimmed);

	let A = if A2_trimmed.n != 0 {
		CscMatrix::vcat(&A1_trimmed, &A2_trimmed)
	} else {
		A1_trimmed
	};
	let A = CscMatrix::vcat(&A, &A3_trimmed);
	let A = CscMatrix::vcat(&A, &A4_trimmed);
	//TODO: in some cases it results in A5 with shape 0,0 - so can we just excklude it ?
	let A = if A5_trimmed.n != 0 {
		CscMatrix::vcat(&A, &A5_trimmed)
	} else {
		A
	};
	let A = if A6_trimmed.n != 0 {
		CscMatrix::vcat(&A, &A6_trimmed)
	} else {
		A
	};
	let A = if A7_trimmed.n != 0 {
		CscMatrix::vcat(&A, &A7_trimmed)
	} else {
		A
	};
	let b = ndarray::concatenate![Axis(0), b1, b2, b3, b4, b5, b6, b7];
	let cones = vec![cone1, cone2, cone3]
		.into_iter()
		.chain(cones4.into_iter())
		.chain(vec![cone5, cone6, cone7].into_iter())
		.collect::<Vec<_>>();

	let settings = DefaultSettingsBuilder::default().verbose(false).build().unwrap();
	let mut solver = DefaultSolver::new(&P_trimmed, &q_trimmed, &A, &b.to_vec(), &cones, settings);
	solver.solve();
	let x = solver.solution.x;
	let status = solver.solution.status;
	let solve_time = solver.solution.solve_time;
	let obj_value = solver.solution.obj_val;
	let obj_value_dual = solver.solution.obj_val_dual;
	//println!("status: {:?}", status);
	//println!("time: {:?}", solve_time);

	let mut new_amm_deltas = BTreeMap::new();
	let mut exec_intent_deltas = vec![0.0; partial_intents_len];
	let mut x_expanded = vec![0.0; k];
	for (i, &index) in indices_to_keep.iter().enumerate() {
		x_expanded[index] = x[i];
	}
	let x_scaled = p.get_real_x(x_expanded.clone());
	for i in 0..n {
		let tkn = asset_list[i];
		new_amm_deltas.insert(tkn, x_scaled[n + i]);
	}
	for j in 0..partial_intents_len {
		exec_intent_deltas[j] = -x_scaled[4 * n + j];
	}

	let obj_offset = if let Some(I) = p.get_indicators() {
		let v = I.iter().map(|&x| x as f64).collect::<Vec<_>>();
		objective_I_coefs.to_vec().dot(&v)
	} else {
		0.0
	};
	(
		new_amm_deltas,
		exec_intent_deltas,
		Array2::from_shape_vec((k, 1), x_expanded).unwrap(),
		p.scale_obj_amt(obj_value + obj_offset),
		p.scale_obj_amt(obj_value_dual + obj_offset),
		status.into(),
	)
}

fn scale_down_partial_intents(p: &ICEProblem, trade_pcts: &[f64], scale: f64) -> (Option<Vec<f64>>, usize) {
	let mut zero_ct = 0;
	let mut intent_sell_maxs = p.partial_sell_maxs.clone();

	for (i, &m) in p.partial_sell_maxs.iter().enumerate() {
		let old_sell_quantity = m * trade_pcts[i];
		let mut new_sell_max = m / scale;

		if old_sell_quantity < new_sell_max {
			let partial_intent_idx = p.partial_indices[i];
			let intent = p.intents[partial_intent_idx].clone();
			let tkn = intent.asset_in;
			let sell_amt_lrna_value = new_sell_max * p.get_asset_pool_data(tkn).hub_price;

			if sell_amt_lrna_value < p.min_partial {
				new_sell_max = 0.0;
				zero_ct += 1;
			}
			intent_sell_maxs[i] = new_sell_max;
		}
	}

	(Some(intent_sell_maxs), zero_ct)
}

fn get_directional_flags(amm_deltas: &BTreeMap<AssetId, f64>) -> BTreeMap<AssetId, i8> {
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
