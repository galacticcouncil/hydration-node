use std::collections::{BTreeMap, BTreeSet};
use clarabel::algebra::CscMatrix;
use pallet_ice::traits::OmnipoolAssetInfo;
use pallet_ice::types::{Intent, IntentId};
use primitives::{AccountId, AssetId, Balance};

pub type FloatType = f64;
pub const FLOAT_INF: FloatType = FloatType::INFINITY;

pub enum ProblemStatus {
    NotSolved,
    Solved,
}

pub struct ICEProblem{
    pub intent_ids: Vec<IntentId>,
    pub intents: Vec<Intent<AccountId, AssetId>>,
    pub omnipool_data: Vec<OmnipoolAssetInfo<AssetId>>,

    pub n: usize, // number of assets in intents
    pub m: usize, // number of partial intents
    pub r: usize, // number of full intents

    pub asset_ids: Vec<AssetId>,
    pub partial_sell_amounts: Vec<Balance>,
    pub partial_indices: Vec<usize>,
    pub full_indices: Vec<usize>,
}

pub struct Params {
    pub scaling: BTreeMap<AssetId, FloatType>,
    pub tau: CscMatrix,
    pub phi: CscMatrix,
}

impl ICEProblem{
    pub fn new(intents_and_ids: Vec<(IntentId, Intent<AccountId, AssetId>)>, omnipool_data: Vec<OmnipoolAssetInfo<AssetId>>) -> Self{

        let mut intents= Vec::with_capacity(intents_and_ids.len());
        let mut intent_ids = Vec::with_capacity(intents_and_ids.len());
        let mut partial_sell_amounts = Vec::new();
        let mut partial_indices = Vec::new();
        let mut full_indices = Vec::new();
        let mut asset_ids = BTreeSet::new();

        let asset_profit = 0u32.into(); //HDX
        asset_ids.insert(asset_profit);

        for (idx, (intent_id, intent)) in intents_and_ids.iter().enumerate(){
            intent_ids.push(*intent_id);
            if intent.partial {
                partial_indices.push(idx);
                partial_sell_amounts.push(intent.swap.amount_in);
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

        ICEProblem{
            intent_ids,
            intents,
            omnipool_data,
            n,
            m,
            r,
            asset_ids: asset_ids.into_iter().collect(),
            partial_sell_amounts,
            partial_indices,
            full_indices,
        }
    }
}
