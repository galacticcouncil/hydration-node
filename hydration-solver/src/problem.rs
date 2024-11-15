use crate::data::AssetData;
use crate::to_f64_by_decimals;
use clarabel::algebra::CscMatrix;
use clarabel::solver::SolverStatus;
use ndarray::{Array1, Array2};
use pallet_ice::types::{Intent, IntentId};
use primitives::{AccountId, AssetId};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

pub type FloatType = f64;
pub const FLOAT_INF: FloatType = FloatType::INFINITY;

#[derive(PartialEq, Eq)]
pub enum ProblemStatus {
	NotSolved,
	Solved,
	PrimalInfeasible,
	DualInfeasible,
}

impl From<SolverStatus> for ProblemStatus {
	fn from(value: SolverStatus) -> Self {
		match value {
			SolverStatus::Solved => ProblemStatus::Solved,
			SolverStatus::AlmostSolved => ProblemStatus::Solved,
			SolverStatus::PrimalInfeasible => ProblemStatus::PrimalInfeasible,
			SolverStatus::DualInfeasible => ProblemStatus::DualInfeasible,
			SolverStatus::Unsolved => ProblemStatus::NotSolved,
			_ => panic!("Unexpected solver status"),
		}
	}
}

#[derive(Clone)]
pub struct ICEProblem {
	pub tkn_profit: AssetId,
	pub intent_ids: Vec<IntentId>,
	pub intents: Vec<Intent<AccountId, AssetId>>,
	pub intent_amounts: Vec<(FloatType, FloatType)>,

	pub pool_data: BTreeMap<AssetId, AssetData>,

	pub n: usize, // number of assets in intents
	pub m: usize, // number of partial intents
	pub r: usize, // number of full intents

	pub min_partial: FloatType,

	pub indicators: Vec<usize>,

	pub asset_ids: Vec<AssetId>,
	pub partial_sell_maxs: Vec<FloatType>,
	pub initial_sell_maxs: Vec<FloatType>,
	pub partial_indices: Vec<usize>,
	pub full_indices: Vec<usize>,

	pub directional_flags: Option<BTreeMap<AssetId, i8>>,
	pub force_amm_approx: Option<BTreeMap<AssetId, AmmApprox>>,

	pub step_params: Option<StepParams>,
	pub fee_match: FloatType,
}

impl ICEProblem {
	pub(crate) fn get_partial_intent_prices(&self) -> Vec<FloatType> {
		todo!()
	}
}

impl ICEProblem {
	pub(crate) fn get_partial_intents_amounts(&self) -> Vec<(FloatType, FloatType)> {
		self.partial_indices
			.iter()
			.map(|&idx| self.intent_amounts[idx])
			.collect()
	}

	pub(crate) fn get_full_intents_amounts(&self) -> Vec<(FloatType, FloatType)> {
		self.full_indices.iter().map(|&idx| self.intent_amounts[idx]).collect()
	}

	pub(crate) fn get_amm_approx(&self, asset_id: AssetId) -> AmmApprox {
		*self.force_amm_approx.as_ref().unwrap().get(&asset_id).unwrap()
	}

	pub(crate) fn scale_obj_amt(&self, amt: FloatType) -> FloatType {
		let scaling = self.get_scaling();
		amt * scaling[&self.tkn_profit]
	}

