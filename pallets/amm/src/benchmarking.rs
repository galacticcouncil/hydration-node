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
use sp_std::prelude::*;
use system::RawOrigin;

use crate::Module as AMM;

use primitives::{AssetId, Balance, Price};

const SEED: u32 = 1;
const MAX_USER_INDEX: u32 = 1000;
const MAX_AMOUNT: u32 = 1_000_000;

fn funded_account<T: Trait>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);
	match T::Currency::update_balance(1, &caller, 1_000_000_000_000_000) {
		_ => {} // let's do nothing here if error, let's just fail the benchmark test ( very rare i would say )
	}
	match T::Currency::update_balance(2, &caller, 1_000_000_000_000_000) {
		_ => {} // let's do nothing here if error, let's just fail the benchmark test ( very rare i would say )
	}
	caller
}

benchmarks! {
	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let a in 1 .. MAX_AMOUNT=> ();
	}

	create_pool {
		let u in ...;
		let a in ...;

		let caller = funded_account::<T>("caller", u);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = a as u128;
		let initial_price : Price = Price::from(10);

	}: _(RawOrigin::Signed(caller), asset_a, asset_b, amount, initial_price)


	add_liquidity {
		let u in ...;

		let caller = funded_account::<T>("caller", u);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 100;
		let max_limit : Balance = 10 * 1_000_000;

		AMM::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(), 1,2, 1000, Price::from(10))?;

	}: _(RawOrigin::Signed(caller), asset_a, asset_b, amount, max_limit)

	remove_liquidity {
		let u in ...;

		let caller = funded_account::<T>("caller", u);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10;

		AMM::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(), 1,2, 1000, Price::from(10))?;

	}: _(RawOrigin::Signed(caller), asset_a, asset_b, amount)

	sell {
		let u in ...;

		let caller = funded_account::<T>("caller", u);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10 * 1_000_000_000;
		let discount = false;

		let min_bought = 80 * 1_000_000_000;

		AMM::<T>::create_pool(RawOrigin::Signed(caller.clone()).into(), asset_a, asset_b, 1 * 1_000_000_000_000, Price::from(10))?;

	}: _(RawOrigin::Signed(caller), asset_a, asset_b, amount, min_bought, discount)
}
