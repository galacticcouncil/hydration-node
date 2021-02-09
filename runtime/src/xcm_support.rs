#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{convert::TryFrom, vec::Vec};

use sp_std::{marker::PhantomData, prelude::*};
use xcm::v0::{MultiAsset, MultiLocation};

use sp_std::collections::btree_map::BTreeMap;

use xcm::v0::Junction;

use frame_support::traits::Get;
use xcm_executor::traits::{FilterAssetLocation, NativeAsset};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	HDT = 0,
	DOT = 1,
}

impl Into<Vec<u8>> for CurrencyId {
	fn into(self) -> Vec<u8> {
		use CurrencyId::*;
		match self {
			HDT => b"HDT".to_vec(),
			DOT => b"DOT".to_vec(),
		}
	}
}

impl TryFrom<Vec<u8>> for CurrencyId {
	type Error = ();
	fn try_from(v: Vec<u8>) -> Result<CurrencyId, ()> {
		match v.as_slice() {
			b"HDT" => Ok(CurrencyId::HDT),
			b"DOT" => Ok(CurrencyId::DOT),
			_ => Err(()),
		}
	}
}

pub struct NativePalletAssetOr<NativeTokens>(PhantomData<NativeTokens>);

impl<NativeTokens: Get<BTreeMap<Vec<u8>, MultiLocation>>> FilterAssetLocation for NativePalletAssetOr<NativeTokens> {
	fn filter_asset_location(asset: &MultiAsset, origin: &MultiLocation) -> bool {
		if NativeAsset::filter_asset_location(asset, origin) {
			return true;
		}

		// native asset identified by a general key
		if let MultiAsset::ConcreteFungible { ref id, .. } = asset {
			if let Some(Junction::GeneralKey(key)) = id.last() {
				if NativeTokens::get().contains_key(key) {
					return (*origin) == *(NativeTokens::get().get(key).unwrap());
				}
			}
		}

		false
	}
}
