#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use crate::types::{AssetId, Balance, Intent, IntentId, ResolvedIntent};
use alloc::vec::Vec;
use codec::Decode;
use sp_std::sync::Arc;

pub trait SolutionProvider: Send + Sync {
	fn get_solution(&self, intents: Vec<IntentRepr>, data: Vec<DataRepr>) -> Vec<ResolvedIntent>;
}

pub type SolverPtr = Arc<dyn SolutionProvider + Send + 'static>;

#[cfg(feature = "std")]
sp_externalities::decl_extension! {
	/// The solver extension to retrieve solution from the externalities.
	pub struct SolverExt(SolverPtr);
}

use hydradx_traits::ice::AssetInfo;
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

// AMM asset state representation
// 1. AMM identifier - 0 for Omnipool, 1 for StableSwap )
// 2. Asset identifier
// 3. Reserve amount
// 4. Hub reserve amount
// 5. Decimals
// 6. Fee
// 7. Hub fee
pub type DataRepr = (u8, AssetId, Balance, Balance, u8, (u32, u32), (u32, u32));

// Intent representation
// 1. Intent identifier
// 2. Asset in identifier
// 3. Asset out identifier
// 4. Amount in
// 5. Amount out
pub type IntentRepr = (IntentId, AssetId, AssetId, Balance, Balance);

pub(crate) fn into_intent_repr<AccountId>(data: (IntentId, Intent<AccountId>)) -> IntentRepr {
	(
		data.0,
		data.1.swap.asset_in,
		data.1.swap.asset_out,
		data.1.swap.amount_in,
		data.1.swap.amount_out,
	)
}

pub(crate) fn into_pool_data_repr(data: AssetInfo<AssetId>) -> DataRepr {
	(0, 0, 0, 0, 0, (0, 0), (0, 0))
}
