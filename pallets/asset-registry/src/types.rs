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

pub type Balance = u128;

use hydradx_traits::AssetKind;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type Name<L> = BoundedVec<u8, L>;
pub type Symbol<L> = BoundedVec<u8, L>;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AssetType {
	Token,
	XYK,
	StableSwap,
	Bond,
	External,
	Erc20,
}

impl From<AssetKind> for AssetType {
	fn from(value: AssetKind) -> Self {
		match value {
			AssetKind::Token => Self::Token,
			AssetKind::XYK => Self::XYK,
			AssetKind::StableSwap => Self::StableSwap,
			AssetKind::Bond => Self::Bond,
			AssetKind::External => Self::External,
			AssetKind::Erc20 => Self::Erc20,
		}
	}
}

impl From<AssetType> for AssetKind {
	fn from(value: AssetType) -> Self {
		match value {
			AssetType::Token => Self::Token,
			AssetType::XYK => Self::XYK,
			AssetType::StableSwap => Self::StableSwap,
			AssetType::Bond => Self::Bond,
			AssetType::External => Self::External,
			AssetType::Erc20 => Self::Erc20,
		}
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(StringLimit))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct AssetDetails<StringLimit: Get<u32>> {
	/// The name of this asset. Limited in length by `StringLimit`.
	pub name: Option<Name<StringLimit>>,

	/// Asset type
	pub asset_type: AssetType,

	/// Existential deposit
	pub existential_deposit: Balance,

	/// The ticker symbol for this asset. Limited in length by `StringLimit`.
	pub symbol: Option<Symbol<StringLimit>>,

	/// The number of decimals this asset uses to represent one unit.
	pub decimals: Option<u8>,

	/// XCM rate limit.
	pub xcm_rate_limit: Option<Balance>,

	/// Asset sufficiency.
	pub is_sufficient: bool,
}

impl<StringLimit: Get<u32>> AssetDetails<StringLimit> {
	pub fn new(
		name: Option<BoundedVec<u8, StringLimit>>,
		asset_type: AssetType,
		existential_deposit: Balance,
		symbol: Option<BoundedVec<u8, StringLimit>>,
		decimals: Option<u8>,
		xcm_rate_limit: Option<Balance>,
		is_sufficient: bool,
	) -> Self {
		Self {
			name,
			asset_type,
			existential_deposit,
			symbol,
			decimals,
			xcm_rate_limit,
			is_sufficient,
		}
	}
}
