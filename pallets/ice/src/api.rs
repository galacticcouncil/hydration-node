#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use crate::types::{AssetId, Balance, Intent, IntentId, ResolvedIntent};
use alloc::vec::Vec;
use codec::Decode;
use sp_std::sync::Arc;
use sp_std::vec;

pub trait SolutionProvider: Send + Sync {
	fn get_solution(&self, intents: Vec<IntentRepr>, data: Vec<DataRepr>) -> Vec<ResolvedIntent>;
}

pub type SolverPtr = Arc<dyn SolutionProvider + Send + 'static>;

#[cfg(feature = "std")]
sp_externalities::decl_extension! {
	/// The solver extension to retrieve solution from the externalities.
	pub struct SolverExt(SolverPtr);
}

use hydradx_traits::ice::AmmInfo;
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
// 8. Pool id (stableswap)
// 9. Amplification (stableswap)
// 10. Shares (stableswap)
// 11. D (stableswap)
pub type DataRepr = (
	u8,
	AssetId,
	Balance,
	Balance,
	u8,
	(u32, u32),
	(u32, u32),
	AssetId,
	u128,
	u128,
	u128,
);

// Intent representation
// 1. Intent identifier
// 2. Asset in identifier
// 3. Asset out identifier
// 4. Amount in
// 5. Amount out
pub type IntentRepr = (IntentId, AssetId, AssetId, Balance, Balance, bool);

pub(crate) fn into_intent_repr<AccountId>(data: (IntentId, Intent<AccountId>)) -> IntentRepr {
	(
		data.0,
		data.1.swap.asset_in,
		data.1.swap.asset_out,
		data.1.swap.amount_in,
		data.1.swap.amount_out,
		data.1.partial,
	)
}

pub(crate) fn into_pool_data_repr(data: AmmInfo<AssetId>) -> Vec<DataRepr> {
	let mut r = vec![];
	match data {
		AmmInfo::Omnipool(state) => {
			for asset in state.assets {
				let fee = (asset.fee.deconstruct(), 1_000_000);
				let hub_fee = (asset.hub_fee.deconstruct(), 1_000_000);
				r.push((
					0,
					asset.asset_id,
					asset.reserve,
					asset.hub_reserve,
					asset.decimals,
					fee,
					hub_fee,
					0,
					0,
					0,
					0,
				));
			}
		}
		AmmInfo::Stablepool(state) => {
			let fee = (state.fee.deconstruct(), 1_000_000);
			let pool_id = state.pool_id;
			let amp = state.amplification;
			for asset in state.assets {
				r.push((
					1,
					asset.asset_id,
					asset.reserve,
					0,
					asset.decimals,
					fee,
					(0, 0),
					pool_id,
					amp,
					state.shares,
					state.d,
				));
			}
		}
	}
	r
}
