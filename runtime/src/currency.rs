#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{convert::TryFrom, vec::Vec};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum SupportedAssetIds {
	HDX = 0,
	DOT = 1,
}

impl Into<Vec<u8>> for SupportedAssetIds {
	fn into(self) -> Vec<u8> {
		use SupportedAssetIds::*;
		match self {
			HDX => b"HDX".to_vec(),
			DOT => b"DOT".to_vec(),
		}
	}
}

impl TryFrom<Vec<u8>> for SupportedAssetIds {
	type Error = ();
	fn try_from(v: Vec<u8>) -> Result<SupportedAssetIds, ()> {
		match v.as_slice() {
			b"HDX" => Ok(SupportedAssetIds::HDX),
			b"DOT" => Ok(SupportedAssetIds::DOT),
			_ => Err(()),
		}
	}
}
