#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use pallet_intent::types::{AssetId, Balance, Intent, IntentId};
use alloc::vec::Vec;
use codec::Decode;
use sp_std::sync::Arc;
use sp_std::vec;

pub trait SolutionProvider: Send + Sync {
    fn get_solution(&self, data: Vec<u8>) -> Vec<u8>;
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
    fn get_solution(&mut self, data: Vec<u8>) -> Vec<u8> {
        self.extension::<SolverExt>()
            .expect("SolutionStoreExt is not registered")
            .get_solution(data)
    }
}