// This file is part of HydraDX-node.

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
use frame_system::RawOrigin;
use sp_std::prelude::*;

use crate::Pallet as XYK;

use crate::types::{AssetId, Balance};
use hydradx_traits::router::{PoolType, TradeExecution};

const SEED: u32 = 1;

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);
	T::Currency::update_balance(1, &caller, 1_000_000_000_000_000).unwrap();
	T::Currency::update_balance(2, &caller, 1_000_000_000_000_000).unwrap();
	caller
}

benchmarks! {
	create_pool {
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount_a : Balance = 10 * 1_000_000_000;
		let amount_b : Balance = 20 * 1_000_000_000;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, amount_a, asset_b, amount_b)
	verify {
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999990000000000);
	}

	add_liquidity {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10 * 1_000_000_000;
		let max_limit : Balance = 10 * 1_000_000_000_000;

		XYK::<T>::create_pool(RawOrigin::Signed(maker).into(), asset_a, 1_000_000_000,asset_b, 1_000_000_000)?;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, max_limit)
	verify {
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999990000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999990000000000 - 1); // Due to rounding in favor of pool
	}

	remove_liquidity {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1_000_000_000;

		XYK::<T>::create_pool(RawOrigin::Signed(maker).into(), 1, 10_000_000_000, 2, 20_000_000_000)?;
		XYK::<T>::add_liquidity(RawOrigin::Signed(caller.clone()).into(), 1, 2, 5_000_000_000, 10_100_000_000)?;

		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999995000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999990000000000 - 1);// Due to rounding in favor of pool

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount)
	verify {
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999996000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999992000000000 - 1);// Due to rounding in favor of pool
	}

	sell {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1_000_000_000;
		let discount = false;

		let min_bought: Balance = 10 * 1_000;

		XYK::<T>::create_pool(RawOrigin::Signed(maker).into(), asset_a, 1_000_000_000_000, asset_b, 3_000_000_000_000)?;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, min_bought, discount)
	verify{
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999999000000000);
	}

	buy {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1_000_000_000;
		let discount = false;

		let max_sold: Balance = 6_000_000_000;

		XYK::<T>::create_pool(RawOrigin::Signed(maker).into(), asset_a, 1_000_000_000_000, asset_b, 3_000_000_000_000)?;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, max_sold, discount)
	verify{
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 1000001000000000);
	}

	router_execution_sell {
		let c in 1..2;	// if c == 1, calculate_sell is executed
		let e in 0..1;	// if e == 1, execute_sell is executed

		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1_000_000_000;
		let discount = false;

		let min_bought: Balance = 10 * 1_000;

		XYK::<T>::create_pool(RawOrigin::Signed(maker).into(), asset_a, 1_000_000_000_000, asset_b, 3_000_000_000_000)?;

	}: {
		for _ in 1..c {
			assert!(<XYK::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::calculate_sell(PoolType::XYK, asset_a, asset_b, amount).is_ok());
		}
		if e != 0 {
			assert!(<XYK::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::execute_sell(RawOrigin::Signed(caller.clone()).into(), PoolType::XYK, asset_a, asset_b, amount, min_bought).is_ok());
		}
	}
	verify{
		if e != 0 {
			assert_eq!(T::Currency::free_balance(asset_a, &caller), 999999000000000);
		}
	}

	router_execution_buy {
		let c in 1..3;	// number of times calculate_buy is executed
		let e in 0..1;	// if e == 1, execute_buy is executed

		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1_000_000_000;
		let discount = false;

		let max_sold: Balance = 6_000_000_000;

		XYK::<T>::create_pool(RawOrigin::Signed(maker).into(), asset_a, 1_000_000_000_000, asset_b, 3_000_000_000_000)?;

	}: {
		for _ in 1..c {
			assert!(<XYK::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::calculate_buy(PoolType::XYK, asset_a, asset_b, amount).is_ok());
		}
		if e != 0 {
			assert!(<XYK::<T> as TradeExecution<T::RuntimeOrigin, T::AccountId, AssetId, Balance>>::execute_buy(RawOrigin::Signed(caller.clone()).into(), PoolType::XYK, asset_a, asset_b, amount, max_sold).is_ok());
		}
	}
	verify{
		if e != 0 {
			assert_eq!(T::Currency::free_balance(asset_b, &caller), 1000001000000000);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::mock::{ExtBuilder, System, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		ExtBuilder::default().build().execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Pallet::<Test>::test_benchmark_create_pool());
			assert_ok!(Pallet::<Test>::test_benchmark_add_liquidity());
			assert_ok!(Pallet::<Test>::test_benchmark_remove_liquidity());
			assert_ok!(Pallet::<Test>::test_benchmark_sell());
			assert_ok!(Pallet::<Test>::test_benchmark_buy());
			assert_ok!(Pallet::<Test>::test_benchmark_router_execution_sell());
			assert_ok!(Pallet::<Test>::test_benchmark_router_execution_buy());
		});
	}
}
