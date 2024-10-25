use hydra_dx_math::ratio::Ratio;
use hydradx_traits::router::Trade;
use pallet_ice::types::{Balance, ResolvedIntent, TradeInstruction};
use serde::Deserialize;
use sp_runtime::traits::Bounded;
use sp_runtime::{FixedU128, Permill};
use sp_std::vec::Vec;

pub trait IceSolution<AssetId> {
	fn resolved_intents(&self) -> Vec<ResolvedIntent>;
	fn trades(self) -> Vec<TradeInstruction<AssetId>>;
	fn score(&self) -> u64;
}
