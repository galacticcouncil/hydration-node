#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{ConstU32, RuntimeDebug, TypeInfo};
use frame_support::sp_runtime::traits::CheckedConversion;
use frame_support::sp_runtime::{DispatchError, Permill};
use frame_support::BoundedVec;
use hydra_dx_math::types::Ratio;
use hydradx_traits::router::Route;
use sp_core::U256;

pub type AssetId = u32;
pub type Balance = u128;
pub type BlockNumber = u32;
pub type IntentId = u128;
pub type Score = u128;

pub type PoolId = AssetId;
pub type Price = Ratio;

pub const MAX_NUMBER_OF_RESOLVED_INTENTS: u32 = 100;
pub const MAX_NUMBER_OF_SOLUTION_TRADES: u32 = 200;

pub type ResolvedIntents = BoundedVec<ResolvedIntent, ConstU32<MAX_NUMBER_OF_RESOLVED_INTENTS>>;
pub type SolutionTrades = BoundedVec<PoolTrade, ConstU32<MAX_NUMBER_OF_SOLUTION_TRADES>>;

pub type ResolvedIntent = Intent;

#[derive(Clone, DecodeWithMemTracking, Debug, Encode, Decode, TypeInfo, Eq, PartialEq)]
pub struct Intent {
	pub id: IntentId,
	pub data: IntentData,
}

/// Active solution producer, selected on chain.
///
/// `submit_solution` stays the single entry point for every solver; the mode is
/// the lens through which the chain validates and executes what it receives.
/// Under `Passthrough` the claimed `Solution` amounts and `score` are advisory —
/// the binding limits are re-derived from stored intent state at execution.
#[derive(
	Clone, Copy, Default, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo,
)]
pub enum SolverMode {
	#[default]
	V4,
	/// Emergency drain: every intent executed independently as a plain router trade.
	Passthrough,
	/// Kill switch — no solution is accepted.
	Disabled,
}

#[derive(Clone, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum IntentData {
	Swap(SwapData),
	Dca(DcaData),
}

/// User-facing intent data for extrinsic submission.
/// Uses SwapParams/DcaParams instead of SwapData/DcaData to avoid exposing internal state.
#[derive(Clone, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum IntentDataInput {
	Swap(SwapParams),
	Dca(DcaParams),
}

impl IntentDataInput {
	pub fn asset_in(&self) -> AssetId {
		match self {
			IntentDataInput::Swap(s) => s.asset_in,
			IntentDataInput::Dca(d) => d.asset_in,
		}
	}

	pub fn asset_out(&self) -> AssetId {
		match self {
			IntentDataInput::Swap(s) => s.asset_out,
			IntentDataInput::Dca(d) => d.asset_out,
		}
	}
}

impl IntentData {
	pub fn is_partial(&self) -> bool {
		match self {
			IntentData::Swap(s) => s.partial.is_partial(),
			IntentData::Dca(_) => false,
		}
	}

	pub fn asset_in(&self) -> AssetId {
		match self {
			IntentData::Swap(s) => s.asset_in,
			IntentData::Dca(d) => d.asset_in,
		}
	}

	pub fn asset_out(&self) -> AssetId {
		match self {
			IntentData::Swap(s) => s.asset_out,
			IntentData::Dca(d) => d.asset_out,
		}
	}

	pub fn amount_in(&self) -> Balance {
		match self {
			IntentData::Swap(s) => s.amount_in,
			IntentData::Dca(d) => d.amount_in,
		}
	}

	pub fn amount_out(&self) -> Balance {
		match self {
			IntentData::Swap(s) => s.amount_out,
			IntentData::Dca(d) => d.amount_out,
		}
	}

	/// Function calculates surplus amount from `resolved` intent.
	///
	/// Surplus must be >= zero
	pub fn surplus(&self, resolve: &IntentData) -> Option<Balance> {
		match self {
			IntentData::Swap(s) => {
				let amt = if s.partial.is_partial() {
					self.pro_rata(resolve)?
				} else {
					s.amount_out
				};
				resolve.amount_out().checked_sub(amt)
			}
			IntentData::Dca(d) => resolve.amount_out().checked_sub(d.amount_out),
		}
	}

