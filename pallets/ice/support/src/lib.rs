#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{ConstU32, RuntimeDebug, TypeInfo};
use frame_support::sp_runtime::traits::CheckedConversion;
use frame_support::BoundedVec;
use hydra_dx_math::types::Ratio;
use hydradx_traits::router::Route;
use sp_core::U256;

pub type AssetId = u32;
pub type Balance = u128;
pub type IntentId = u128;
pub type Score = u128;

pub type PoolId = AssetId;
pub type Price = Ratio;

pub const MAX_NUMBER_OF_RESOLVED_INTENTS: u32 = 100;
pub const MAX_NUMBER_OF_SOLUTION_TRADES: u32 = 200;
pub const MAX_NUMBER_OF_CLEARING_PRICES: u32 = MAX_NUMBER_OF_SOLUTION_TRADES * 2;

pub type ResolvedIntents = BoundedVec<ResolvedIntent, ConstU32<MAX_NUMBER_OF_RESOLVED_INTENTS>>;
pub type SolutionTrades = BoundedVec<PoolTrade, ConstU32<MAX_NUMBER_OF_SOLUTION_TRADES>>;
pub type ClearingPrices = BoundedVec<(AssetId, Price), ConstU32<MAX_NUMBER_OF_CLEARING_PRICES>>;

pub type ResolvedIntent = Intent;

#[derive(Clone, Debug, Encode, Decode, TypeInfo, Eq, PartialEq)]
pub struct Intent {
	pub id: IntentId,
	pub data: IntentData,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum IntentData {
	Swap(SwapData),
}

impl IntentData {
	pub fn is_partial(&self) -> bool {
		match &self {
			IntentData::Swap(s) => s.partial,
		}
	}

	pub fn asset_in(&self) -> AssetId {
		match &self {
			IntentData::Swap(s) => s.asset_in,
		}
	}

	pub fn asset_out(&self) -> AssetId {
		match &self {
			IntentData::Swap(s) => s.asset_out,
		}
	}

	pub fn amount_in(&self) -> Balance {
		match &self {
			IntentData::Swap(s) => s.amount_in,
		}
	}

	pub fn amount_out(&self) -> Balance {
		match &self {
			IntentData::Swap(s) => s.amount_out,
		}
	}

	/// Function calculates surplus amount from `resolved` intent.
	///
	/// Surplus must be >= zero
	pub fn surplus(&self, resolve: &IntentData) -> Option<Balance> {
		match &self {
			IntentData::Swap(s) => match s.swap_type {
				SwapType::ExactIn => {
					let amt = if s.partial {
						self.pro_rata(resolve)?
					} else {
						s.amount_out
					};

					resolve.amount_out().checked_sub(amt)
				}
				SwapType::ExactOut => {
					let amt = if s.partial {
						self.pro_rata(resolve)?
					} else {
						s.amount_in
					};

					amt.checked_sub(resolve.amount_in())
				}
			},
		}
	}

	// Function calculates pro rata amount based on `resolved` intent.
	pub fn pro_rata(&self, resolve: &IntentData) -> Option<Balance> {
		match &self {
			IntentData::Swap(s) => match s.swap_type {
				SwapType::ExactIn => U256::from(resolve.amount_in())
					.checked_mul(U256::from(s.amount_out))?
					.checked_div(U256::from(s.amount_in))?
					.checked_into(),

				SwapType::ExactOut => U256::from(resolve.amount_out())
					.checked_mul(U256::from(s.amount_in))?
					.checked_div(U256::from(s.amount_out))?
					.checked_into(),
			},
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct SwapData {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub swap_type: SwapType,
	pub partial: bool,
}

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum SwapType {
	ExactIn,
	ExactOut,
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct Solution {
	pub resolved_intents: ResolvedIntents,
	pub trades: SolutionTrades,
	pub clearing_prices: ClearingPrices,
	pub score: Score,
}

#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct PoolTrade {
	/// Direction of trade (sell or buy)
	pub direction: SwapType,
	/// Amount of asset sold
	pub amount_in: Balance,
	/// Amount of asset bought
	pub amount_out: Balance,
	/// Type of pool used for this transaction
	pub route: Route<AssetId>,
}
