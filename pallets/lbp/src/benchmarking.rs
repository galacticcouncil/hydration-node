// This file is part of HydraDX-node.

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

use super::*;

use crate::Pallet as LBP;

use hydradx_traits::router::{PoolType, TradeExecution};

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

const SEED: u32 = 1;

const ASSET_HDX: AssetId = 0;
const ASSET_A_ID: AssetId = 1;
const ASSET_B_ID: AssetId = 2;
const ASSET_A_AMOUNT: Balance = 1_000_000_000;
const ASSET_B_AMOUNT: Balance = 2_000_000_000;
const INITIAL_WEIGHT: LBPWeight = 20_000_000;
const FINAL_WEIGHT: LBPWeight = 90_000_000;

const DEFAULT_FEE: (u32, u32) = (2, 1_000);

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);
	T::MultiCurrency::update_balance(ASSET_HDX, &caller, 1_000_000_000_000_000).unwrap();
	T::MultiCurrency::update_balance(ASSET_A_ID, &caller, 1_000_000_000_000_000).unwrap();
	T::MultiCurrency::update_balance(ASSET_B_ID, &caller, 1_000_000_000_000_000).unwrap();
	caller
}

benchmarks! {
	create_pool {
		let caller = funded_account::<T>("caller", 0);
		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

	}: _(RawOrigin::Root, caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, caller, 0)
	verify {
		assert!(PoolData::<T>::contains_key(&pool_id));
	}

	update_pool_data {
		let caller = funded_account::<T>("caller", 0);
		let fee_collector = funded_account::<T>("fee_collector", 0);
		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);
		let new_start = Some(BlockNumberFor::<T>::from(50_u32));
		let new_end = Some(BlockNumberFor::<T>::from(100_u32));
		let new_initial_weight = 45_250_600;
		let new_final_weight = 55_250_600;
		let fee = (5, 1000);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, fee, caller.clone(), 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

	}: _(RawOrigin::Signed(caller.clone()), pool_id.clone(), Some(caller.clone()), new_start, new_end, Some(new_initial_weight), Some(new_final_weight), Some(DEFAULT_FEE), Some(fee_collector), Some(1))
	verify {
		let pool_data = LBP::<T>::pool_data(pool_id).unwrap();
		assert_eq!(pool_data.start, new_start);
		assert_eq!(pool_data.end, new_end);
		assert_eq!(pool_data.initial_weight, new_initial_weight);
		assert_eq!(pool_data.final_weight, new_final_weight);
	}

	add_liquidity {
		let caller = funded_account::<T>("caller", 0);
		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, caller.clone(), 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

	}: _(RawOrigin::Signed(caller), (ASSET_A_ID, 1_000_000_000_u128), (ASSET_B_ID, 2_000_000_000_u128))
	verify {
		assert_eq!(T::MultiCurrency::free_balance(ASSET_A_ID, &pool_id), 2_000_000_000_u128);
		assert_eq!(T::MultiCurrency::free_balance(ASSET_B_ID, &pool_id), 4_000_000_000_u128);
	}

	remove_liquidity {
		let caller = funded_account::<T>("caller", 0);
		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, caller.clone(), 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

	}: _(RawOrigin::Signed(caller.clone()), pool_id.clone())
	verify {
		assert!(!PoolData::<T>::contains_key(&pool_id));
		assert_eq!(T::MultiCurrency::free_balance(ASSET_A_ID, &caller), 1000000000000000);
		assert_eq!(T::MultiCurrency::free_balance(ASSET_B_ID, &caller), 1000000000000000);
	}

	sell {
		let caller = funded_account::<T>("caller", 0);
		let fee_collector = funded_account::<T>("fee_collector", 0);
		let asset_in: AssetId = ASSET_A_ID;
		let asset_out: AssetId = ASSET_B_ID;
		let amount : Balance = 100_000_000;
		let max_limit: Balance = 10_000_000;

		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, fee_collector, 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

		let start = BlockNumberFor::<T>::from(1u32);
		let end = BlockNumberFor::<T>::from(11u32);

		LBP::<T>::update_pool_data(RawOrigin::Signed(caller.clone()).into(), pool_id, None, Some(start), Some(end), None, None, None, None, None)?;

		frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(2u32));

	}: _(RawOrigin::Signed(caller.clone()), asset_in, asset_out, amount, max_limit)
	verify{
		assert_eq!(T::MultiCurrency::free_balance(asset_in, &caller), 999998900000000);
		assert_eq!(T::MultiCurrency::free_balance(asset_out, &caller), 999998069275212);
	}

	buy {
		let caller = funded_account::<T>("caller", 0);
		let fee_collector = funded_account::<T>("fee_collector", 0);
		let asset_in: AssetId = ASSET_A_ID;
		let asset_out: AssetId = ASSET_B_ID;
		let amount : Balance = 100_000_000;
		let max_limit: Balance = 1_000_000_000;
		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, fee_collector, 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

		let start = BlockNumberFor::<T>::from(1u32);
		let end = BlockNumberFor::<T>::from(11u32);

		LBP::<T>::update_pool_data(RawOrigin::Signed(caller.clone()).into(), pool_id, None, Some(start), Some(end), None, None, None, None, None)?;

		frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(2u32));

	}: _(RawOrigin::Signed(caller.clone()), asset_out, asset_in, amount, max_limit)
	verify{
		assert_eq!(T::MultiCurrency::free_balance(asset_out, &caller), 999998100000000);
		assert_eq!(T::MultiCurrency::free_balance(asset_in, &caller), 999998851241411);
	}

	// The following benchmarks should have similar LBP setup as the corresponding benchmarks
	// for the route executor in the runtime. Otherwise the router weights are incorrect.
	router_execution_sell {
		let c in 1..2;	// if c == 1, calculate_sell is executed
		let e in 0..1;	// if e == 1, execute_sell is executed

		let caller = funded_account::<T>("caller", 0);
		let fee_collector = funded_account::<T>("fee_collector", 0);
		let asset_in: AssetId = ASSET_A_ID;
		let asset_out: AssetId = ASSET_B_ID;
		let amount : Balance = 100_000_000;
		let max_limit: Balance = 10_000_000;

		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, fee_collector, 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

		let start = BlockNumberFor::<T>::from(1u32);
		let end = BlockNumberFor::<T>::from(11u32);

		LBP::<T>::update_pool_data(RawOrigin::Signed(caller.clone()).into(), pool_id, None, Some(start), Some(end), None, None, None, None, None)?;

		frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(2u32));

	}: {
		for _ in 1..c {
			assert!(<LBP::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::calculate_sell(PoolType::LBP, asset_in, asset_out, amount).is_ok());
		}
		if e != 0 {
			assert!(<LBP::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::execute_sell(RawOrigin::Signed(caller.clone()).into(), PoolType::LBP, asset_in, asset_out, amount, max_limit).is_ok());
		}
	}
	verify{
		if e != 0 {
			assert_eq!(T::MultiCurrency::free_balance(asset_in, &caller), 999998900000000);
			assert_eq!(T::MultiCurrency::free_balance(asset_out, &caller), 999998069275212);
		}
	}

	router_execution_buy {
		let c in 1..3;	// number of times calculate_buy is executed
		let e in 0..1;	// if e == 1, execute_buy is executed

		let caller = funded_account::<T>("caller", 0);
		let fee_collector = funded_account::<T>("fee_collector", 0);
		let asset_in: AssetId = ASSET_A_ID;
		let asset_out: AssetId = ASSET_B_ID;
		let amount : Balance = 100_000_000;
		let max_limit: Balance = 1_000_000_000;

		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, fee_collector, 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

		let start = BlockNumberFor::<T>::from(1u32);
		let end = BlockNumberFor::<T>::from(11u32);

		LBP::<T>::update_pool_data(RawOrigin::Signed(caller.clone()).into(), pool_id, None, Some(start), Some(end), None, None, None, None, None)?;

		frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(2u32));

	}: {
		for _ in 1..c {
			assert!(<LBP::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::calculate_buy(PoolType::LBP, asset_in, asset_out, amount).is_ok());
		}
		if e != 0 {
			assert!(<LBP::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::execute_buy(RawOrigin::Signed(caller.clone()).into(), PoolType::LBP, asset_in, asset_out, amount, max_limit).is_ok());
		}
	}
	verify{
		if e != 0 {
			assert_eq!(T::MultiCurrency::free_balance(asset_out, &caller), 999998100000000);
			assert_eq!(T::MultiCurrency::free_balance(asset_in, &caller), 999998851241411);
		}
	}

	// We need to handle this case separately, because for some unknown reason, router_execution_buy provides incorrect weight when just one calculate_buy is executed
	calculate_buy {
		let caller = funded_account::<T>("caller", 0);
		let fee_collector = funded_account::<T>("fee_collector", 0);
		let asset_in: AssetId = ASSET_A_ID;
		let asset_out: AssetId = ASSET_B_ID;
		let amount : Balance = 100_000_000;
		let max_limit: Balance = 1_000_000_000;

		let pool_id = LBP::<T>::pair_account_from_assets(ASSET_A_ID, ASSET_B_ID);

		LBP::<T>::create_pool(RawOrigin::Root.into(), caller.clone(), ASSET_A_ID, ASSET_A_AMOUNT, ASSET_B_ID, ASSET_B_AMOUNT, INITIAL_WEIGHT, FINAL_WEIGHT, WeightCurveType::Linear, DEFAULT_FEE, fee_collector, 0)?;
		ensure!(PoolData::<T>::contains_key(&pool_id), "Pool does not exist.");

		let start = BlockNumberFor::<T>::from(1u32);
		let end = BlockNumberFor::<T>::from(11u32);

		LBP::<T>::update_pool_data(RawOrigin::Signed(caller).into(), pool_id, None, Some(start), Some(end), None, None, None, None, None)?;

		frame_system::Pallet::<T>::set_block_number(BlockNumberFor::<T>::from(2u32));

	}: {
		assert!(<LBP::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::calculate_buy(PoolType::LBP, asset_in, asset_out, amount).is_ok());
	}
	verify{}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(Pallet::<Test>::test_benchmark_create_pool());
			assert_ok!(Pallet::<Test>::test_benchmark_update_pool_data());
			assert_ok!(Pallet::<Test>::test_benchmark_add_liquidity());
			assert_ok!(Pallet::<Test>::test_benchmark_remove_liquidity());
			assert_ok!(Pallet::<Test>::test_benchmark_sell());
			assert_ok!(Pallet::<Test>::test_benchmark_buy());
			assert_ok!(Pallet::<Test>::test_benchmark_router_execution_sell());
			assert_ok!(Pallet::<Test>::test_benchmark_router_execution_buy());
		});
	}
}
