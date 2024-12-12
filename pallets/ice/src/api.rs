#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use crate::types::{Intent, IntentId, ResolvedIntent};
use alloc::vec::Vec;
use codec::Decode;
use sp_std::sync::Arc;

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait ICEApi<A, AssetId>
	where Vec<(IntentId, Intent<A, AssetId>)>: Decode,
	Vec<ResolvedIntent>: Decode
	{
		fn intents(header: &Block::Header) -> Vec<(IntentId, Intent<A, AssetId>)>;
		fn submit_solution(
			header: &Block::Header,
			solution: Vec<ResolvedIntent>,
		) -> Result<(), sp_runtime::DispatchError>;
	}
}

pub trait SolutionProvider: Send + Sync {
	fn get_solution(&self) -> Vec<ResolvedIntent>;
}

pub type SolverPtr = Arc<dyn SolutionProvider + Send + 'static>;

#[cfg(feature = "std")]
sp_externalities::decl_extension! {
	/// The keystore extension to register/retrieve from the externalities.
	pub struct SolverExt(SolverPtr);
}

#[cfg(feature = "std")]
use sp_externalities::{Externalities, ExternalitiesExt};
use sp_runtime_interface::runtime_interface;

#[runtime_interface]
pub trait ICE {
	fn get_solution(&mut self) -> Vec<ResolvedIntent> {
		self.extension::<SolverExt>()
			.expect("SolutionStoreExt is not registered")
			.get_solution()
	}
}
