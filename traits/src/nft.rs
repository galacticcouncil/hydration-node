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

use frame_support::dispatch::DispatchResult;

pub trait CreateTypedCollection<AccountId, CollectionId, CollectionType, Metadata> {
    /// This function create an NFT collection of `collection_type` type.
    fn create_typed_collection(
        owner: AccountId,
        collection_id: CollectionId,
        collection_type: CollectionType,
        metadata: Option<Metadata>,
    ) -> DispatchResult;
}

pub trait ReserveCollectionId<CollectionId> {
    /// This function returns `true` if collection id is from the reserved range, `false` otherwise.
    fn is_id_reserved(id: CollectionId) -> bool;
}
