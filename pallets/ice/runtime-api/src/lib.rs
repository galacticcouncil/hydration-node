#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use ice_support::{AssetId, Balance, Intent, IntentId};
use scale_info::TypeInfo;
use sp_runtime::Permill;
use sp_std::vec::Vec;

pub use ice_support::Solution;
pub use ice_support::SolverMode;

/// Side-effect-free inputs the node needs to run the ICE solver natively.
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct SolverInput {
	pub intents: Vec<Intent>,
	/// SCALE-encoded simulator snapshot (the `SimulatorSet::State` tuple).
	pub state: Vec<u8>,
	/// ED for every asset the solver may query (snapshot pool assets ∪ intent assets).
	pub existential_deposits: Vec<(AssetId, Balance)>,
	/// Minimum output the chain enforces at resolution, for the intents where it
	/// differs from their own `amount_out` — today the oracle-derived floor on DCA
	/// intents. Admission only: the score is still derived from `amount_out`, which
	/// is stored on-chain and therefore exactly reproducible. An intent absent here
	/// is bound by its own `amount_out`.
	pub min_amount_out: Vec<(IntentId, Balance)>,
	pub fee: Permill,
	/// Active solver mode — read from the same block state as the rest of the input.
	///
	/// Appended last on purpose: a stale binary still decodes the leading fields, and
	/// its solutions are shape-rejected at validation under any non-`V4` mode anyway.
	pub mode: SolverMode,
}

sp_api::decl_runtime_apis! {
	/// Inputs for the node-side ICE solver. Side-effect-free; called once per block.
	pub trait IceSolverApi {
		fn solver_input() -> Option<SolverInput>;
	}
}
