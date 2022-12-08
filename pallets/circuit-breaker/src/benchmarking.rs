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
use frame_system::RawOrigin;
use sp_std::prelude::*;

benchmarks! {
	 where_clause {
		where T::AssetId: From<u32>,
	}

	set_trade_volume_limit {
		let asset_id = T::AssetId::from(1u32);
		let trade_limit = (crate::MAX_TRADE_VOLUME_LIMIT, 1);

	}: _(RawOrigin::Root, asset_id, trade_limit)
	verify {
		// assert_eq!(T::Currency::free_balance(asset_a, &caller), 999990000000000);
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
			assert_ok!(Pallet::<Test>::test_benchmark_set_trade_volume_limit());
		});
	}
}
