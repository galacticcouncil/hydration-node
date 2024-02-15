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

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Hooks;
use frame_system::RawOrigin;
use sp_std::prelude::*;

fn whitelist_storage_maps<T: Config>() {
	// Whitelist storage map from further DB operations.
	let iter = <AllowedTradeVolumeLimitPerAsset<T>>::iter();
	for (k, _v) in iter {
		let key = <AllowedTradeVolumeLimitPerAsset<T>>::hashed_key_for(k);
		frame_benchmarking::benchmarking::add_to_whitelist(key.into());
	}
	// Whitelist storage map from further DB operations.
	let iter = <AllowedAddLiquidityAmountPerAsset<T>>::iter();
	for (k, _v) in iter {
		let key = <AllowedAddLiquidityAmountPerAsset<T>>::hashed_key_for(k);
		frame_benchmarking::benchmarking::add_to_whitelist(key.into());
	}
	// Whitelist storage map from further DB operations.
	let iter = <AllowedRemoveLiquidityAmountPerAsset<T>>::iter();
	for (k, _v) in iter {
		let key = <AllowedRemoveLiquidityAmountPerAsset<T>>::hashed_key_for(k);
		frame_benchmarking::benchmarking::add_to_whitelist(key.into());
	}
}

benchmarks! {
	 where_clause {
		where T::AssetId: From<u32>,
	}

	on_finalize {
		let n in 0 .. 400;
		let m in 0 .. 400;

		let block_num: BlockNumberFor<T> = 5u32.into();
		frame_system::Pallet::<T>::set_block_number(block_num);

		Pallet::<T>::on_initialize(block_num);

		let amount = T::Balance::from(1_000_000u32);

		for i in 0..n {
			let asset_id = T::AssetId::from(i);
			Pallet::<T>::initialize_trade_limit(asset_id, amount)?;
		}
		for i in 0..m {
			let asset_id = T::AssetId::from(i);
			Pallet::<T>::calculate_and_store_liquidity_limits(asset_id, amount)?;
		}

		whitelist_storage_maps::<T>();
	}: { Pallet::<T>::on_finalize(block_num); }
	verify {}

	on_finalize_single_liquidity_limit_entry {
		let block_num: BlockNumberFor<T> = 5u32.into();
		frame_system::Pallet::<T>::set_block_number(block_num);

		Pallet::<T>::on_initialize(block_num);

		let amount = T::Balance::from(1_000_000u32);
		let asset_id = T::AssetId::from(1);
		Pallet::<T>::calculate_and_store_liquidity_limits(asset_id, amount)?;

		whitelist_storage_maps::<T>();
	}: { Pallet::<T>::on_finalize(block_num); }
	verify {}

	on_finalize_single_trade_limit_entry {
		let block_num: BlockNumberFor<T> = 5u32.into();
		frame_system::Pallet::<T>::set_block_number(block_num);

		Pallet::<T>::on_initialize(block_num);

		let amount = T::Balance::from(1_000_000u32);
		let asset_id = T::AssetId::from(1);
		Pallet::<T>::initialize_trade_limit(asset_id, amount)?;

		whitelist_storage_maps::<T>();
	}: { Pallet::<T>::on_finalize(block_num); }
	verify {}

	on_finalize_empty {
		let block_num: BlockNumberFor<T> = 5u32.into();
		frame_system::Pallet::<T>::set_block_number(block_num);

		Pallet::<T>::on_initialize(block_num);

		whitelist_storage_maps::<T>();
	}: { Pallet::<T>::on_finalize(block_num); }
	verify {}

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

	ensure_add_liquidity_limit {
		let user: T::AccountId = account("user", 0, 1);
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = Some((crate::MAX_LIMIT_VALUE, 1));
		let before = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);

		crate::Pallet::<T>::set_add_liquidity_limit(RawOrigin::Root.into(), asset_id, trade_limit)?;
	}: {
		crate::Pallet::<T>::ensure_add_liquidity_limit(RawOrigin::Signed(user).into(), asset_id, 100u128.into(), 10u128.into())?
	}
	verify {
		let after = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);
		assert!(before != after);
	}

	ensure_remove_liquidity_limit {
		let user: T::AccountId = account("user", 0, 1);
		let asset_id = T::AssetId::from(2u32);
		let trade_limit = Some((crate::MAX_LIMIT_VALUE, 1));
		let before = AllowedAddLiquidityAmountPerAsset::<T>::get(asset_id);
		let initial_liquidity = 100_000_000_000_000u128;
		let removed_liquidity = 100_000_000_000u128;	// 0.1% of initial_liquidity
	}: {
		crate::Pallet::<T>::ensure_remove_liquidity_limit(RawOrigin::Signed(user).into(), asset_id, initial_liquidity.into(), removed_liquidity.into())?
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
		crate::Pallet::<T>::ensure_pool_state_change_limit(asset_in_id, asset_in_reserve.into(), amount_in.into(), asset_out_id, asset_out_reserve.into(), amount_out.into())?
	}
	verify {
		let after_in = AllowedTradeVolumeLimitPerAsset::<T>::get(asset_in_id);
		let after_out = AllowedTradeVolumeLimitPerAsset::<T>::get(asset_out_id);

		assert!(before_in != after_in);
		assert!(before_out != after_out);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
