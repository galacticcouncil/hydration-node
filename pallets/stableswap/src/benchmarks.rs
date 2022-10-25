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

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::pallet_prelude::DispatchError;
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::Permill;

use hydradx_traits::Registry;

use crate::types::{AssetLiquidity, Balance};

// Stable benchmarks
// Worst case scenarios in any stableswap calculations are scenarios where "math" does max number of iterations.
// Therefore, hydra-dx-math build with "runtime-benchmarks" features forces calculations of D and Y to perform all iterations.
// it is no longer needed to come up with some extreme scenario where it would do as many as iterations as possible.
// As it is, it would not be possible to come up with scenarios where D/Y does not converge( or does max iterations).

benchmarks! {
	 where_clause {  where T::AssetId: From<u32> + Into<u32>,
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T::AssetRegistry: Registry<T::AssetId, Vec<u8>, Balance, DispatchError>,
		T: crate::pallet::Config,
		T::AssetId: From<u32>
	}

	create_pool {
		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			let asset_id = T::AssetRegistry::create_asset(&name, 1u128)?;
			asset_ids.push(asset_id);
		}
		// Pool id will be next asset id in registry storage
		let next_asset_id: u32 = (*(asset_ids.last()).unwrap()).into() + 1u32;
		let pool_id: T::AssetId = next_asset_id.into();

		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let withdraw_fee = Permill::from_percent(1);
		let caller: T::AccountId = account("caller", 0, 1);
	}: _(RawOrigin::Signed(caller), asset_ids, amplification, trade_fee, withdraw_fee)
	verify {
		assert!(<Pools<T>>::get(pool_id).is_some());
	}

	add_liquidity{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetLiquidity<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetLiquidity<T::AssetId>> = vec![];

		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			let asset_id = T::AssetRegistry::create_asset(&name, 1u128)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, 1_000_000_000_000_000i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, 1_000_000_000_000_000_000_000i128)?;
			initial.push(AssetLiquidity{
				asset_id,
				amount: initial_liquidity
			});
			added_liquidity.push(AssetLiquidity{
				asset_id,
				amount: liquidity_added
			});
		}
		// Pool id will be next asset id in registry storage
		let next_asset_id: u32 = (*(asset_ids.last()).unwrap()).into() + 1u32;
		let pool_id: T::AssetId = next_asset_id.into();

		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let withdraw_fee = Permill::from_percent(1);

		crate::Pallet::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(),
			asset_ids,
			amplification,
			trade_fee,
			withdraw_fee,
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

	remove_liquidity_one_asset{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetLiquidity<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetLiquidity<T::AssetId>> = vec![];

		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			let asset_id = T::AssetRegistry::create_asset(&name, 1u128)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, 1_000_000_000_000_000i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, liquidity_added as i128)?;
			initial.push(AssetLiquidity{
				asset_id,
				amount: initial_liquidity
			});
			added_liquidity.push(AssetLiquidity{
				asset_id,
				amount: liquidity_added
			});
		}
		// Pool id will be next asset id in registry storage
		let next_asset_id: u32 = (*(asset_ids.last()).unwrap()).into() + 1u32;
		let pool_id: T::AssetId = next_asset_id.into();

		let asset_id_to_withdraw: T::AssetId = *asset_ids.last().unwrap();

		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let withdraw_fee = Permill::from_percent(1);

		crate::Pallet::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(),
			asset_ids,
			amplification,
			trade_fee,
			withdraw_fee,
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

	}: _(RawOrigin::Signed(lp_provider.clone()), pool_id, asset_id_to_withdraw, shares)
	verify {
		assert_eq!(T::Currency::free_balance(pool_id, &lp_provider), 0u128);
		assert_eq!(T::Currency::free_balance(asset_id_to_withdraw, &lp_provider), 1_289_970_212_644_248u128);
	}


	sell{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetLiquidity<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetLiquidity<T::AssetId>> = vec![];

		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			let asset_id = T::AssetRegistry::create_asset(&name, 1u128)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, 1_000_000_000_000_000i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, 1_000_000_000_000_000_000_000i128)?;
			initial.push(AssetLiquidity{
				asset_id,
				amount: initial_liquidity
			});
			added_liquidity.push(AssetLiquidity{
				asset_id,
				amount: liquidity_added
			});
		}
		// Pool id will be next asset id in registry storage
		let next_asset_id: u32 = (*(asset_ids.last()).unwrap()).into() + 1u32;
		let pool_id: T::AssetId = next_asset_id.into();

		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let withdraw_fee = Permill::from_percent(1);

		let asset_in: T::AssetId = *asset_ids.last().unwrap();
		let asset_out: T::AssetId = *asset_ids.first().unwrap();

		crate::Pallet::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(),
			asset_ids,
			amplification,
			trade_fee,
			withdraw_fee,
		)?;

		// Worst case is adding additional liquidity and not initial liquidity
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;

		let seller : T::AccountId = account("seller", 0, 1);
		let amount_sell  = 100_000_000_000_000u128;

		T::Currency::update_balance(asset_in, &seller, amount_sell as i128)?;

		let buy_min_amount = 1_000u128;

	}: _(RawOrigin::Signed(seller.clone()), pool_id, asset_in, asset_out, amount_sell, buy_min_amount)
	verify {
		assert!(T::Currency::free_balance(asset_in, &seller) ==  0u128);
		assert!(T::Currency::free_balance(asset_out, &seller) > 0u128);
	}

	buy{
		let caller: T::AccountId = account("caller", 0, 1);
		let lp_provider: T::AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetLiquidity<T::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetLiquidity<T::AssetId>> = vec![];

		let mut asset_ids: Vec<T::AssetId> = Vec::new() ;
		for idx in 0..MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			let asset_id = T::AssetRegistry::create_asset(&name, 1u128)?;
			asset_ids.push(asset_id);
			T::Currency::update_balance(asset_id, &caller, 1_000_000_000_000_000i128)?;
			T::Currency::update_balance(asset_id, &lp_provider, 1_000_000_000_000_000_000_000i128)?;
			initial.push(AssetLiquidity{
				asset_id,
				amount: initial_liquidity
			});
			added_liquidity.push(AssetLiquidity{
				asset_id,
				amount: liquidity_added
			});
		}
		// Pool id will be next asset id in registry storage
		let next_asset_id: u32 = (*(asset_ids.last()).unwrap()).into() + 1u32;
		let pool_id: T::AssetId = next_asset_id.into();

		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let withdraw_fee = Permill::from_percent(1);

		let asset_in: T::AssetId = *asset_ids.last().unwrap();
		let asset_out: T::AssetId = *asset_ids.first().unwrap();

		crate::Pallet::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(),
			asset_ids,
			amplification,
			trade_fee,
			withdraw_fee,
		)?;

		// Worst case is adding additional liquidity and not initial liquidity
		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial,
		)?;

		let buyer: T::AccountId = account("buyer", 0, 1);

		T::Currency::update_balance(asset_in, &buyer, 100_000_000_000_000i128)?;

		let amount_buy = 10_000_000_000_000u128;
		let sell_max_limit = 20_000_000_000_000_000u128;

	}: _(RawOrigin::Signed(buyer.clone()), pool_id, asset_out, asset_in, amount_buy, sell_max_limit)
	verify {
		assert!(T::Currency::free_balance(asset_out, &buyer) > 0u128);
	}

}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
