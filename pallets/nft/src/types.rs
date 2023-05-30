// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use frame_support::pallet_prelude::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use scale_info::TypeInfo;

/// NFT Collection ID
pub type CollectionId = u128;

/// NFT Item ID
pub type ItemId = u128;

#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CollectionInfo<CollectionType, BoundedVec> {
    /// A collection type that implies permissions, e.g. for transfer and other operations
    pub collection_type: CollectionType,
    /// Arbitrary data about a collection, e.g. IPFS hash
    pub metadata: BoundedVec,
}

#[derive(Encode, Decode, Eq, Copy, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ItemInfo<BoundedVec> {
    pub metadata: BoundedVec,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CollectionType {
    Marketplace = 0_isize,
    LiquidityMining = 1_isize,
}

impl Default for CollectionType {
    fn default() -> Self {
        CollectionType::Marketplace
    }
}

pub trait NftPermission<InnerCollectionType> {
    fn can_create(collection_type: &InnerCollectionType) -> bool;
    fn can_mint(collection_type: &InnerCollectionType) -> bool;
    fn can_transfer(collection_type: &InnerCollectionType) -> bool;
    fn can_burn(collection_type: &InnerCollectionType) -> bool;
    fn can_destroy(collection_type: &InnerCollectionType) -> bool;
    fn has_deposit(collection_type: &InnerCollectionType) -> bool;
}

#[derive(Encode, Decode, Eq, Copy, PartialEq, Clone, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct NftPermissions;

impl NftPermission<CollectionType> for NftPermissions {
    fn can_create(collection_type: &CollectionType) -> bool {
        matches!(*collection_type, CollectionType::Marketplace)
    }

    fn can_mint(collection_type: &CollectionType) -> bool {
        matches!(*collection_type, CollectionType::Marketplace)
    }

    fn can_transfer(collection_type: &CollectionType) -> bool {
        matches!(
            *collection_type,
            CollectionType::Marketplace | CollectionType::LiquidityMining
        )
    }

    fn can_burn(collection_type: &CollectionType) -> bool {
        matches!(*collection_type, CollectionType::Marketplace)
    }

    fn can_destroy(collection_type: &CollectionType) -> bool {
        matches!(*collection_type, CollectionType::Marketplace)
    }

    fn has_deposit(collection_type: &CollectionType) -> bool {
        matches!(*collection_type, CollectionType::Marketplace)
    }
}
