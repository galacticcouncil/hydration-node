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

use crate::types::BenchmarkHelper;
use frame_benchmarking::{account, benchmarks};
use frame_support::{assert_ok, traits::Hooks};
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
		where
			T::AssetId: From<u32>,
			T: pallet_timestamp::Config<Moment = u64>
	}

	on_initialize_skip_lockdown_lifting {
		let block_num: BlockNumberFor<T> = 1u32.into();
		frame_system::Pallet::<T>::set_block_number(block_num);
	}: { Pallet::<T>::on_initialize(block_num); }
	verify {}

	on_initialize_lift_lockdown {
		let block_num = frame_system::Pallet::<T>::block_number();

		let until = crate::Pallet::<T>::timestamp_now() + (primitives::constants::time::MILLISECS_PER_BLOCK * 4);
		assert_ok!(crate::Pallet::<T>::set_global_withdraw_lockdown(RawOrigin::Root.into(), until));

		pallet_timestamp::Pallet::<T>::set_timestamp(until);
		assert!(crate::Pallet::<T>::withdraw_lockdown_until().is_some());
	}: { Pallet::<T>::on_initialize(block_num); }
	verify {
		assert!(crate::Pallet::<T>::withdraw_lockdown_until().is_none());
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

	set_global_withdraw_limit {
		let balance = T::Balance::from(1_000_000u32);
	}: _(RawOrigin::Root, balance)
	verify {
		assert_eq!(crate::Pallet::<T>::global_withdraw_limit(), Some(balance));
	}

	reset_withdraw_lockdown {
		let now = crate::Pallet::<T>::timestamp_now();
		let init_value = (T::Balance::from(1_000_000u32), now - 1);
		WithdrawLimitAccumulator::<T>::put(init_value);
		WithdrawLockdownUntil::<T>::put(now);
	}: _(RawOrigin::Root)
	verify {
		assert!(crate::Pallet::<T>::withdraw_lockdown_until().is_none());
		assert_eq!(crate::Pallet::<T>::withdraw_limit_accumulator(), (T::Balance::zero(), now));
	}

	set_global_withdraw_lockdown {
		let until = crate::Pallet::<T>::timestamp_now() + 1;
	}: _(RawOrigin::Root, until)
	verify {
		assert_eq!(crate::Pallet::<T>::withdraw_lockdown_until(), Some(until));
	}

	add_egress_accounts {
		let n in 0 .. 100;
		let mut accounts: Vec<T::AccountId> = Vec::with_capacity(n as usize);
		for i in 0..n {
			// deterministic accounts; any method ok
			let acc: T::AccountId = frame_benchmarking::account("egress", i, 0);
			accounts.push(acc);
		}
	}: _(RawOrigin::Root, accounts) // or the proper AuthorityOrigin if not Root
	verify {
		// spot check: last inserted exists
		if n > 0 {
			let last = frame_benchmarking::account::<T::AccountId>("egress", n-1, 0);
			assert!(EgressAccounts::<T>::contains_key(last));
		}
	}

	remove_egress_accounts {
		let n in 0 .. 100;
		let mut accounts: Vec<T::AccountId> = Vec::with_capacity(n as usize);
		for i in 0..n {
			let acc: T::AccountId = frame_benchmarking::account("egress", i, 0);
			EgressAccounts::<T>::insert(&acc, ());
			accounts.push(acc);
		}
	}: _(RawOrigin::Root, accounts)
	verify {
		if n > 0 {
			let last = frame_benchmarking::account::<T::AccountId>("egress", n-1, 0);
			assert!(!EgressAccounts::<T>::contains_key(last));
		}
	}

	set_asset_category {
		let asset_id = T::AssetId::from(0u32);
		let expected_category = Some(GlobalAssetCategory::Local);
	}: _(RawOrigin::Root, asset_id, expected_category.clone())
	verify {
		assert_eq!(crate::Pallet::<T>::global_asset_overrides(asset_id), expected_category);
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


	lockdown_asset {
		let account: T::AccountId = account("seller", 0, 0);

		let asset = T::AssetId::from(1);

		let state = AssetLockdownState::<T>::get(asset);
		assert_eq!(state, None);
	}: {
		crate::Pallet::<T>::lockdown_asset(RawOrigin::Root.into(), asset, 100u32.into())?
	}
	verify {
		let state = AssetLockdownState::<T>::get(asset);

		assert_eq!(state, Some(LockdownStatus::Locked(100u32.into())));
	}

	force_lift_lockdown {
		frame_system::Pallet::<T>::set_block_number(1u32.into());

		let account: T::AccountId = account("seller", 0, 0);

		let asset = T::AssetId::from(95);
		T::BenchmarkHelper::register_asset(asset, 100_000_000_000_000u128.into())?;

		T::BenchmarkHelper::deposit(account,asset, 101_000_000_000_000u128.into())?;

		let period: u32 = <T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Period::get().try_into().unwrap();
				let delay = period + 1u32;

		let state = AssetLockdownState::<T>::get(asset).unwrap();
		assert_eq!(state, LockdownStatus::Locked(delay.into()));
	}: {
		let bn = frame_system::Pallet::<T>::block_number();

		crate::Pallet::<T>::force_lift_lockdown(RawOrigin::Root.into(), asset)?
	}
	verify {
		let state = AssetLockdownState::<T>::get(asset);
		assert_eq!(state, Some(LockdownStatus::Unlocked((1u32.into(), 101_000_000_000_000u128.into()))));
	}

	release_deposit {
		frame_system::Pallet::<T>::set_block_number(1u32.into());

		let account: T::AccountId = account("seller", 0, 0);

		let asset = T::AssetId::from(95);
		T::BenchmarkHelper::register_asset(asset, 100_000_000_000_000u128.into())?;

		T::BenchmarkHelper::deposit(account.clone(),asset, 101_000_000_000_000u128.into())?;
		let period: u32 = <T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Period::get().try_into().unwrap();
		let delay = period + 1u32;
		let state = AssetLockdownState::<T>::get(asset);
		assert_eq!(state, Some(LockdownStatus::Locked(delay.into())));

		let lockdown_over = delay + 1u32;
		frame_system::Pallet::<T>::set_block_number(lockdown_over.into());
		T::BenchmarkHelper::deposit(account.clone(),asset, 1_000_000_000_000u128.into())?;//we need this to remove lockdown

		let state = AssetLockdownState::<T>::get(asset);
		assert_eq!(state, Some(LockdownStatus::Unlocked((lockdown_over.into(), 101_000_000_000_000u128.into()))));

	}: {
		crate::Pallet::<T>::release_deposit(RawOrigin::Root.into(), account, asset)?
	}
	verify {
		//No verify as if successfull, the extrinsic completed
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
