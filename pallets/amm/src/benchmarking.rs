// This file is part of Substrate.

// Copyright (C) 2020 Parity Technologies (UK) Ltd.
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

use crate::Module as AMM;

use primitives::{AssetId, Balance, Price};

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
		let amount : Balance = 10 * 1_000_000_000;
		let initial_price : Price = Price::from(2);

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, initial_price)
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

		AMM::<T>::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a,asset_b, 1_000_000_000, Price::from(1))?;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, max_limit)
	verify {
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999990000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999990000000000);
	}

	remove_liquidity {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1_000_000_000;

		AMM::<T>::create_pool(RawOrigin::Signed(maker.clone()).into(), 1, 2, 10_000_000_000, Price::from(2))?;
		AMM::<T>::add_liquidity(RawOrigin::Signed(caller.clone()).into(), 1, 2, 5_000_000_000, 10_000_000_000)?;

		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999995000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999990000000000);

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount)
	verify {
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999996000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999992000000000);
	}

	sell {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1 * 1_000_000_000;
		let discount = false;

		let min_bought: Balance = 10 * 1_000;

		AMM::<T>::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, asset_b, 1 * 1_000_000_000_000, Price::from(3))?;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, min_bought, discount)
	verify{
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 999999000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 1000002991014968);
	}

	buy {
		let maker = funded_account::<T>("maker", 0);
		let caller = funded_account::<T>("caller", 0);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 1 * 1_000_000_000;
		let discount = false;

		let max_sold: Balance = 6_000_000_000;

		AMM::<T>::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, asset_b, 1 * 1_000_000_000_000, Price::from(3))?;

	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, max_sold, discount)
	verify{
		assert_eq!(T::Currency::free_balance(asset_a, &caller), 1000001000000000);
		assert_eq!(T::Currency::free_balance(asset_b, &caller), 999996990984966);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_create_pool::<Test>());
			assert_ok!(test_benchmark_add_liquidity::<Test>());
			assert_ok!(test_benchmark_remove_liquidity::<Test>());
			assert_ok!(test_benchmark_sell::<Test>());
			assert_ok!(test_benchmark_buy::<Test>());
		});
	}
}
