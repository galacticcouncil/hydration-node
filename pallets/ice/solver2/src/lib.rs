#![cfg_attr(not(feature = "std"), no_std)]
extern crate core;

use crate::traits::{ICESolver, IceSolution};
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_ice::types::{Balance, BoundedRoute, Intent, IntentId, ResolvedIntent, TradeInstruction};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::Saturating;
use sp_std::collections::btree_map::BTreeMap;

pub mod cvx;
pub mod cvx2;
pub mod omni;
#[cfg(test)]
mod tests;
pub mod traits;

#[derive(Debug, PartialEq)]
pub struct SolverSolution<AssetId> {
	pub intents: Vec<ResolvedIntent>,
	pub trades: Vec<TradeInstruction<AssetId>>,
	pub score: u64,
}

impl<AssetId> IceSolution<AssetId> for SolverSolution<AssetId> {
	fn resolved_intents(&self) -> Vec<ResolvedIntent> {
		self.intents.clone()
	}

	fn trades(self) -> Vec<TradeInstruction<AssetId>> {
		self.trades
	}

	fn score(&self) -> u64 {
		self.score
	}
}