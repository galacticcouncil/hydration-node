#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use ice_support::{AssetId, Balance, Intent};
use scale_info::TypeInfo;
use sp_runtime::Permill;
use sp_std::vec::Vec;

pub use ice_support::Solution;

/// Side-effect-free inputs the node needs to run the ICE solver natively.
#[derive(Clone, Encode, Decode, TypeInfo)]
pub struct SolverInput {
	pub intents: Vec<Intent>,
	/// SCALE-encoded simulator snapshot (the `SimulatorSet::State` tuple).
	pub state: Vec<u8>,
	/// ED for every asset the solver may query (snapshot pool assets ∪ intent assets).
	pub existential_deposits: Vec<(AssetId, Balance)>,
	pub fee: Permill,
}

sp_api::decl_runtime_apis! {
	/// Inputs for the node-side ICE solver. Side-effect-free; called once per block.
	pub trait IceSolverApi {
		fn solver_input() -> Option<SolverInput>;
	}
}
