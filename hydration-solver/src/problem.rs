use crate::data::AssetData;
use crate::to_f64_by_decimals;
use crate::types::{AssetId, Intent, IntentId};
use clarabel::algebra::{BlockConcatenate, CscMatrix};
use clarabel::solver::SolverStatus;
use float_next_after::NextAfter;
use ndarray::{s, Array1, Array2, Array3, ArrayBase, Axis, Ix1, Ix2, OwnedRepr};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

pub type FloatType = f64;
pub const FLOAT_INF: FloatType = FloatType::INFINITY;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ProblemStatus {
	NotSolved,
	Solved,
	PrimalInfeasible,
	DualInfeasible,
	InsufficientProgress,
}

impl From<SolverStatus> for ProblemStatus {
	fn from(value: SolverStatus) -> Self {
		match value {
			SolverStatus::Solved => ProblemStatus::Solved,
			SolverStatus::AlmostSolved => ProblemStatus::Solved,
			SolverStatus::PrimalInfeasible => ProblemStatus::PrimalInfeasible,
			SolverStatus::DualInfeasible => ProblemStatus::DualInfeasible,
			SolverStatus::Unsolved => ProblemStatus::NotSolved,
			SolverStatus::InsufficientProgress => ProblemStatus::InsufficientProgress,
			_ => panic!("Unexpected solver status {:?}", value),
		}
	}
}

#[derive(Clone)]
pub struct ICEProblem {
	pub tkn_profit: AssetId,
	pub intent_ids: Vec<IntentId>,
	pub intents: Vec<Intent>,
	pub intent_amounts: Vec<(FloatType, FloatType)>,

	pub pool_data: BTreeMap<AssetId, AssetData>,

	pub n: usize, // number of assets in intents
	pub m: usize, // number of partial intents
	pub r: usize, // number of full intents

	pub min_partial: FloatType,

	pub indicators: Option<Vec<usize>>,

	pub asset_ids: Vec<AssetId>,
	pub partial_sell_maxs: Vec<FloatType>,
	pub initial_sell_maxs: Vec<FloatType>,
	pub partial_indices: Vec<usize>,
	pub full_indices: Vec<usize>,

	pub directional_flags: Option<BTreeMap<AssetId, i8>>,
	pub force_amm_approx: Option<BTreeMap<AssetId, AmmApprox>>,

	pub step_params: StepParams,
	pub fee_match: FloatType,
}