	// Function calculates pro rata amount based on `resolved` intent.
	pub fn pro_rata(&self, resolve: &IntentData) -> Option<Balance> {
		match self {
			IntentData::Swap(s) => U256::from(resolve.amount_in())
				.checked_mul(U256::from(s.amount_out))?
				.checked_div(U256::from(s.amount_in))?
				.checked_into(),
			IntentData::Dca(_) => None, // DCA is never partial
		}
	}
}

/// Whether an intent supports partial fills.
#[derive(Clone, Copy, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Partial {
	/// All-or-nothing: intent must be fully resolved or not at all.
	No,
	/// Partially fillable. `Balance` = cumulative amount_in already filled.
	/// Original `amount_in` and `amount_out` are immutable; minimum rate is
	/// always derived from their ratio.
	Yes(Balance),
}

impl Partial {
	/// Returns the cumulative filled amount, or 0 for non-partial intents.
	pub fn filled(&self) -> Balance {
		match self {
			Partial::No => 0,
			Partial::Yes(filled) => *filled,
		}
	}

	pub fn is_partial(&self) -> bool {
		matches!(self, Partial::Yes(_))
	}
}

impl From<bool> for Partial {
	fn from(partial: bool) -> Self {
		if partial {
			Partial::Yes(0)
		} else {
			Partial::No
		}
	}
}

/// User-facing swap parameters for intent submission.
#[derive(Clone, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct SwapParams {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub partial: bool,
}

/// Stored swap data with partial fill tracking.
/// Original `amount_in` and `amount_out` are immutable — minimum rate is
/// always derived from their ratio.
#[derive(Clone, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct SwapData {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub partial: Partial,
}

impl SwapData {
	/// Remaining amount that can still be filled.
	pub fn remaining(&self) -> Balance {
		self.amount_in.saturating_sub(self.partial.filled())
	}
}

impl From<&SwapParams> for SwapData {
	fn from(params: &SwapParams) -> Self {
		SwapData {
			asset_in: params.asset_in,
			asset_out: params.asset_out,
			amount_in: params.amount_in,
			amount_out: params.amount_out,
			partial: Partial::from(params.partial),
		}
	}
}

/// User-facing DCA parameters for intent submission.
/// Does not include internal state fields (remaining_budget, last_execution_block)
/// which are initialized by the pallet.
#[derive(Clone, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct DcaParams {
	/// Asset being sold per trade
	pub asset_in: AssetId,
	/// Asset being bought per trade
	pub asset_out: AssetId,
	/// Per-trade exact sell amount
	pub amount_in: Balance,
	/// Per-trade hard minimum receive (user's absolute floor)
	pub amount_out: Balance,
	/// Dynamic slippage tolerance applied relative to oracle price
	pub slippage: Permill,
	/// Total budget: Some(amount) = fixed, None = rolling/indefinite
	pub budget: Option<Balance>,
	/// Blocks between executions
	pub period: u32,
}

impl DcaParams {
	pub fn into_data(self, remaining_budget: Balance, last_execution_block: u32) -> DcaData {
		DcaData {
			asset_in: self.asset_in,
			asset_out: self.asset_out,
			amount_in: self.amount_in,
			amount_out: self.amount_out,
			slippage: self.slippage,
			budget: self.budget,
			remaining_budget,
			period: self.period,
			last_execution_block,
		}
	}
}

/// Creates a DCA intent for an account whose old-pallet DCA schedule is being converted.
///
/// Implementations must bypass the per-account intent cap: the schedule already existed and the old
/// pallet releases its reserve before this is called, so refusing here would strand the user's funds.
/// Deliberately not implemented for `()` — a runtime that forgets to wire it must fail to compile
/// rather than silently cancel every schedule it was supposed to migrate.
pub trait IntentMigrator<AccountId> {
	fn add_migrated_intent(owner: AccountId, params: DcaParams) -> Result<IntentId, DispatchError>;
}

