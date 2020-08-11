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
use frame_support::sp_runtime::traits::Saturating;
use sp_std::prelude::*;
use system::RawOrigin;

use primitives::{AssetId, Balance};

const SEED: u32 = 0;
const MAX_EXISTENTIAL_DEPOSIT: u32 = 1000;
const MAX_USER_INDEX: u32 = 1000;

benchmarks! {
	_ {
		let e in 2 .. MAX_EXISTENTIAL_DEPOSIT => ();
		let u in 1 .. MAX_USER_INDEX => ();
	}

	create_pool {
		let u in ...;
		let e in ...;

		let caller = account("caller", u, SEED);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 100;
		let initial_price : Balance = 10;

	}: _(RawOrigin::Signed(caller), asset_a, asset_b, amount, initial_price)
}
