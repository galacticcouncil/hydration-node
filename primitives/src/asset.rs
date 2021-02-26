use crate::AssetId;

use codec::{Decode, Encode};

use sp_std::vec::Vec;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Asset Pair representation for AMM trades
/// ( asset_a, asset_b ) combination where asset_a is meant to be exchanged for asset_b
///
/// asset_in represents asset coming into the pool
/// asset_out represents asset coming out of the pool
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Debug, Encode, Decode, Copy, Clone, PartialEq, Eq, Default)]
pub struct AssetPair {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

impl AssetPair {
	pub fn new(asset_in: AssetId, asset_out: AssetId) -> Self {
		Self { asset_in, asset_out }
	}
	/// Return ordered asset tuple (A,B) where A < B
	/// Used in storage
	pub fn ordered_pair(&self) -> (AssetId, AssetId) {
		match self.asset_in <= self.asset_out {
			true => (self.asset_in, self.asset_out),
			false => (self.asset_out, self.asset_in),
		}
	}

	/// Return share token name
	pub fn name(&self) -> Vec<u8> {
		let mut buf: Vec<u8> = Vec::new();

		let (asset_a, asset_b) = self.ordered_pair();

		buf.extend_from_slice(&asset_a.to_le_bytes());
		buf.extend_from_slice(b"HDT");
		buf.extend_from_slice(&asset_b.to_le_bytes());

		buf
	}
}
