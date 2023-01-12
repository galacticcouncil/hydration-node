// This file is part of HydraDX-node

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

use sp_runtime::FixedU128;

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::Permill;

const TVL_CAP: Balance = 222_222_000_000_000_000_000_000;

/*
benchmarks! {
	 where_clause {  where
		CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_stableswap::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<<T as pallet_stableswap::Config>::AssetId>,
		<T as pallet_omnipool::Config>::AssetId: From<<T as pallet_stableswap::Config>::AssetId>
	}
	/*
	create_subpool{

		let share_asset = 1u32;
		let asset_a = 1u32;
		let asset_b = 1u32;
		let cap = Permill::from_percent(100);
		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let withdraw_fee = Permill::from_percent(1);

	}: _(RawOrigin::Root, share_asset.into(), asset_a.into(), asset_b.into(), cap, amplification, trade_fee, withdraw_fee)
	verify {
	}
	 */
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}


 */