#[derive(Clone, DecodeWithMemTracking, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct DcaData {
	/// Asset being sold per trade
	pub asset_in: AssetId,
	/// Asset being bought per trade
	pub asset_out: AssetId,
	/// Per-trade exact sell amount
	pub amount_in: Balance,
	/// Per-trade hard minimum receive (user's absolute floor)
	pub amount_out: Balance,
	/// Dynamic slippage tolerance applied relative to oracle price
	pub slippage: Permill,
	/// Total budget: Some(amount) = fixed, None = rolling/indefinite
	pub budget: Option<Balance>,
	/// Remaining reserved funds (mutable, decremented after each trade)
	pub remaining_budget: Balance,
	/// Blocks between executions
	pub period: u32,
	/// Block when DCA was last executed (or created); updated on each resolution
	pub last_execution_block: u32,
}

impl DcaData {
	/// Convert DCA per-trade parameters to a SwapData for solver presentation.
	///
	/// The caller supplies `amount_out` — typically the oracle-derived
	/// effective limit (dynamic slippage floor), not the user's hard limit.
	/// Using the effective limit keeps the solver's view of the intent in
	/// sync with the pallet's resolve-time check in
	/// `validate_dca_intent_resolve`.
	pub fn to_swap_data(&self, amount_out: Balance) -> SwapData {
		SwapData {
			asset_in: self.asset_in,
			asset_out: self.asset_out,
			amount_in: self.amount_in,
			amount_out,
			partial: Partial::No,
		}
	}
}

#[derive(
	Copy,
	DecodeWithMemTracking,
	Clone,
	Encode,
	Decode,
	Eq,
	PartialEq,
	RuntimeDebug,
	MaxEncodedLen,
	TypeInfo,
	PartialOrd,
	Ord,
)]
pub enum SwapType {
	ExactIn,
	ExactOut,
}

impl SwapType {
	pub fn reverse(&self) -> Self {
		if *self == SwapType::ExactIn {
			return SwapType::ExactOut;
		}

		Self::ExactIn
	}
}

#[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, DecodeWithMemTracking, Eq)]
pub struct Solution {
	pub resolved_intents: ResolvedIntents,
	pub trades: SolutionTrades,
	pub score: Score,
	/// Block whose state the solution was built against, stamped by the submitter.
	///
	/// Never read by the runtime — it exists so that two solutions built one block
	/// apart never encode to the same bytes (an identical extrinsic hash gets
	/// refused by the tx pool as `TemporarilyBanned` after the previous one was
	/// rejected or went stale), and so submit→execute drift is observable.
	/// Unforgeable today only because `submit_solution` is `Local`-only; if solver
	/// competition opens the call up, gate it with `built_at < current_block`.
	pub built_at: BlockNumber,
}

impl Solution {
	/// Solvers do not know which block their input snapshot came from — the
	/// submitter stamps `built_at`.
	pub fn new(resolved_intents: ResolvedIntents, trades: SolutionTrades, score: Score) -> Self {
		Self {
			resolved_intents,
			trades,
			score,
			built_at: 0,
		}
	}
}

#[derive(Debug, DecodeWithMemTracking, Clone, Encode, Decode, TypeInfo, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
	use super::*;

	fn solution_at(built_at: BlockNumber) -> Solution {
		let mut s = Solution::new(
			ResolvedIntents::truncate_from(Vec::new()),
			SolutionTrades::truncate_from(Vec::new()),
			42,
		);
		s.built_at = built_at;
		s
	}

	#[test]
	fn solution_should_encode_differently_when_built_at_differs() {
		assert_ne!(solution_at(747_864).encode(), solution_at(747_865).encode());
	}

	#[test]
	fn solution_should_encode_identically_when_built_at_matches() {
		assert_eq!(solution_at(747_864).encode(), solution_at(747_864).encode());
	}

	#[test]
	fn solution_new_should_leave_built_at_unstamped() {
		let s = Solution::new(
			ResolvedIntents::truncate_from(Vec::new()),
			SolutionTrades::truncate_from(Vec::new()),
			42,
		);
		assert_eq!(s.built_at, 0);
	}
}
