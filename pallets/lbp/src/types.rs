// This file is part of HydraDX-node.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub type AssetId = u32;
pub type Amount = i128;
pub type Balance = u128;

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Asset Pair representation for AMM trades
/// ( asset_a, asset_b ) combination where asset_a is meant to be exchanged for asset_b
///
/// asset_in represents asset coming into the pool
/// asset_out represents asset coming out of the pool
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Debug, Encode, Decode, Copy, Clone, PartialEq, Eq, Default, TypeInfo)]
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
