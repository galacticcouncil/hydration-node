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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate as NFT;
use frame_benchmarking::{account, benchmarks, vec};
use frame_support::traits::{tokens::nonfungibles::InspectEnumerable, Currency, Get};
use frame_system::RawOrigin;
use pallet_uniques as UNQ;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::convert::TryInto;

const SEED: u32 = 0;
const ENDOWMENT: u128 = 100_000_000_000_000_000_000;
const COLLECTION_ID_0: u32 = 1_000_000;

fn create_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let caller: T::AccountId = account(name, index, SEED);
    <T as pallet_uniques::Config>::Currency::deposit_creating(&caller, ENDOWMENT.unique_saturated_into());
    caller
}

fn do_create_collection<T: Config>(caller: T::AccountId, collection_id: T::NftCollectionId) {
    let metadata: BoundedVec<_, _> = vec![0; <T as UNQ::Config>::StringLimit::get() as usize]
        .try_into()
        .unwrap();
    assert!(NFT::Pallet::<T>::create_collection(
        RawOrigin::Signed(caller).into(),
        collection_id,
        Default::default(),
        metadata
    )
    .is_ok());
}

fn do_mint<T: Config>(caller: T::AccountId, collection_id: T::NftCollectionId, item_id: T::NftItemId) {
    let metadata: BoundedVec<_, _> = vec![0; <T as UNQ::Config>::StringLimit::get() as usize]
        .try_into()
        .unwrap();
    assert!(NFT::Pallet::<T>::mint(RawOrigin::Signed(caller).into(), collection_id, item_id, metadata).is_ok());
}

benchmarks! {
    create_collection {
        let caller = create_account::<T>("caller", 0);
        let metadata: BoundedVec<_, _> = vec![0; <T as UNQ::Config>::StringLimit::get() as usize].try_into().unwrap();
    }: _(RawOrigin::Signed(caller.clone()), COLLECTION_ID_0.into(), Default::default(), metadata)
    verify {
        assert_eq!(UNQ::Pallet::<T>::collection_owner(T::NftCollectionId::from(COLLECTION_ID_0).into()), Some(caller));
    }

    mint {
        let caller = create_account::<T>("caller", 0);
        do_create_collection::<T>(caller.clone(), 1_000_000u32.into());
        let metadata: BoundedVec<_, _> = vec![0; <T as UNQ::Config>::StringLimit::get() as usize].try_into().unwrap();
    }: _(RawOrigin::Signed(caller.clone()), COLLECTION_ID_0.into(), 0u32.into(), metadata)
    verify {
        assert_eq!(UNQ::Pallet::<T>::owner(T::NftCollectionId::from(COLLECTION_ID_0).into(), T::NftItemId::from(0u32).into()), Some(caller));
    }

    transfer {
        let caller = create_account::<T>("caller", 1);
        do_create_collection::<T>(caller.clone(), COLLECTION_ID_0.into());
        let caller_lookup = T::Lookup::unlookup(caller.clone());
        let caller2 = create_account::<T>("caller2", 1);
        let caller2_lookup = T::Lookup::unlookup(caller2.clone());
        do_mint::<T>(caller.clone(), COLLECTION_ID_0.into(), 0u32.into());
    }: _(RawOrigin::Signed(caller), COLLECTION_ID_0.into(), 0u32.into(), caller2_lookup)
    verify {
        assert_eq!(UNQ::Pallet::<T>::owner(T::NftCollectionId::from(COLLECTION_ID_0).into(), T::NftItemId::from(0u32).into()), Some(caller2));
    }

    destroy_collection {
        let caller = create_account::<T>("caller", 1);
        do_create_collection::<T>(caller.clone(), COLLECTION_ID_0.into());
    }: _(RawOrigin::Signed(caller), COLLECTION_ID_0.into())
    verify {
        assert_eq!(UNQ::Pallet::<T>::collections().count(), 0);
    }

    burn {
        let caller = create_account::<T>("caller", 1);
        do_create_collection::<T>(caller.clone(), COLLECTION_ID_0.into());
        do_mint::<T>(caller.clone(), COLLECTION_ID_0.into(), 0u32.into());
    }: _(RawOrigin::Signed(caller.clone()), COLLECTION_ID_0.into(), 0u32.into())
    verify {
        assert_eq!(UNQ::Pallet::<T>::owned(&caller).count(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::Pallet;
    use crate::mock::*;
    use frame_benchmarking::impl_benchmark_test_suite;

    impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
