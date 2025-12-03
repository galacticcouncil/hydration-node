#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;
use codec::Decode;
use pallet_intent::types::{AssetId, Balance, Intent, IntentId};
use sp_std::sync::Arc;
use sp_std::vec;

pub trait SolutionProvider: Send + Sync {
	fn get_solution(&self, intents: Vec<u8>, data: Vec<u8>) -> Option<Vec<u8>>;
}

pub type SolverPtr = Arc<dyn SolutionProvider + Send + 'static>;

#[cfg(feature = "std")]
sp_externalities::decl_extension! {
	/// The solver extension to retrieve a solution from the externalities.
	pub struct SolverExt(SolverPtr);
}

#[cfg(feature = "std")]
use sp_externalities::{Externalities, ExternalitiesExt};
use sp_runtime_interface::{runtime_interface, RIType};

#[runtime_interface]
pub trait ICE {
	fn get_solution(&mut self, intents: Vec<u8>, data: Vec<u8>) -> Option<Vec<u8>> {
		self.extension::<SolverExt>()?.get_solution(intents, data)
	}
}
