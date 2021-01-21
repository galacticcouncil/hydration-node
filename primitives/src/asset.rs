use crate::AssetId;

use codec::{Decode, Encode};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Asset Pair representation
/// ( asset_a, asset_b ) combination where asset_a is meant to be exchanged for asset_b
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, Default)]
pub struct AssetPair {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

pub type AssetPairType = AssetPair;

impl AssetPair {
	pub fn new(asset_in: AssetId, asset_out: AssetId) -> Self {
		Self { asset_in, asset_out }
	}
	/// Return ordered asset tuple (A,B) where A < B
	/// Used in storage
	pub fn pair(&self) -> (AssetId, AssetId) {
		match self.asset_in <= self.asset_out {
			true => (self.asset_in, self.asset_out),
			false => (self.asset_out, self.asset_in),
		}
	}
}
