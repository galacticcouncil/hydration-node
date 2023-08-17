// This file is part of pallet-asset-registry.

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

use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

use hydradx_traits::AssetKind;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AssetType<AssetId> {
	Token,
	PoolShare(AssetId, AssetId), // Use XYX instead
	XYK,
	StableSwap,
	Bond,
}

impl<AssetId> From<AssetKind> for AssetType<AssetId> {
	fn from(value: AssetKind) -> Self {
		match value {
			AssetKind::Token => Self::Token,
			AssetKind::XYK => Self::XYK,
			AssetKind::StableSwap => Self::StableSwap,
			AssetKind::Bond => Self::Bond,
		}
	}
}

impl<AssetId> From<AssetType<AssetId>> for AssetKind {
	fn from(value: AssetType<AssetId>) -> Self {
		match value {
			AssetType::Token => Self::Token,
			AssetType::PoolShare(_, _) => Self::XYK,
			AssetType::XYK => Self::XYK,
			AssetType::StableSwap => Self::StableSwap,
			AssetType::Bond => Self::Bond,
		}
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct AssetDetails<AssetId, Balance, BoundedString> {
	/// The name of this asset. Limited in length by `StringLimit`.
	pub name: BoundedString,

	pub asset_type: AssetType<AssetId>,

	pub existential_deposit: Balance,

	pub xcm_rate_limit: Option<Balance>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct AssetMetadata<BoundedString> {
	/// The ticker symbol for this asset. Limited in length by `StringLimit`.
	pub(super) symbol: BoundedString,
	/// The number of decimals this asset uses to represent one unit.
	pub(super) decimals: u8,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Metadata {
	pub(super) symbol: Vec<u8>,
	pub(super) decimals: u8,
}
