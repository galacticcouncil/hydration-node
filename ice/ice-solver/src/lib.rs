#![cfg_attr(not(feature = "std"), no_std)]
pub mod common;
pub mod passthrough;
pub mod v4;

#[cfg(feature = "std")]
pub mod replay_format;

#[cfg(test)]
mod tests;

use frame_support::sp_runtime::Permill;
use hydradx_traits::amm::AMMInterface;
use ice_support::{Balance, Intent, IntentId, Solution};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

/// Minimum output the chain enforces at resolution, for the intents where it is
/// stricter than their own `amount_out` (today the DCA oracle floor).
///
/// Admission only. `amount_out` remains the sole basis for `surplus`, because
/// the chain re-derives the score from storage and any divergence is a
/// `ScoreMismatch`. These floors are recomputed from an oracle and must never
/// reach the score.
pub type MinOuts = BTreeMap<IntentId, Balance>;

/// Whether one AMM transfer may be split across several pool-disjoint routes.
///
/// Chosen per node and never shipped on chain: solutions built with and
/// without splitting are equally valid, so collators may differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitConfig {
	/// Legs per transfer, at most [`common::split::MAX_SPLIT_LEGS`]. `1` turns
	/// splitting off and reproduces the single-route solver exactly.
	pub max_legs: u8,
	/// Extra simulations fitting and trade building may each spend per solve on
	/// split searches before falling back to single routes.
	pub sell_budget: u32,
	/// Price split totals while fitting too, so the crossing admits the volume a
	/// split can fill.
	pub fitting: bool,
}

impl Default for SplitConfig {
	fn default() -> Self {
		Self {
			max_legs: 2,
			sell_budget: 10_000,
			fitting: true,
		}
	}
}

impl SplitConfig {
	pub fn disabled() -> Self {
		Self {
			max_legs: 1,
			..Self::default()
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolverOptions {
	pub split: SplitConfig,
	/// Smallest partial fill, as a share of the intent's `amount_in`; a smaller
	/// remainder is only ever filled whole. A limit only small trades clear would
	/// otherwise be filled a sliver per block, one solution each.
	pub min_partial_fill: Permill,
}

impl Default for SolverOptions {
	fn default() -> Self {
		Self {
			split: SplitConfig::default(),
			min_partial_fill: Permill::from_percent(1),
		}
	}
}

/// The entry points every solver generation exposes.
///
/// One interface per generation makes the mode switch a type parameter: the
/// node worker, the integration harness and the benches all pick a builder
/// without knowing anything else about it.
pub trait IceSolver<A: AMMInterface> {
	fn solve_with_options(
		intents: Vec<Intent>,
		min_outs: MinOuts,
		initial_state: A::State,
		matched_fee: Permill,
		options: &SolverOptions,
	) -> Result<Solution, A::Error>;

	fn solve_with_limits(
		intents: Vec<Intent>,
		min_outs: MinOuts,
		initial_state: A::State,
		matched_fee: Permill,
	) -> Result<Solution, A::Error> {
		Self::solve_with_options(intents, min_outs, initial_state, matched_fee, &SolverOptions::default())
	}

	fn solve(intents: Vec<Intent>, initial_state: A::State, matched_fee: Permill) -> Result<Solution, A::Error> {
		Self::solve_with_limits(intents, MinOuts::new(), initial_state, matched_fee)
	}
}