impl ICEProblem {
	pub fn new(intents_and_ids: Vec<Intent>, pool_data: BTreeMap<AssetId, AssetData>) -> Self {
		let mut intents = Vec::with_capacity(intents_and_ids.len());
		let mut intent_ids = Vec::with_capacity(intents_and_ids.len());
		let mut intent_amounts = Vec::with_capacity(intents_and_ids.len());
		let mut partial_sell_amounts = Vec::new();
		let mut partial_indices = Vec::new();
		let mut full_indices = Vec::new();
		let mut asset_ids = BTreeSet::new();

		let asset_profit = 0u32.into(); //HDX
		asset_ids.insert(asset_profit);

		for (idx, intent) in intents_and_ids.iter().enumerate() {
			intent_ids.push(intent.intent_id);
			intents.push(intent.clone());

			let amount_in = to_f64_by_decimals!(intent.amount_in, pool_data.get(&intent.asset_in).unwrap().decimals);
			let amount_out = to_f64_by_decimals!(intent.amount_out, pool_data.get(&intent.asset_out).unwrap().decimals);

			intent_amounts.push((amount_in, amount_out));

			if intent.partial {
				partial_indices.push(idx);
				partial_sell_amounts.push(amount_in);
			} else {
				full_indices.push(idx);
			}
			if intent.asset_in != 1u32 {
				asset_ids.insert(intent.asset_in);
			}
			if intent.asset_out != 1u32 {
				//note: this should never happened, as it is not allowed to buy lrna!
				asset_ids.insert(intent.asset_out);
			} else {
				debug_assert!(false, "It is not allowed to buy lrna!");
			}
		}

		let n = asset_ids.len();
		let m = partial_indices.len();
		let r = full_indices.len();

		// this comes from the initial solution which we skipped,
		// so we intened to resolve all full intents
		//TODO: this should take input as init indicators and if set, do something - check python
		let indicators = None;

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
			step_params: StepParams::default(),
			fee_match: 0.0005,
		}
	}

	pub(crate) fn get_partial_intent_prices(&self) -> Vec<FloatType> {
		let mut prices = Vec::new();
		for &idx in self.partial_indices.iter() {
			let (amount_in, amount_out) = self.intent_amounts[idx];
			let price = amount_out / amount_in; //TODO: division by zero?!!
			prices.push(price);
		}
		prices
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
		if let Some(approx) = self.force_amm_approx.as_ref() {
			*approx.get(&asset_id).unwrap_or(&AmmApprox::None)
		} else {
			AmmApprox::None
		}
	}

	pub(crate) fn scale_obj_amt(&self, amt: FloatType) -> FloatType {
		let scaling = self.get_scaling();
		amt * scaling[&self.tkn_profit]
	}

	pub(crate) fn get_epsilon_tkn(&self) -> BTreeMap<AssetId, FloatType> {
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

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Direction {
	Sell,
	Buy,
	Both,
	Neither,
}

impl ICEProblem {
	pub(crate) fn get_omnipool_directions(&self) -> BTreeMap<AssetId, Direction> {
		self.step_params.omnipool_directions.as_ref().unwrap().clone()
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
			.map(|(j, &idx)| x[4 * n + j] * scaling[&self.intents[idx].asset_in])
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
		self.step_params.q.as_ref().cloned().unwrap()
	}
}

impl ICEProblem {
	pub(crate) fn get_profit_A(&self) -> Array2<FloatType> {
		self.step_params.profit_a.as_ref().cloned().unwrap()
	}
}

impl ICEProblem {
	pub(crate) fn get_amm_asset_coefs(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.amm_asset_coefs.as_ref().unwrap()
	}
}

impl ICEProblem {
	pub(crate) fn get_amm_lrna_coefs(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.amm_lrna_coefs.as_ref().unwrap()
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
			.map(|(j, &idx)| x[4 * n + j] / scaling[&self.intents[idx].asset_in])
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

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
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
	pub(crate) fn get_indicators(&self) -> Option<Vec<usize>> {
		self.indicators.as_ref().cloned()
	}
	pub(crate) fn get_indicators_len(&self) -> usize {
		if let Some(inds) = self.indicators.as_ref() {
			inds.iter().sum()
		} else {
			0
		}
	}

	pub(crate) fn get_asset_pool_data(&self, asset_id: AssetId) -> &AssetData {
		self.pool_data.get(&asset_id).unwrap()
	}

	pub(crate) fn price(&self, asset_a: AssetId, asset_b: AssetId) -> FloatType {
		let da = self.get_asset_pool_data(asset_a);
		let db = self.get_asset_pool_data(asset_b);
		if asset_a == asset_b {
			1.0
		} else if asset_b == 1u32 {
			da.hub_price
		} else if asset_a == 1u32 {
			1. / db.hub_price
		} else {
			let da_hub_reserve = da.hub_reserve;
			let da_reserve = da.reserve;
			let db_hub_reserve = db.hub_reserve;
			let db_reserve = db.reserve;
			da_hub_reserve / da_reserve / db_hub_reserve * db_reserve
		}
	}

	pub(crate) fn set_up_problem(&mut self, params: SetupParams) {
		if let Some(new_indicators) = params.indicators {
			debug_assert_eq!(new_indicators.len(), self.r);
			self.indicators = Some(new_indicators);
		} else if params.clear_indicators {
			self.indicators = None;
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
		self.recalculate(params.rescale);
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
		self.step_params = step_params;
	}

	pub(crate) fn get_intent(&self, idx: usize) -> &Intent {
		&self.intents[idx]
	}

	pub(crate) fn get_scaling(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.scaling.as_ref().unwrap()
	}

	pub(crate) fn get_max_in(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.max_in.as_ref().unwrap()
	}

	pub(crate) fn get_max_out(&self) -> &BTreeMap<AssetId, FloatType> {
		self.step_params.max_out.as_ref().unwrap()
	}

	pub(crate) fn get_partial_sell_maxs_scaled(&self) -> Vec<FloatType> {
		let mut partial_sell_maxes = self.partial_sell_maxs.clone();
		for (j, &idx) in self.partial_indices.iter().enumerate() {
			let intent = &self.intents[idx];
			let tkn = intent.asset_in;
			if tkn != 1u32 {
				let liquidity = self.pool_data.get(&tkn).unwrap().reserve;
				partial_sell_maxes[j] = partial_sell_maxes[j].min(liquidity / 2.0);
			}
		}
		let scaling = self.get_scaling();
		partial_sell_maxes
			.iter()
			.enumerate()
			.map(|(j, &max)| max / scaling[&self.intents[self.partial_indices[j]].asset_in])
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
		let scaling = self.get_scaling();
		let lrna_scaling = scaling[&1u32.into()]; // Assuming 1u32 represents 'LRNA'

		let min_y = self.step_params.min_y.as_ref().unwrap();
		let max_y = self.step_params.max_y.as_ref().unwrap();
		let min_x = self.step_params.min_x.as_ref().unwrap();
		let max_x = self.step_params.max_x.as_ref().unwrap();
		let min_lrna_lambda = self.step_params.min_lrna_lambda.as_ref().unwrap();
		let max_lrna_lambda = self.step_params.max_lrna_lambda.as_ref().unwrap();
		let min_lambda = self.step_params.min_lambda.as_ref().unwrap();
		let max_lambda = self.step_params.max_lambda.as_ref().unwrap();

		let scaled_min_y = ndarray::Array1::from(min_y.iter().map(|&val| val / lrna_scaling).collect::<Vec<_>>());
		let scaled_max_y = ndarray::Array1::from(max_y.iter().map(|&val| val / lrna_scaling).collect::<Vec<_>>());
		let scaled_min_x = ndarray::Array1::from(
			self.asset_ids
				.iter()
				.enumerate()
				.map(|(i, &tkn)| min_x[i] / scaling[&tkn])
				.collect::<Vec<_>>(),
		);
		let scaled_max_x = ndarray::Array1::from(
			self.asset_ids
				.iter()
				.enumerate()
				.map(|(i, &tkn)| max_x[i] / scaling[&tkn])
				.collect::<Vec<_>>(),
		);
		let scaled_min_lrna_lambda = ndarray::Array1::from(
			min_lrna_lambda
				.iter()
				.map(|&val| val / lrna_scaling)
				.collect::<Vec<_>>(),
		);
		let scaled_max_lrna_lambda = ndarray::Array1::from(
			max_lrna_lambda
				.iter()
				.map(|&val| val / lrna_scaling)
				.collect::<Vec<_>>(),
		);
		let scaled_min_lambda = ndarray::Array1::from(
			self.asset_ids
				.iter()
				.enumerate()
				.map(|(i, &tkn)| min_lambda[i] / scaling[&tkn])
				.collect::<Vec<_>>(),
		);
		let scaled_max_lambda = ndarray::Array1::from(
			self.asset_ids
				.iter()
				.enumerate()
				.map(|(i, &tkn)| max_lambda[i] / scaling[&tkn])
				.collect::<Vec<_>>(),
		);

		(
			scaled_min_y,
			scaled_max_y,
			scaled_min_x,
			scaled_max_x,
			scaled_min_lrna_lambda,
			scaled_max_lrna_lambda,
			scaled_min_lambda,
			scaled_max_lambda,
		)
	}
}

#[derive(Default, Clone, Debug)]
pub struct StepParams {
	pub known_flow: Option<BTreeMap<AssetId, (FloatType, FloatType)>>,
	pub max_in: Option<BTreeMap<AssetId, FloatType>>,
	pub max_out: Option<BTreeMap<AssetId, FloatType>>,
	pub min_in: Option<BTreeMap<AssetId, FloatType>>,
	pub min_out: Option<BTreeMap<AssetId, FloatType>>,
	pub scaling: Option<BTreeMap<AssetId, FloatType>>,
	pub omnipool_directions: Option<BTreeMap<AssetId, Direction>>,
	pub tau: Option<Array2<FloatType>>,
	pub phi: Option<Array2<FloatType>>,
	pub q: Option<Vec<FloatType>>,
	pub profit_a: Option<Array2<FloatType>>,
	min_x: Option<Vec<FloatType>>,
	max_x: Option<Vec<FloatType>>,
	min_lambda: Option<Vec<FloatType>>,
	max_lambda: Option<Vec<FloatType>>,
	min_y: Option<Vec<FloatType>>,
	max_y: Option<Vec<FloatType>>,
	min_lrna_lambda: Option<Vec<FloatType>>,
	max_lrna_lambda: Option<Vec<FloatType>>,
	amm_lrna_coefs: Option<BTreeMap<AssetId, FloatType>>,
	amm_asset_coefs: Option<BTreeMap<AssetId, FloatType>>,
}

impl StepParams {
	fn set_known_flow(&mut self, problem: &ICEProblem) {
		let mut known_flow: BTreeMap<AssetId, (FloatType, FloatType)> = BTreeMap::new();

		// Initialize known_flow with zero values for all assets
		for &asset_id in problem.asset_ids.iter() {
			known_flow.insert(asset_id, (0.0, 0.0));
		}

		// Add LRNA to known_flow
		known_flow.insert(1u32.into(), (0.0, 0.0)); // Assuming 1u32 represents 'LRNA'

		// Update known_flow based on full intents
		if let Some(I) = &problem.get_indicators() {
			assert_eq!(I.len(), problem.full_indices.len());
			for (i, &idx) in problem.full_indices.iter().enumerate() {
				if I[i] as f64 > 0.5 {
					let intent = &problem.intents[idx];
					let (sell_quantity, buy_quantity) = problem.intent_amounts[idx];
					let tkn_sell = intent.asset_in;
					let tkn_buy = intent.asset_out;

					let entry = known_flow.entry(tkn_sell).or_insert((0.0, 0.0));
					entry.0 = entry.0 + sell_quantity;

					let entry = known_flow.entry(tkn_buy).or_insert((0.0, 0.0));
					entry.1 = entry.1 + buy_quantity;
				}
			}
		}

		self.known_flow = Some(known_flow);
	}
	fn set_max_in_out(&mut self, problem: &ICEProblem) {
		let mut max_in: BTreeMap<AssetId, FloatType> = BTreeMap::new();
		let mut max_out: BTreeMap<AssetId, FloatType> = BTreeMap::new();
		let mut min_in: BTreeMap<AssetId, FloatType> = BTreeMap::new();
		let mut min_out: BTreeMap<AssetId, FloatType> = BTreeMap::new();

		for &asset_id in problem.asset_ids.iter() {
			max_in.insert(asset_id, 0.0);
			max_out.insert(asset_id, 0.0);
			min_in.insert(asset_id, 0.0);
			min_out.insert(asset_id, 0.0);
		}

		max_in.insert(1u32.into(), 0.0); // Assuming 1u32 represents 'LRNA'
		max_out.insert(1u32.into(), 0.0);
		min_in.insert(1u32.into(), 0.0);
		min_out.insert(1u32.into(), 0.0);

		for (i, &idx) in problem.partial_indices.iter().enumerate() {
			let intent = &problem.intents[idx];
			let (amount_in, amount_out) = problem.intent_amounts[idx];
			let tkn_sell = intent.asset_in;
			let tkn_buy = intent.asset_out;
			let sell_quantity = problem.partial_sell_maxs[i];
			let buy_quantity = amount_out / amount_in * sell_quantity;

			*max_in.get_mut(&tkn_sell).unwrap() += sell_quantity;
			*max_out.get_mut(&tkn_buy).unwrap() += if buy_quantity != 0.0 {
				buy_quantity.next_after(FloatType::INFINITY)
			} else {
				0.0
			};
		}

		if problem.get_indicators().is_none() {
			for &idx in problem.full_indices.iter() {
				let intent = &problem.intents[idx];
				let (sell_quantity, buy_quantity) = problem.intent_amounts[idx];
				let tkn_sell = intent.asset_in;
				let tkn_buy = intent.asset_out;

				*max_in.get_mut(&tkn_sell).unwrap() += sell_quantity;
				*max_out.get_mut(&tkn_buy).unwrap() += buy_quantity;
			}
		}

		for (&tkn, &(in_flow, out_flow)) in self.known_flow.as_ref().unwrap().iter() {
			*max_in.get_mut(&tkn).unwrap() += in_flow - out_flow;
			*min_in.get_mut(&tkn).unwrap() += in_flow - out_flow;
			*max_out.get_mut(&tkn).unwrap() -= in_flow - out_flow;
			*min_out.get_mut(&tkn).unwrap() -= in_flow - out_flow;
		}

		let fees: BTreeMap<AssetId, FloatType> = problem
			.asset_ids
			.iter()
			.map(|&tkn| (tkn, problem.get_asset_pool_data(tkn).fee))
			.collect();

		for &tkn in problem.asset_ids.iter() {
			*max_in.get_mut(&tkn).unwrap() = max_in[&tkn].max(0.0);
			*min_in.get_mut(&tkn).unwrap() = min_in[&tkn].max(0.0);
			*max_out.get_mut(&tkn).unwrap() = (max_out[&tkn] / (1.0 - fees[&tkn])).max(0.0);
			*min_out.get_mut(&tkn).unwrap() = (min_out[&tkn] / (1.0 - fees[&tkn])).max(0.0);
		}

		*max_out.get_mut(&1u32.into()).unwrap() = 0.0; // Assuming 1u32 represents 'LRNA'
		*min_out.get_mut(&1u32.into()).unwrap() = 0.0;
		*max_in.get_mut(&1u32.into()).unwrap() = max_in[&1u32.into()].max(0.0);
		*min_in.get_mut(&1u32.into()).unwrap() = min_in[&1u32.into()].max(0.0);

		self.max_in = Some(max_in);
		self.max_out = Some(max_out);
		self.min_in = Some(min_in);
		self.min_out = Some(min_out);
	}
	fn set_bounds(&mut self, problem: &ICEProblem) {
		let n = problem.asset_ids.len();
		let mut min_x = vec![0.0; n];
		let mut max_x = vec![0.0; n];
		let mut min_lambda = vec![0.0; n];
		let mut max_lambda = vec![0.0; n];
		let mut min_y = vec![0.0; n];
		let mut max_y = vec![0.0; n];
		let mut min_lrna_lambda = vec![0.0; n];
		let mut max_lrna_lambda = vec![0.0; n];

		for (i, &tkn) in problem.asset_ids.iter().enumerate() {
			min_x[i] = self.min_in.as_ref().unwrap()[&tkn] - self.max_out.as_ref().unwrap()[&tkn];
			max_x[i] = self.max_in.as_ref().unwrap()[&tkn] - self.min_out.as_ref().unwrap()[&tkn];
			min_lambda[i] = (-max_x[i]).max(0.0);
			max_lambda[i] = (-min_x[i]).max(0.0);

			let omnipool_data = problem.get_asset_pool_data(tkn);
			let min_y_val = -omnipool_data.hub_reserve * max_x[i] / (max_x[i] + omnipool_data.reserve);
			min_y[i] = min_y_val - 0.1 * min_y_val.abs();
			let max_y_val = -omnipool_data.hub_reserve * min_x[i] / (min_x[i] + omnipool_data.reserve);
			max_y[i] = max_y_val + 0.1 * max_y_val.abs();
			min_lrna_lambda[i] = (-max_y[i]).max(0.0);
			max_lrna_lambda[i] = (-min_y[i]).max(0.0);
		}

		let profit_i = problem
			.asset_ids
			.iter()
			.position(|&tkn| tkn == problem.tkn_profit)
			.unwrap();
		let profit_tkn_data = problem.get_asset_pool_data(problem.tkn_profit);
		min_x[profit_i] = -profit_tkn_data.reserve;
		max_lambda[profit_i] = (-min_x[profit_i]).max(0.0);
		min_y[profit_i] = -profit_tkn_data.hub_reserve;
		max_lrna_lambda[profit_i] = (-min_y[profit_i]).max(0.0);

		self.min_x = Some(min_x);
		self.max_x = Some(max_x);
		self.min_lambda = Some(min_lambda);
		self.max_lambda = Some(max_lambda);
		self.min_y = Some(min_y);
		self.max_y = Some(max_y);
		self.min_lrna_lambda = Some(min_lrna_lambda);
		self.max_lrna_lambda = Some(max_lrna_lambda);
	}
	fn set_scaling(&mut self, problem: &ICEProblem) {
		let mut scaling: BTreeMap<AssetId, FloatType> = BTreeMap::new();

		// Initialize scaling with zero values for all assets
		for &asset_id in problem.asset_ids.iter() {
			scaling.insert(asset_id, 0.0);
		}

		// Initialize scaling for LRNA
		scaling.insert(1u32.into(), 0.0); // Assuming 1u32 represents 'LRNA'

		for &tkn in problem.asset_ids.iter() {
			let max_in = self.max_in.as_ref().unwrap()[&tkn];
			let max_out = self.max_out.as_ref().unwrap()[&tkn];
			scaling.insert(tkn, max_in.max(max_out));

			if scaling[&tkn] == 0.0 && tkn != problem.tkn_profit {
				scaling.insert(tkn, 1.0);
			}

			// Set scaling for LRNA equal to scaling for asset, adjusted by spot price
			let omnipool_data = problem.get_asset_pool_data(tkn);
			let price = problem.price(tkn, problem.tkn_profit);
			let scalar = scaling[&tkn] * omnipool_data.hub_reserve / omnipool_data.reserve;
			scaling.insert(1u32.into(), scaling[&1u32.into()].max(scalar));

			// Raise scaling for tkn_profit to scaling for asset, adjusted by spot price, if needed
			let scalar_profit = scaling[&tkn] * problem.price(tkn, problem.tkn_profit);
			scaling.insert(problem.tkn_profit, scaling[&problem.tkn_profit].max(scalar_profit));
		}

		self.scaling = Some(scaling);
	}
	fn set_amm_coefs(&mut self, problem: &ICEProblem) {
		let mut amm_lrna_coefs: BTreeMap<AssetId, FloatType> = BTreeMap::new();
		let mut amm_asset_coefs: BTreeMap<AssetId, FloatType> = BTreeMap::new();

		let scaling = self.scaling.as_ref().unwrap();
		for &tkn in problem.asset_ids.iter() {
			let omnipool_data = problem.get_asset_pool_data(tkn);
			amm_lrna_coefs.insert(tkn, scaling[&1u32.into()] / omnipool_data.hub_reserve); // Assuming 1u32 represents 'LRNA'
			amm_asset_coefs.insert(tkn, scaling[&tkn] / omnipool_data.reserve);
		}

		self.amm_lrna_coefs = Some(amm_lrna_coefs);
		self.amm_asset_coefs = Some(amm_asset_coefs);
	}
}

impl StepParams {
	pub fn set_omnipool_directions(&mut self, problem: &ICEProblem) {
		let mut known_intent_directions = BTreeMap::new();
		known_intent_directions.insert(problem.tkn_profit, Direction::Both);

		for (j, &idx) in problem.partial_indices.iter().enumerate() {
			let intent = &problem.intents[idx];
			if problem.partial_sell_maxs[j] > 0.0 {
				let tkn_sell = intent.asset_in;
				let tkn_buy = intent.asset_out;

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
			let known_flow = self.known_flow.as_ref().unwrap();
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
		let directions = if let Some(d) = problem.directional_flags.as_ref() {
			d.clone()
		} else {
			BTreeMap::new()
		};
		for &tkn in problem.asset_ids.iter() {
			if let Some(&flag) = directions.get(&tkn) {
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

type A1T = ArrayBase<OwnedRepr<FloatType>, Ix1>;

impl StepParams {
	fn set_tau_phi(&mut self, problem: &ICEProblem) {
		let n = problem.asset_ids.len();
		let m = problem.partial_indices.len();
		let r = problem.full_indices.len();

		let mut tau1 = ndarray::Array2::zeros((n + 1, m + r));
		let mut phi1 = ndarray::Array2::zeros((n + 1, m + r));
		//let mut tau2 = ndarray::Array2::zeros((n + 1, r));
		//let mut phi2 = ndarray::Array2::zeros((n + 1, r));

		let mut tkn_list = vec![1u32];
		tkn_list.extend(problem.asset_ids.iter().cloned());

		for (j, &idx) in problem.partial_indices.iter().enumerate() {
			let intent = &problem.intents[idx];
			let tkn_sell = intent.asset_in;
			let tkn_buy = intent.asset_out;
			let tkn_sell_idx = tkn_list.iter().position(|&tkn| tkn == tkn_sell).unwrap();
			let tkn_buy_idx = tkn_list.iter().position(|&tkn| tkn == tkn_buy).unwrap();
			tau1[(tkn_sell_idx, j)] = 1.;
			phi1[(tkn_buy_idx, j)] = 1.;
			//tau1.set_entry((tkn_sell_idx, j), 1.);
			//phi1.set_entry((tkn_buy_idx, j), 1.);
		}
		for (l, &idx) in problem.full_indices.iter().enumerate() {
			let intent = &problem.intents[idx];
			let tkn_sell = intent.asset_in;
			let tkn_buy = intent.asset_out;
			let tkn_sell_idx = tkn_list.iter().position(|&tkn| tkn == tkn_sell).unwrap();
			let tkn_buy_idx = tkn_list.iter().position(|&tkn| tkn == tkn_buy).unwrap();
			tau1[(tkn_sell_idx, l + m)] = 1.;
			phi1[(tkn_buy_idx, l + m)] = 1.;
			//tau2[(tkn_sell_idx, l)] = 1.;
			//phi2[(tkn_buy_idx, l)] = 1.;
			//tau2.set_entry((tkn_sell_idx, l), 1.);
			//phi2.set_entry((tkn_buy_idx, l), 1.);
		}

		self.tau = Some(tau1);
		self.phi = Some(phi1);
	}

	pub fn set_coefficients(&mut self, problem: &ICEProblem) {
		// profit calculations
		let n = problem.n;
		let m = problem.m;
		let r = problem.r;

		// y_i are net LRNA into Omnipool
		let profit_lrna_y_coefs: ArrayBase<OwnedRepr<FloatType>, Ix1> = -ndarray::Array1::ones(n);
		// x_i are net assets into Omnipool
		let profit_lrna_x_coefs: ArrayBase<OwnedRepr<FloatType>, Ix1> = ndarray::Array1::zeros(n);
		// lrna_lambda_i are LRNA amounts coming out of Omnipool
		let profit_lrna_lrna_lambda_coefs: A1T = ndarray::Array::from(
			problem
				.asset_ids
				.iter()
				.map(|&tkn| -problem.get_asset_pool_data(tkn).protocol_fee)
				.collect::<Vec<_>>(),
		);

		let profit_lrna_lambda_coefs: A1T = ndarray::Array1::zeros(n);

		let lrna_d_coefs = self.tau.as_ref().unwrap().row(0).clone().to_vec();
		let profit_lrna_d_coefs = ndarray::Array::from(lrna_d_coefs[..m].to_vec());

		let sell_amts: Vec<FloatType> = problem
			.full_indices
			.iter()
			.map(|&idx| problem.intent_amounts[idx].0)
			.collect();
		let profit_lrna_I_coefs: Vec<FloatType> = lrna_d_coefs[m..]
			.to_vec()
			.iter()
			.zip(sell_amts.iter())
			.map(|(&tau, &sell_amt)| tau * sell_amt / self.scaling.as_ref().unwrap()[&1u32])
			.collect(); //TODO: set scaling sets initial value to 0 for lrna;;verify if not division by zero

		/*
		let profit_lrna_coefs = ndarray::concatenate![
			Axis(0),
			profit_lrna_y_coefs,
			profit_lrna_x_coefs,
			profit_lrna_lrna_lambda_coefs,
			profit_lrna_lambda_coefs,
			profit_lrna_d_coefs,
			profit_lrna_I_coefs
		];
		 */
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
		let profit_y_coefs = ndarray::Array2::zeros((n, n));
		let profit_x_coefs = -Array2::<FloatType>::eye(n);
		let profit_lrna_lambda_coefs = ndarray::Array2::zeros((n, n));
		let profit_lambda_coefs = -Array2::<FloatType>::from_diag(&Array1::from(
			fees.iter().map(|&fee| fee - problem.fee_match).collect::<Vec<_>>(),
		));
		let scaling = self.scaling.as_ref().unwrap();
		let scaling_vars: Vec<FloatType> = problem
			.partial_indices
			.iter()
			.enumerate()
			.map(|(j, &idx)| {
				let intent = &problem.intents[idx];
				partial_intent_prices[j] * scaling[&intent.asset_in] / scaling[&intent.asset_out]
			})
			.collect();

		let vars_scaled = scaling_vars
			.iter()
			.map(|&v| v * 1.0 / (1.0 - problem.fee_match))
			.collect::<Vec<_>>();

		let phi = self.phi.as_ref().unwrap();
		let tau = self.tau.as_ref().unwrap();
		let profit_d_coefs = if m != 0 {
			//TODO: this was originally multiplying by Array2::from_diags() - verify
			let scaled_phi = phi.slice(s![1.., ..m]).to_owned() * &Array1::from(vars_scaled.clone());
			tau.slice(s![1.., ..m]).to_owned() - scaled_phi
		} else {
			// empty
			Array2::zeros((n, m))
		};

		let buy_amts: Vec<FloatType> = problem
			.full_indices
			.iter()
			.map(|&idx| problem.intent_amounts[idx].1)
			.collect();
		let buy_scaled = buy_amts
			.iter()
			.map(|&v| v * 1.0 / (1.0 - problem.fee_match))
			.collect::<Vec<_>>();

		let phi = self.phi.as_ref().unwrap();
		let scaled_phi = phi.slice(s![1.., m..]).to_owned() * &Array1::from(buy_scaled.clone());
		let scaled_tau = tau.slice(s![1.., m..]).to_owned() * &Array1::from(sell_amts.clone());
		let unscaled_diff = scaled_tau - scaled_phi;
		let scalars: Vec<FloatType> = problem.asset_ids.iter().map(|&tkn| scaling[&tkn]).collect();
		let un_size = unscaled_diff.shape()[0];
		let scalars = Array2::from_shape_vec((un_size, 1), scalars).unwrap();
		let i_coefs = unscaled_diff / scalars;

		let l = profit_lrna_coefs.len();
		let profit_A_LRNA = Array2::from_shape_vec((1, l), profit_lrna_coefs).unwrap();
		let profit_A_assets = ndarray::concatenate![
			Axis(1),
			profit_y_coefs,
			profit_x_coefs,
			profit_lrna_lambda_coefs,
			profit_lambda_coefs,
			profit_d_coefs,
			i_coefs,
		];

		let profit_A = Some(ndarray::concatenate![Axis(0), profit_A_LRNA, profit_A_assets]);
		self.profit_a = profit_A.clone();

		let profit_tkn_idx = problem
			.asset_ids
			.iter()
			.position(|&tkn| tkn == problem.tkn_profit)
			.unwrap();
		self.q = Some(profit_A.as_ref().unwrap().row(profit_tkn_idx + 1).to_vec());
	}
}
