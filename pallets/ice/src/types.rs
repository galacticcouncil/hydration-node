use codec::{Decode, Encode};
use frame_support::pallet_prelude::TypeInfo;
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::router::Route;
use pallet_intent::types::AssetId;
use pallet_intent::types::Intent;
use pallet_intent::types::IntentId;
use sp_std::vec::Vec;

pub type Balance = u128;

#[derive(Debug, Clone, PartialEq, Encode, Decode, TypeInfo)]
pub enum TradeType {
	Buy,
	Sell,
}

#[derive(Debug, Clone, PartialEq, Encode, Decode, TypeInfo)]
pub struct Trade {
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub trade_type: TradeType,
	pub route: Route<AssetId>,
}

//TODO: change vec for boundedVec
#[derive(Debug, Clone, PartialEq, Encode, Decode, TypeInfo)]
pub struct Solution {
	pub resolved: Vec<(IntentId, Intent, Trade)>,
	pub clearing_prices: Vec<(AssetId, Ratio)>,
}

#[derive(Encode, Decode)]
pub struct SolverData {
	intents: Vec<Intent>,
}