	pub(crate) fn get_epsilon_tkn(&self) -> BTreeMap<AssetId, FloatType> {
		//python: return {t: max([abs(self._max_in[t]), abs(self._max_out[t])]) / self.omnipool.liquidity[t] for t in self.asset_list}
		let mut r = BTreeMap::new();
		for asset_id in self.asset_ids.iter() {
			let max_in = self.get_max_in()[&asset_id];
			let max_out = self.get_max_out()[&asset_id];
			let liquidity = self.get_asset_pool_data(*asset_id).reserve;
			let epsilon = max_in.abs().max(max_out.abs()) / liquidity;
			r.insert(*asset_id, epsilon);
		}
		r
	}
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum Direction {
	Sell,
	Buy,
	Both,
	Neither,
}

impl ICEProblem {
	pub(crate) fn get_omnipool_directions(&self) -> BTreeMap<AssetId, Direction> {
		self.step_params
			.as_ref()
			.unwrap()
			.omnipool_directions
			.as_ref()
			.unwrap()
			.clone()
	}
}

impl ICEProblem {
	pub fn get_real_x(&self, x: Vec<FloatType>) -> Vec<FloatType> {
		let n = self.n;
		let m = self.m;
		let r = self.r;
		assert!(x.len() == 4 * n + m || x.len() == 4 * n + m + r);

		let scaling = self.get_scaling();
		let real_yi: Vec<FloatType> = (0..n).map(|i| x[i] * scaling[&1u32]).collect(); // Assuming 1u32 represents 'LRNA'
		let real_xi: Vec<FloatType> = self
			.asset_ids
			.iter()
			.enumerate()
			.map(|(i, &tkn)| x[n + i] * scaling[&tkn])
			.collect();
		let real_lrna_lambda: Vec<FloatType> = (0..n).map(|i| x[2 * n + i] * scaling[&1u32]).collect();
		let real_lambda: Vec<FloatType> = self
			.asset_ids
			.iter()
			.enumerate()
			.map(|(i, &tkn)| x[3 * n + i] * scaling[&tkn])
			.collect();
		let real_d: Vec<FloatType> = self
			.partial_indices
			.iter()
			.enumerate()
			.map(|(j, &idx)| x[4 * n + j] * scaling[&self.intents[idx].swap.asset_in])
			.collect();

		let mut real_x = [real_yi, real_xi, real_lrna_lambda, real_lambda, real_d].concat();
		if x.len() == 4 * n + m + r {
			let real_I: Vec<FloatType> = (0..r).map(|l| x[4 * n + m + l]).collect();
			real_x.extend(real_I);
		}
		real_x
	}
}

impl ICEProblem {
	pub(crate) fn get_q(&self) -> Vec<FloatType> {
		todo!()
	}
}

impl ICEProblem {
	pub(crate) fn get_profit_A(&self) -> Array2<FloatType> {
		todo!()
	}
}

impl ICEProblem {
	pub(crate) fn get_amm_asset_coefs(&self) -> &BTreeMap<AssetId, FloatType> {
		todo!()
	}
}

impl ICEProblem {
	pub(crate) fn get_amm_lrna_coefs(&self) -> &BTreeMap<AssetId, FloatType> {
		todo!()
	}
}

impl ICEProblem {
	pub fn get_scaled_x(&self, x: Vec<FloatType>) -> Vec<FloatType> {
		let n = self.n;
		let m = self.m;
		let r = self.r;
		assert!(x.len() == 4 * n + m || x.len() == 4 * n + m + r);

		let scaling = self.get_scaling();
		let scaled_yi: Vec<FloatType> = (0..n).map(|i| x[i] / scaling[&1u32]).collect(); // Assuming 1u32 represents 'LRNA'
		let scaled_xi: Vec<FloatType> = self
			.asset_ids
			.iter()
			.enumerate()
			.map(|(i, &tkn)| x[n + i] / scaling[&tkn])
			.collect();
		let scaled_lrna_lambda: Vec<FloatType> = (0..n).map(|i| x[2 * n + i] / scaling[&1u32]).collect();
		let scaled_lambda: Vec<FloatType> = self
			.asset_ids
			.iter()
			.enumerate()
			.map(|(i, &tkn)| x[3 * n + i] / scaling[&tkn])
			.collect();
		let scaled_d: Vec<FloatType> = self
			.partial_indices
			.iter()
			.enumerate()
			.map(|(j, &idx)| x[4 * n + j] / scaling[&self.intents[idx].swap.asset_in])
			.collect();

		let mut scaled_x = [scaled_yi, scaled_xi, scaled_lrna_lambda, scaled_lambda, scaled_d].concat();
		if x.len() == 4 * n + m + r {
			let scaled_I: Vec<FloatType> = (0..r).map(|l| x[4 * n + m + l]).collect();
			scaled_x.extend(scaled_I);
		}
		scaled_x
	}
}

#[derive(Clone)]
pub struct SetupParams {
	pub indicators: Option<Vec<usize>>,
	pub flags: Option<BTreeMap<AssetId, i8>>,
	pub sell_maxes: Option<Vec<FloatType>>,
	pub force_amm_approx: Option<BTreeMap<AssetId, AmmApprox>>,
	pub rescale: bool,
	pub clear_sell_maxes: bool,
	pub clear_indicators: bool,
	pub clear_amm_approx: bool,
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum AmmApprox {
	Linear,
	Quadratic,
	Full,
	None,
}

impl SetupParams {
	pub fn new() -> Self {
		SetupParams {
			indicators: None,
			flags: None,
			sell_maxes: None,
			force_amm_approx: None,
			rescale: true,
			clear_sell_maxes: true,
			clear_indicators: true,
			clear_amm_approx: true,
		}
	}
	pub fn with_indicators(mut self, indicators: Vec<usize>) -> Self {
		self.indicators = Some(indicators);
		self
	}
	pub fn with_flags(mut self, flags: BTreeMap<AssetId, i8>) -> Self {
		self.flags = Some(flags);
		self
	}
	pub fn with_sell_maxes(mut self, sell_maxes: Vec<FloatType>) -> Self {
		self.sell_maxes = Some(sell_maxes);
		self
	}
	pub fn with_force_amm_approx(mut self, force_amm_approx: BTreeMap<AssetId, AmmApprox>) -> Self {
		self.force_amm_approx = Some(force_amm_approx);
		self
	}
	pub fn with_rescale(mut self, rescale: bool) -> Self {
		self.rescale = rescale;
		self
	}
	pub fn with_clear_sell_maxes(mut self, clear_sell_maxes: bool) -> Self {
		self.clear_sell_maxes = clear_sell_maxes;
		self
	}
	pub fn with_clear_indicators(mut self, clear_indicators: bool) -> Self {
		self.clear_indicators = clear_indicators;
		self
	}
	pub fn with_clear_amm_approx(mut self, clear_amm_approx: bool) -> Self {
		self.clear_amm_approx = clear_amm_approx;
		self
	}
}

impl ICEProblem {
	pub fn new(
		intents_and_ids: Vec<(IntentId, Intent<AccountId, AssetId>)>,
		pool_data: BTreeMap<AssetId, AssetData>,
	) -> Self {
		let mut intents = Vec::with_capacity(intents_and_ids.len());
		let mut intent_ids = Vec::with_capacity(intents_and_ids.len());
		let mut intent_amounts = Vec::with_capacity(intents_and_ids.len());
		let mut partial_sell_amounts = Vec::new();
		let mut partial_indices = Vec::new();
		let mut full_indices = Vec::new();
		let mut asset_ids = BTreeSet::new();

		let asset_profit = 0u32.into(); //HDX
		asset_ids.insert(asset_profit);

		for (idx, (intent_id, intent)) in intents_and_ids.iter().enumerate() {
			intent_ids.push(*intent_id);

			let amount_in = to_f64_by_decimals!(
				intent.swap.amount_in,
				pool_data.get(&intent.swap.asset_in).unwrap().decimals
			);
			let amount_out = to_f64_by_decimals!(
				intent.swap.amount_out,
				pool_data.get(&intent.swap.asset_out).unwrap().decimals
			);

			intent_amounts.push((amount_in, amount_out));

			if intent.partial {
				partial_indices.push(idx);
				partial_sell_amounts.push(amount_in);
			} else {
				full_indices.push(idx);
			}
			if intent.swap.asset_in != 1u32 {
				asset_ids.insert(intent.swap.asset_in);
			}
			if intent.swap.asset_out != 1u32 {
				//note: this should never happened, as it is not allowed to buy lrna!
				asset_ids.insert(intent.swap.asset_out);
			} else {
				debug_assert!(false, "It is not allowed to buy lrna!");
			}
		}

		let n = asset_ids.len();
		let m = partial_indices.len();
		let r = full_indices.len();

		// this comes from the initial solution which we skipped,
		// so we intened to resolve all full intents
		let indicators = vec![1usize; r];

		let initial_sell_maxs = partial_sell_amounts.clone();

		ICEProblem {
			tkn_profit: 0u32, // HDX
			intent_ids,
			intents,
			intent_amounts,
			pool_data,
			min_partial: 1.,
			n,
			m,
			r,
			indicators,
			asset_ids: asset_ids.into_iter().collect(),
			partial_sell_maxs: partial_sell_amounts,
			initial_sell_maxs,
			partial_indices,
			full_indices,
			directional_flags: None,
			force_amm_approx: None,
			step_params: None,
			fee_match: 0.0005,
		}
	}

