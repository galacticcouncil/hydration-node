#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use crate::types::{AssetId, Balance, Intent, IntentId, ResolvedIntent};
use alloc::vec::Vec;
use codec::Decode;
use sp_std::sync::Arc;

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait ICEApi<A>
	where Vec<(IntentId, Intent<A>)>: Decode,
	Vec<ResolvedIntent>: Decode
	{
		fn intents(header: &Block::Header) -> Vec<(IntentId, Intent<A>)>;
		fn submit_solution(
			header: &Block::Header,
			solution: Vec<ResolvedIntent>,
		) -> Result<(), sp_runtime::DispatchError>;
	}
}

pub trait SolutionProvider: Send + Sync {
	fn get_solution(&self, intents: Vec<IntentRepr>, data: Vec<DataRepr>) -> Vec<ResolvedIntent>;
}

pub type SolverPtr = Arc<dyn SolutionProvider + Send + 'static>;

#[cfg(feature = "std")]
sp_externalities::decl_extension! {
	/// The keystore extension to register/retrieve from the externalities.
	pub struct SolverExt(SolverPtr);
}

use crate::traits::AssetInfo;
#[cfg(feature = "std")]
use sp_externalities::{Externalities, ExternalitiesExt};
use sp_runtime_interface::{runtime_interface, RIType};

#[runtime_interface]
pub trait ICE {
	fn get_solution(&mut self, intents: Vec<IntentRepr>, data: Vec<DataRepr>) -> Vec<ResolvedIntent> {
		self.extension::<SolverExt>()
			.expect("SolutionStoreExt is not registered")
			.get_solution(intents, data)
	}
}

// Unfortunately, we need simple representations of the types to be able to use across the FFI
// dev: perhaps, it could be possible to implement IntoFFIValue to simplify.
pub type DataRepr = (u8, AssetId, Balance, Balance, u8, (u32, u32), (u32, u32));
pub type IntentRepr = (IntentId, AssetId, AssetId, Balance, Balance);

impl From<AssetInfo<AssetId>> for DataRepr {
	fn from(value: AssetInfo<AssetId>) -> Self {
		todo!()
	}
}

impl From<DataRepr> for AssetInfo<AssetId> {
	fn from(value: DataRepr) -> Self {
		todo!()
	}
}
