// This file is part of Basilisk-node

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

use crate::types::AssetAmount;
use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::traits::EnsureOrigin;
use frame_system::{Pallet as System, RawOrigin};
use hydradx_traits::router::{PoolType, TradeExecution};
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::Permill;

const ASSET_ID_OFFSET: u32 = 2_000;

// Stable benchmarks
// Worst case scenarios in any stableswap calculations are scenarios where "math" does max number of iterations.
// Therefore, hydra-dx-math build with "runtime-benchmarks" features forces calculations of D and Y to perform all iterations.
// it is no longer needed to come up with some extreme scenario where it would do as many as iterations as possible.
// As it is, it would not be possible to come up with scenarios where D/Y does not converge( or does max iterations).
benchmarks! {
	 where_clause {  where T::AssetId: From<u32> + Into<u32>,
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config,
		T::AssetId: From<u32>
	}

	create_pool {
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL{
			let asset_id = idx + ASSET_ID_OFFSET;
			T::BenchmarkHelper::register_asset(asset_id.into(), 12)?;
			asset_ids.push(asset_id.into());
		}
		let pool_id = 1000u32;
		T::BenchmarkHelper::register_asset(pool_id.into(), 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let caller: T::AccountId = account("caller", 0, 1);
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
	}: _<T::RuntimeOrigin>(successful_origin, pool_id.into(), asset_ids, amplification, trade_fee)
	verify {
		assert!(<Pools<T>>::get::<T::AssetId>(pool_id.into()).is_some());
	}

	add_liquidity{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, 1_000_000_000_000_000i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, 1_000_000_000_000_000_000_000i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}

		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;

		// Worst case is adding additional liquidity and not initial liquidity
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;
	}: _(RawOrigin::Signed(lp_provider.clone()), pool_id, added_liquidity)
	verify {
		assert!(T::Currency::free_balance(pool_id, &lp_provider) > 0u128);
	}

	add_liquidity_shares{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, 1_000_000_000_000_000i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, 1_000_000_000_000_000_000_000i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}

		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		let asset_id: T::AssetId = *asset_ids.last().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;

		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;
		let desired_shares = 1198499641600967085948u128;
	}: _(RawOrigin::Signed(lp_provider.clone()), pool_id, desired_shares,asset_id, 1221886049851226)
	verify {
		assert_eq!(T::Currency::free_balance(pool_id, &lp_provider), desired_shares);
		assert_eq!(T::Currency::free_balance(asset_id, &lp_provider), 999998791384905220210);
	}

	remove_liquidity_one_asset{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;

		let asset_id_to_withdraw: T::AssetId = *asset_ids.last().unwrap();
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;

		// Worst case is adding additional liquidity and not initial liquidity
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(),
			pool_id,
			added_liquidity
		)?;

		// just make sure that LP provided all his liquidity of this asset
		assert_eq!(T::Currency::free_balance(asset_id_to_withdraw, &lp_provider), 0u128);
		let shares = T::Currency::free_balance(pool_id, &lp_provider);
	}: _(RawOrigin::Signed(lp_provider.clone()), pool_id, asset_id_to_withdraw, shares, 0)
	verify {
		assert_eq!(T::Currency::free_balance(pool_id, &lp_provider), 0u128);
		assert_eq!(T::Currency::free_balance(asset_id_to_withdraw, &lp_provider), 1_492_491_167_377_362);
	}

	withdraw_asset_amount{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;

		let asset_id_to_withdraw: T::AssetId = *asset_ids.last().unwrap();
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;

		// Worst case is adding additional liquidity and not initial liquidity
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(),
			pool_id,
			added_liquidity
		)?;

		// just make sure that LP provided all his liquidity of this asset
		assert_eq!(T::Currency::free_balance(asset_id_to_withdraw, &lp_provider), 0u128);
		let shares = T::Currency::free_balance(pool_id, &lp_provider);
	}: _(RawOrigin::Signed(lp_provider.clone()), pool_id, asset_id_to_withdraw, liquidity_added, shares)
	verify {
		let shares_remaining = T::Currency::free_balance(pool_id, &lp_provider);
		assert!(shares_remaining < shares);
		assert_eq!(T::Currency::free_balance(asset_id_to_withdraw, &lp_provider), liquidity_added);
	}

	sell{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let asset_in: T::AssetId = *asset_ids.last().unwrap();
		let asset_out: T::AssetId = *asset_ids.first().unwrap();
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;

		let seller : T::AccountId = account("seller", 0, 1);
		let amount_sell  = 100_000_000_000_000u128;
		T::Currency::update_balance(asset_in, &seller, amount_sell as i128)?;
		let buy_min_amount = 1_000u128;
		// Worst case is when amplification is changing
		crate::Pallet::<T>::update_amplification(RawOrigin::Root.into(),
			pool_id,
			1000,
			100u32.into(),
			1000u32.into(),
		)?;
		System::<T>::set_block_number(500u32.into());
	}: _(RawOrigin::Signed(seller.clone()), pool_id, asset_in, asset_out, amount_sell, buy_min_amount)
	verify {
		assert_eq!(T::Currency::free_balance(asset_in, &seller), 0u128);
		assert_eq!(T::Currency::free_balance(asset_out, &seller), 98_999_980_239_523);
	}

	buy{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let asset_in: T::AssetId = *asset_ids.last().unwrap();
		let asset_out: T::AssetId = *asset_ids.first().unwrap();
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;

		let buyer: T::AccountId = account("buyer", 0, 1);
		T::Currency::update_balance(asset_in, &buyer, 100_000_000_000_000i128)?;
		let amount_buy = 10_000_000_000_000u128;
		let sell_max_limit = 11_000_000_000_000u128;
		// Worst case is when amplification is changing
		crate::Pallet::<T>::update_amplification(RawOrigin::Root.into(),
			pool_id,
			1000,
			100u32.into(),
			1000u32.into(),
		)?;
		System::<T>::set_block_number(500u32.into());
	}: _(RawOrigin::Signed(buyer.clone()), pool_id, asset_out, asset_in, amount_buy, sell_max_limit)
	verify {
		assert_eq!(T::Currency::free_balance(asset_out, &buyer), 10_000_000_000_000);
		assert_eq!(T::Currency::free_balance(asset_in, &buyer), 89_899_999_798_401);
	}

	set_asset_tradable_state {
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let asset_to_change = asset_ids[0];
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin.clone(),
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;

		let asset_tradability_old = crate::Pallet::<T>::asset_tradability(pool_id, asset_to_change,);
	}: _<T::RuntimeOrigin>(successful_origin, pool_id, asset_to_change, Tradability::FROZEN)
	verify {
		let asset_tradability_new = crate::Pallet::<T>::asset_tradability(pool_id, asset_to_change,);
		assert_ne!(asset_tradability_old, asset_tradability_new);
	}

	update_pool_fee{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin.clone(),
			pool_id,
			asset_ids,
			100u16,
			Permill::from_percent(1),
		)?;

		let new_fee = Permill::from_percent(50);
	}: _<T::RuntimeOrigin>(successful_origin, pool_id, new_fee)
	verify {
		let pool = crate::Pallet::<T>::pools(pool_id).unwrap();
		assert_eq!(pool.fee, new_fee);
	}

	update_amplification{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin.clone(),
			pool_id,
			asset_ids,
			100u16,
			Permill::from_percent(1),
		)?;

		// Worst case is when amplification is changing
		crate::Pallet::<T>::update_amplification(RawOrigin::Root.into(),
			pool_id,
			1000,
			100u32.into(),
			1000u32.into(),
		)?;

		System::<T>::set_block_number(500u32.into());

	}: _<T::RuntimeOrigin>(successful_origin, pool_id, 5000, 501u32.into(), 1000u32.into())
	verify {
		let pool = crate::Pallet::<T>::pools(pool_id).unwrap();

		assert_eq!(pool.initial_amplification, NonZeroU16::new(500).unwrap());
		assert_eq!(pool.final_amplification, NonZeroU16::new(5000).unwrap());
		assert_eq!(pool.initial_block, 501u32.into());
		assert_eq!(pool.final_block, 1000u32.into());
	}

	router_execution_sell{
		let c in 1..2;
		let e in 0..1;	// if e == 1, execute_sell is executed

		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let asset_in: T::AssetId = *asset_ids.last().unwrap();
		let asset_out: T::AssetId = *asset_ids.first().unwrap();
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;

		let seller : T::AccountId = account("seller", 0, 1);
		let amount_sell  = 100_000_000_000_000u128;
		T::Currency::update_balance(asset_in, &seller, amount_sell as i128)?;
		let buy_min_amount = 1_000u128;
		// Worst case is when amplification is changing
		crate::Pallet::<T>::update_amplification(RawOrigin::Root.into(),
			pool_id,
			1000,
			100u32.into(),
			1000u32.into(),
		)?;
		System::<T>::set_block_number(500u32.into());
	}: {
		assert!(<crate::Pallet::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance>>::calculate_sell(PoolType::Stableswap(pool_id), asset_in, asset_out, amount_sell).is_ok());
		if e != 0 {
			assert!(<crate::Pallet::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance>>::execute_sell(RawOrigin::Signed(seller.clone()).into(), PoolType::Stableswap(pool_id), asset_in, asset_out, amount_sell, buy_min_amount).is_ok());
		}
	}
	verify {
		if e != 0 {
			assert_eq!(T::Currency::free_balance(asset_in, &seller), 0u128);
			assert_eq!(T::Currency::free_balance(asset_out, &seller), 98_999_980_239_523);
		}
	}

	router_execution_buy{
		let c in 1..2;	// number of times calculate_buy is executed
		let e in 0..1;	// if e == 1, execute_buy is executed

		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<T::AssetId>> = vec![];
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
			T::BenchmarkHelper::register_asset(asset_id, 12)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, initial_liquidity as i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id: T::AssetId = (1000u32).into();
		T::BenchmarkHelper::register_asset(pool_id, 18)?;
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let asset_in: T::AssetId = *asset_ids.last().unwrap();
		let asset_out: T::AssetId = *asset_ids.first().unwrap();
		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;

		let buyer: T::AccountId = account("buyer", 0, 1);
		T::Currency::update_balance(asset_in, &buyer, 100_000_000_000_000i128)?;
		let amount_buy = 10_000_000_000_000u128;
		let sell_max_limit = 11_000_000_000_000u128;
		// Worst case is when amplification is changing
		crate::Pallet::<T>::update_amplification(RawOrigin::Root.into(),
			pool_id,
			1000,
			100u32.into(),
			1000u32.into(),
		)?;
		System::<T>::set_block_number(500u32.into());
	}: {
		for _ in 1..c {
			assert!(<crate::Pallet::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance>>::calculate_buy(PoolType::Stableswap(pool_id), asset_in, asset_out, amount_buy).is_ok());
		}
		if e != 0 {
			assert!(<crate::Pallet::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, T::AssetId, Balance>>::execute_buy(RawOrigin::Signed(buyer.clone()).into(), PoolType::Stableswap(pool_id), asset_in, asset_out, amount_buy, sell_max_limit).is_ok());
		}
	}
	verify {
		if e != 0 {
			assert_eq!(T::Currency::free_balance(asset_out, &buyer), 10_000_000_000_000);
			assert_eq!(T::Currency::free_balance(asset_in, &buyer), 89_899_999_798_401);
		}
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
