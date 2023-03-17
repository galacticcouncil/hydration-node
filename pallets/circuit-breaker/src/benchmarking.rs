// This file is part of Basilisk-node.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::benchmarks;
use frame_benchmarking::impl_benchmark_test_suite;
use frame_system::RawOrigin;
use sp_std::prelude::*;
benchmarks! {
	 where_clause {
		where T::AssetId: From<u32>,
	}

	set_trade_volume_limit {
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = (crate::MAX_LIMIT_VALUE, 1);

	}: _(RawOrigin::Root, asset_id, trade_limit)
	verify {
		assert_eq!(TradeVolumeLimitPerAsset::<T>::get(asset_id), trade_limit);
	}

	set_add_liquidity_limit {
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = Some((crate::MAX_LIMIT_VALUE, 1));

	}: _(RawOrigin::Root, asset_id, trade_limit)
	verify {
		assert_eq!(LiquidityAddLimitPerAsset::<T>::get(asset_id), trade_limit);
	}

	set_remove_liquidity_limit {
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = Some((crate::MAX_LIMIT_VALUE, 1));

	}: _(RawOrigin::Root, asset_id, trade_limit)
	verify {
		assert_eq!(LiquidityRemoveLimitPerAsset::<T>::get(asset_id), trade_limit);
	}

	ensure_add_liquidty_limit {
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = Some((crate::MAX_LIMIT_VALUE, 1));
		let before = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);

	}: {
		crate::Pallet::<T>::ensure_add_liquidty_limit(asset_id.into(), 0u128.into(), 10u128.into())
	}
	verify {
		let after = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);
		assert!(before != after);
	}

	ensure_remove_liquidity_limit {
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = Some((crate::MAX_LIMIT_VALUE, 1));
		let before = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);
	}: {
		crate::Pallet::<T>::ensure_remove_liquidity_limit(asset_id.into(), 0u128.into(), 10u128.into())
	}
	verify {
		let after = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);
		assert!(before != after);
	}

	ensure_pool_state_change_limit {
		let asset_in_id = T::AssetId::from(2u32);
		let asset_in_reserve = 100_000_000_000_000u128;
		let amount_in= 10_000_000_000_000u128;
		let asset_out_id = T::AssetId::from(3u32);
		let asset_out_reserve = 200_000_000_000_000u128;
		let amount_out = 10_000_000_000_000u128;
		let before_in = AllowedTradeVolumeLimitPerAsset::<T>::get(asset_in_id);
		let before_out = AllowedTradeVolumeLimitPerAsset::<T>::get(asset_out_id);
	}: {
		crate::Pallet::<T>::ensure_pool_state_change_limit(asset_in_id.into(), asset_in_reserve.into(), amount_in.into(), asset_out_id.into(), asset_out_reserve.into(), amount_out.into())
	}
	verify {
		let after_in = AllowedTradeVolumeLimitPerAsset::<T>::get(asset_in_id);
		let after_out = AllowedTradeVolumeLimitPerAsset::<T>::get(asset_out_id);

		assert!(before_in != after_in);
		assert!(before_out != after_out);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);

}
