use pallet_ice::types::{Balance, ResolvedIntent, TradeInstruction};
use sp_std::vec::Vec;

pub trait IceSolution<AssetId> {
	fn resolved_intents(&self) -> Vec<ResolvedIntent>;
	fn trades(self) -> Vec<TradeInstruction<AssetId>>;
	fn score(&self) -> u64;
}
