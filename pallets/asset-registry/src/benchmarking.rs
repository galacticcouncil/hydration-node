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

use crate::types::AssetDetails;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;

benchmarks! {
	 where_clause { where
		T: crate::pallet::Config,
	}

	register {
		let asset_id= T::AssetId::from(3);
		let name = b"Test name".to_vec();
		let ed = 1_000_000_u128;
		let symbol = b"TKN".to_vec();
		let decimals = 12_u8;
		let location: T::AssetNativeLocation = Default::default();
		let xcm_rate_limit = 1_000_u128;
		let is_sufficient = true;

	}: _(RawOrigin::Root, Some(asset_id), Some(name.clone()), AssetType::Token, Some(ed), Some(symbol), Some(decimals), Some(location), Some(xcm_rate_limit), is_sufficient)
	verify {
		let b_name = Pallet::<T>::to_bounded_name(name).unwrap();
		assert!(Pallet::<T>::asset_ids(b_name).is_some());

		assert!(Pallet::<T>::assets(asset_id).is_some());
	}

	update {
		let asset_id = T::AssetId::from(3);
		let name = b"Test name".to_vec();
		let ed = 1_000_000_u128;
		let symbol = b"TKN".to_vec();
		let decimals = 12_u8;
		let location: T::AssetNativeLocation = Default::default();
		let xcm_rate_limit = 1_000_u128;
		let is_sufficient = true;

		let _ = Pallet::<T>::register(RawOrigin::Root.into(), Some(asset_id), Some(name), AssetType::Token, Some(ed), Some(symbol), Some(decimals), Some(location), Some(xcm_rate_limit), is_sufficient);

		let new_name= b"New name".to_vec();
		let new_type = AssetType::PoolShare(T::AssetId::from(10u8),T::AssetId::from(20u8));
		let new_ed = 1_000_000_u128;
		let new_xcm_rate_limit = 1_000_u128;
		let new_is_sufficient = false;
		let new_symbol = b"TKNn".to_vec();
		let new_decimals = 12_u8;


	}: _(RawOrigin::Root, asset_id, Some(new_name.clone()), Some(new_type), Some(new_ed), Some(new_xcm_rate_limit), Some(new_is_sufficient), Some(new_symbol.clone()), Some(new_decimals), Some(Default::default()))
	verify {
		let b_name = Pallet::<T>::to_bounded_name(new_name).unwrap();
		let b_symbol = Pallet::<T>::to_bounded_name(new_symbol).unwrap();

		assert_eq!(Pallet::<T>::asset_ids(&b_name), Some(asset_id));

		assert_eq!(crate::Pallet::<T>::assets(asset_id), Some(AssetDetails {
			name: Some(b_name),
			asset_type: new_type,
			existential_deposit: new_ed,
			symbol: Some(b_symbol),
			decimals: Some(new_decimals),
			xcm_rate_limit: Some(xcm_rate_limit),
			is_sufficient: new_is_sufficient,
		}));
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