	pub(crate) fn get_indicators(&self) -> Vec<usize> {
		self.indicators.clone()
	}

	pub(crate) fn get_asset_pool_data(&self, asset_id: AssetId) -> &AssetData {
		self.pool_data.get(&asset_id).unwrap()
	}

	pub(crate) fn set_up_problem(&mut self, params: SetupParams) {
		if let Some(new_indicators) = params.indicators {
			debug_assert_eq!(new_indicators.len(), self.r);
			self.indicators = new_indicators;
		} else if params.clear_indicators {
			self.indicators = vec![1usize; self.r]; //reest to original
		}
		if let Some(new_maxes) = params.sell_maxes {
			self.partial_sell_maxs = new_maxes;
		} else if params.clear_sell_maxes {
			self.partial_sell_maxs = self.initial_sell_maxs.clone();
		}
		if let Some(new_flags) = params.flags {
			self.directional_flags = Some(new_flags);
		} else {
			self.directional_flags = None;
		}
		if let Some(new_force_amm_approx) = params.force_amm_approx {
			self.force_amm_approx = Some(new_force_amm_approx);
		} else if params.clear_amm_approx {
			self.force_amm_approx = None;
		}
		self.recalculate(params.rescale)
	}

	fn recalculate(&mut self, rescale: bool) {
		let mut step_params = StepParams::default();
		step_params.set_known_flow(self);
		step_params.set_max_in_out(self);
		step_params.set_bounds(self);
		if rescale {
			step_params.set_scaling(self);
			step_params.set_amm_coefs(self);
		}
		step_params.set_omnipool_directions(self);
		step_params.set_tau_phi(self);
		step_params.set_coefficients(self);
		self.step_params = Some(step_params);
	}

