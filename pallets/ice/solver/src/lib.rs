#![cfg_attr(not(feature = "std"), no_std)]

use crate::traits::IceSolution;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_ice::types::{Balance, BoundedRoute, Intent, IntentId, ResolvedIntent, TradeInstruction};
use sp_runtime::Saturating;
use sp_std::vec::Vec;

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

#[macro_export]
macro_rules! rational_to_f64 {
	($x:expr, $y:expr) => {
		FixedU128::from_rational($x, $y).to_float()
	};
}
#[macro_export]
macro_rules! to_f64_by_decimals {
	($x:expr, $y:expr) => {
		FixedU128::from_rational($x, 10u128.pow($y as u32)).to_float()
	};
}
