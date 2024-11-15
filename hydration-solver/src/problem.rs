use crate::data::AssetData;
use crate::to_f64_by_decimals;
use clarabel::algebra::CscMatrix;
use ndarray::{Array1, Array2};
use pallet_ice::traits::OmnipoolAssetInfo;
use pallet_ice::types::{Intent, IntentId};
use primitives::{AccountId, AssetId, Balance};
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
}

impl ICEProblem {
	pub(crate) fn get_q(&self) -> Array2<FloatType> {
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

pub enum AmmApprox {
	Linear,
	Quadratic,
	Full,
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

#[derive(Default)]
pub struct StepParams {
	pub known_flow: Option<BTreeMap<AssetId, FloatType>>,
	pub max_in: Option<BTreeMap<AssetId, FloatType>>,
	pub max_out: Option<BTreeMap<AssetId, FloatType>>,
	pub min_in: Option<BTreeMap<AssetId, FloatType>>,
	pub min_out: Option<BTreeMap<AssetId, FloatType>>,
	pub scaling: Option<BTreeMap<AssetId, FloatType>>,
	pub tau: Option<CscMatrix>,
	pub phi: Option<CscMatrix>,
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
	fn set_omnipool_directions(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_tau_phi(&mut self, problem: &ICEProblem) {
		todo!()
	}
	fn set_coefficients(&mut self, problem: &ICEProblem) {
		todo!()
	}
}
