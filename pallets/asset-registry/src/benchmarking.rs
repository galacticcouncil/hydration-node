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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;

use crate::types::Metadata;

use sp_std::vec;

benchmarks! {
	register{
		let name = vec![1; T::StringLimit::get() as usize];
		let ed = T::Balance::from(1_000_000u32);

		let symbol = vec![1; T::StringLimit::get() as usize];

		let metadata = Metadata {
			symbol,
			decimals: 100,
		};

	}: _(RawOrigin::Root, name.clone(), AssetType::Token, ed, None, Some(metadata), Some(Default::default()), None)
	verify {
		let bname = crate::Pallet::<T>::to_bounded_name(name).unwrap();
		assert!(crate::Pallet::<T>::asset_ids(bname).is_some());
	}

	update{
		let name = b"NAME".to_vec();
		let ed = T::Balance::from(1_000_000u32);
		let asset_id = T::AssetId::from(10u8);
		let _ = crate::Pallet::<T>::register(RawOrigin::Root.into(), name, AssetType::Token, ed, Some(asset_id), None, None, None);

		let new_name= vec![1; T::StringLimit::get() as usize];

		let new_ed = T::Balance::from(2_000_000u32);

		let rate_limit = T::Balance::from(10_000_000u32);

	}: _(RawOrigin::Root, asset_id, new_name.clone(), AssetType::PoolShare(T::AssetId::from(10u8),T::AssetId::from(20u8)), Some(new_ed), Some(rate_limit))
	verify {
		let bname = crate::Pallet::<T>::to_bounded_name(new_name).unwrap();
		assert_eq!(crate::Pallet::<T>::asset_ids(&bname), Some(asset_id));

		let stored = crate::Pallet::<T>::assets(asset_id);

		assert!(stored.is_some());
		let stored = stored.unwrap();

		let expected = AssetDetails{
			asset_type: AssetType::PoolShare(T::AssetId::from(10u8), T::AssetId::from(20u8)),
			existential_deposit: new_ed,
			name: bname,
			xcm_rate_limit: Some(rate_limit),
		};

		assert_eq!(stored.asset_type, expected.asset_type);
		assert_eq!(stored.existential_deposit, expected.existential_deposit);
		assert_eq!(stored.name.to_vec(), expected.name.to_vec());
	}

	set_metadata{
		let name = b"NAME".to_vec();
		let bname = crate::Pallet::<T>::to_bounded_name(name.clone()).unwrap();
		let ed = T::Balance::from(1_000_000u32);
		let _ = crate::Pallet::<T>::register(RawOrigin::Root.into(), name, AssetType::Token, ed, None, None, None, None);

		let asset_id = crate::Pallet::<T>::asset_ids(bname).unwrap();

		let max_symbol = vec![1; T::StringLimit::get() as usize];

	}: _(RawOrigin::Root, asset_id, max_symbol.clone(), 10u8)
	verify {
		let bsymbol= crate::Pallet::<T>::to_bounded_name(max_symbol).unwrap();

		let stored = crate::Pallet::<T>::asset_metadata(asset_id);

		assert!(stored.is_some());

		let stored = stored.unwrap();

		let expected =AssetMetadata{
			symbol: bsymbol,
			decimals: 10u8
		};

		assert_eq!(stored.symbol.to_vec(), expected.symbol.to_vec());
		assert_eq!(stored.decimals, expected.decimals);
	}

	set_location{
		let name = b"NAME".to_vec();
		let ed = T::Balance::from(1_000_000u32);
		let asset_id = T::AssetId::from(10u8);
		let _ = crate::Pallet::<T>::register(RawOrigin::Root.into(), name.clone(), AssetType::Token, ed, Some(asset_id), None, None, None);

	}: _(RawOrigin::Root, asset_id, Default::default())
	verify {
		let bname = crate::Pallet::<T>::to_bounded_name(name).unwrap();
		let bsymbol= crate::Pallet::<T>::to_bounded_name(b"SYMBOL".to_vec()).unwrap();

		assert_eq!(crate::Pallet::<T>::locations(asset_id), Some(Default::default()));
		assert_eq!(crate::Pallet::<T>::location_assets(T::AssetNativeLocation::default()), Some(asset_id));
	}
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
