// This file is part of pallet-dynamic-fees.
//
// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0
//
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

use super::*;
use crate::types::{AssetFeeConfig, FeeParams};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use sp_runtime::FixedU128;

benchmarks! {
	where_clause {
		where
			T: Config,
			T::AssetId: From<u32>,
	}

	set_asset_fee {
		let asset_id: T::AssetId = 1u32.into();
		// Worst case: Dynamic fee configuration with full validation
		let asset_fee_params = FeeParams {
			min_fee: T::Fee::from_percent(1.into()),
			max_fee: T::Fee::from_percent(10.into()),
			decay: FixedU128::from_rational(1, 10),
			amplification: FixedU128::from_rational(2, 1),
		};
		let protocol_fee_params = FeeParams {
			min_fee: T::Fee::from_percent(1.into()),
			max_fee: T::Fee::from_percent(5.into()),
			decay: FixedU128::from_rational(1, 5),
			amplification: FixedU128::from_rational(3, 1),
		};
		let config = AssetFeeConfig::Dynamic {
			asset_fee_params,
			protocol_fee_params,
		};
	}: _(RawOrigin::Root, asset_id, config)
	verify {
		assert!(AssetFeeConfiguration::<T>::contains_key(asset_id));
	}

	remove_asset_fee {
		let asset_id: T::AssetId = 1u32.into();
		let config = AssetFeeConfig::Fixed {
			asset_fee: T::Fee::from_percent(5.into()),
			protocol_fee: T::Fee::from_percent(2.into()),
		};

		// Setup: First set a configuration
		let _ = Pallet::<T>::set_asset_fee(
			RawOrigin::Root.into(),
			asset_id,
			config,
		);

		// Verify it was set
		assert!(AssetFeeConfiguration::<T>::contains_key(asset_id));
	}: _(RawOrigin::Root, asset_id)
	verify {
		assert!(!AssetFeeConfiguration::<T>::contains_key(asset_id));
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