	pub(crate) fn get_intent(&self, idx: usize) -> &Intent<AccountId, AssetId> {
		&self.intents[idx]
	}

	pub(crate) fn get_scaling(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.as_ref().unwrap().scaling.as_ref().unwrap()
	}

	pub(crate) fn get_max_in(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.as_ref().unwrap().max_in.as_ref().unwrap()
	}

	pub(crate) fn get_max_out(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.as_ref().unwrap().max_out.as_ref().unwrap()
	}

	pub(crate) fn get_partial_sell_maxs_scaled(&self) -> Vec<FloatType> {
		let mut partial_sell_maxes = self.partial_sell_maxs.clone();
		for (j, &idx) in self.partial_indices.iter().enumerate() {
			let intent = &self.intents[idx];
			let tkn = intent.swap.asset_in;
			if tkn != 1u32 {
				let liquidity = self.pool_data.get(&tkn).unwrap().reserve;
				partial_sell_maxes[j] = partial_sell_maxes[j].min(liquidity / 2.0);
			}
		}
		let scaling = self.get_scaling();
		partial_sell_maxes
			.iter()
			.enumerate()
			.map(|(j, &max)| max / scaling[&self.intents[self.partial_indices[j]].swap.asset_in])
			.collect()
	}

	pub fn get_scaled_bounds(
		&self,
	) -> (
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
		ndarray::Array1<FloatType>,
	) {
		todo!()
	}
}

#[derive(Default, Clone)]
pub struct StepParams {
	pub known_flow: Option<BTreeMap<AssetId, (FloatType, FloatType)>>,
	pub max_in: Option<BTreeMap<AssetId, FloatType>>,
	pub max_out: Option<BTreeMap<AssetId, FloatType>>,
	pub min_in: Option<BTreeMap<AssetId, FloatType>>,
	pub min_out: Option<BTreeMap<AssetId, FloatType>>,
	pub scaling: Option<BTreeMap<AssetId, FloatType>>,
	pub omnipool_directions: Option<BTreeMap<AssetId, Direction>>,
	pub tau: Option<Vec<FloatType>>,
	pub phi: Option<Vec<FloatType>>,
	pub q: Option<Vec<FloatType>>,
}

impl StepParams {
	fn set_known_flow(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_max_in_out(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_bounds(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_scaling(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_amm_coefs(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_tau_phi(&mut self, problem: &ICEProblem) {
		todo!()
	}
}

impl StepParams {
	pub fn set_omnipool_directions(&mut self, problem: &ICEProblem) {
		let mut known_intent_directions = BTreeMap::new();
		known_intent_directions.insert(problem.tkn_profit, Direction::Both);

		for (j, &idx) in problem.partial_indices.iter().enumerate() {
			let intent = &problem.intents[idx];
			if problem.partial_sell_maxs[j] > 0.0 {
				let tkn_sell = intent.swap.asset_in;
				let tkn_buy = intent.swap.asset_out;

				match known_intent_directions.entry(tkn_sell) {
					Entry::Vacant(e) => {
						e.insert(Direction::Sell);
					}
					Entry::Occupied(mut e) => {
						if *e.get() == Direction::Buy {
							e.insert(Direction::Both);
						}
					}
				}

				match known_intent_directions.entry(tkn_buy) {
					Entry::Vacant(e) => {
						e.insert(Direction::Buy);
					}
					Entry::Occupied(mut e) => {
						if *e.get() == Direction::Sell {
							e.insert(Direction::Both);
						}
					}
				}
			}
		}

		for &tkn in problem.asset_ids.iter() {
			let known_flow = problem.step_params.as_ref().unwrap().known_flow.as_ref().unwrap();
			let flow_in = known_flow[&tkn].0;
			let flow_out = known_flow[&tkn].1;

			if flow_in > flow_out {
				match known_intent_directions.entry(tkn) {
					Entry::Vacant(e) => {
						e.insert(Direction::Sell);
					}
					Entry::Occupied(mut e) => {
						if *e.get() == Direction::Buy {
							e.insert(Direction::Both);
						}
					}
				}
			} else if flow_in < flow_out {
				match known_intent_directions.entry(tkn) {
					Entry::Vacant(e) => {
						e.insert(Direction::Buy);
					}
					Entry::Occupied(mut e) => {
						if *e.get() == Direction::Sell {
							e.insert(Direction::Both);
						}
					}
				}
			} else if flow_in > 0.0 {
				match known_intent_directions.entry(tkn) {
					Entry::Vacant(e) => {
						e.insert(Direction::Buy);
					}
					Entry::Occupied(mut e) => {
						if *e.get() == Direction::Sell {
							e.insert(Direction::Both);
						}
					}
				}
			}
		}

		let mut omnipool_directions = BTreeMap::new();
		for &tkn in problem.asset_ids.iter() {
			if let Some(&flag) = problem.directional_flags.as_ref().unwrap().get(&tkn) {
				match flag {
					-1 => {
						omnipool_directions.insert(tkn, Direction::Sell);
					}
					1 => {
						omnipool_directions.insert(tkn, Direction::Buy);
					}
					0 => {
						omnipool_directions.insert(tkn, Direction::Neither);
					}
					_ => {}
				}
			} else if let Some(&direction) = known_intent_directions.get(&tkn) {
				match direction {
					Direction::Sell => {
						omnipool_directions.insert(tkn, Direction::Buy);
					}
					Direction::Buy => {
						omnipool_directions.insert(tkn, Direction::Sell);
					}
					_ => {}
				}
			} else {
				omnipool_directions.insert(tkn, Direction::Neither);
			}
		}

		self.omnipool_directions = Some(omnipool_directions);
	}
}

impl StepParams {
	pub fn set_coefficients(&mut self, problem: &ICEProblem) {
		// profit calculations
		let n = problem.n;
		let m = problem.m;
		let r = problem.r;

		// y_i are net LRNA into Omnipool
		let profit_lrna_y_coefs = vec![-1.0; n];
		// x_i are net assets into Omnipool
		let profit_lrna_x_coefs = vec![0.0; n];
		// lrna_lambda_i are LRNA amounts coming out of Omnipool
		let profit_lrna_lrna_lambda_coefs: Vec<FloatType> = problem
			.asset_ids
			.iter()
			.map(|&tkn| -problem.get_asset_pool_data(tkn).protocol_fee)
			.collect();
		let profit_lrna_lambda_coefs = vec![0.0; n];
		let profit_lrna_d_coefs: Vec<FloatType> = self.tau.as_ref().unwrap()[..m].to_vec();

		let sell_amts: Vec<FloatType> = problem
			.full_indices
			.iter()
			.map(|&idx| problem.intent_amounts[idx].0)
			.collect();
		let profit_lrna_I_coefs: Vec<FloatType> = self.tau.as_ref().unwrap()[m..]
			.to_vec()
			.iter()
			.zip(sell_amts.iter())
			.map(|(&tau, &sell_amt)| tau * sell_amt / problem.get_scaling()[&1u32])
			.collect();
		let mut profit_lrna_coefs = vec![];
		profit_lrna_coefs.extend(profit_lrna_y_coefs);
		profit_lrna_coefs.extend(profit_lrna_x_coefs);
		profit_lrna_coefs.extend(profit_lrna_lrna_lambda_coefs);
		profit_lrna_coefs.extend(profit_lrna_lambda_coefs);
		profit_lrna_coefs.extend(profit_lrna_d_coefs);
		profit_lrna_coefs.extend(profit_lrna_I_coefs);

		// leftover must be higher than required fees
		let fees: Vec<FloatType> = problem
			.asset_ids
			.iter()
			.map(|&tkn| problem.get_asset_pool_data(tkn).fee)
			.collect();
		let partial_intent_prices: Vec<FloatType> = problem.get_partial_intent_prices();
		let profit_y_coefs = vec![vec![0.0; n]; n];
		let profit_x_coefs = -Array2::<FloatType>::eye(n);
		let profit_lrna_lambda_coefs = vec![vec![0.0; n]; n];
		let profit_lambda_coefs = -Array2::<FloatType>::from_diag(&Array1::from(
			fees.iter().map(|&fee| fee - problem.fee_match).collect::<Vec<_>>(),
		));
		let scaling_vars: Vec<FloatType> = problem
			.partial_indices
			.iter()
			.enumerate()
			.map(|(j, &idx)| {
				let intent = &problem.intents[idx];
				partial_intent_prices[j] * problem.get_scaling()[&intent.swap.asset_in]
					/ problem.get_scaling()[&intent.swap.asset_out]
			})
			.collect();
		let vars_scaled = scaling_vars
			.iter()
			.map(|&v| v * 1.0 / (1.0 - problem.fee_match))
			.collect::<Vec<_>>();
		let scaled_phi = self.phi.as_ref().unwrap()[1..m]
			.to_owned()
			.iter()
			.zip(vars_scaled.iter())
			.map(|(phi, &var)| phi * var)
			.collect::<Vec<_>>();
		let profit_d_coefs = (self.tau.as_ref().unwrap()[1..m]
			.to_owned()
			.iter()
			.zip(scaled_phi.iter())
			.map(|(tau, &phi)| tau * phi))
		.collect::<Vec<_>>();

		let buy_amts: Vec<FloatType> = problem
			.full_indices
			.iter()
			.map(|&idx| problem.intent_amounts[idx].1)
			.collect();
		let buy_scaled = buy_amts
			.iter()
			.map(|&v| v * 1.0 / (1.0 - problem.fee_match))
			.collect::<Vec<_>>();
		let scaled_phi = self.phi.as_ref().unwrap()[m..]
			.to_owned()
			.iter()
			.zip(buy_scaled.iter())
			.map(|(&phi, &buy)| phi * buy)
			.collect::<Vec<_>>();
		let scaled_tau: Vec<FloatType> = self.tau.as_ref().unwrap()[m..]
			.to_owned()
			.iter()
			.zip(sell_amts.iter())
			.map(|(tau, &sell)| tau * sell)
			.collect();

		let scaled_tau = ndarray::Array::from(scaled_tau);
		let scaled_phi = ndarray::Array::from(scaled_phi);
		let unscaled_diff = scaled_tau - scaled_phi;
		let scalars: Vec<FloatType> = problem
			.asset_ids
			.iter()
			.map(|&tkn| problem.get_scaling()[&tkn])
			.collect();
		let I_coefs = (unscaled_diff / Array2::from_diag(&Array1::from(scalars))).to_owned();

		//TODO: this pls
		/*
		let profit_A_LRNA = Array2::from_shape_vec((1, profit_lrna_coefs.len()), profit_lrna_coefs).unwrap();
		let profit_A_assets = Array2::from_shape_vec((n, n * 6), vec![]).unwrap()
			.hstack(&profit_y_coefs)
			.hstack(&profit_x_coefs)
			.hstack(&profit_lrna_lambda_coefs)
			.hstack(&profit_lambda_coefs)
			.hstack(&profit_d_coefs)
			.hstack(&I_coefs);

		self.profit_A = Some(profit_A_LRNA.vstack(&profit_A_assets));

		let profit_i = problem.asset_ids.iter().position(|&tkn| tkn == problem.tkn_profit).unwrap();
		self.q = Some(self.profit_A.as_ref().unwrap().row(profit_i + 1).to_vec());
		 */
	}
}
