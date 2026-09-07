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

/// The entry points every solver generation exposes.
///
/// One interface per generation makes the mode switch a type parameter: the
/// node worker, the integration harness and the benches all pick a builder
/// without knowing anything else about it.
pub trait IceSolver<A: AMMInterface> {
	fn solve_with_limits(
		intents: Vec<Intent>,
		min_outs: MinOuts,
		initial_state: A::State,
		matched_fee: Permill,
	) -> Result<Solution, A::Error>;

	fn solve(intents: Vec<Intent>, initial_state: A::State, matched_fee: Permill) -> Result<Solution, A::Error> {
		Self::solve_with_limits(intents, MinOuts::new(), initial_state, matched_fee)
	}
}
