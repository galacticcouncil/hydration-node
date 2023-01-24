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

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

pub const ONE: Balance = 1_000_000_000_000;

fn schedule_fake<T: Config>() -> Schedule<T::Asset, T::BlockNumber>
where
	T: crate::pallet::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: Into<u32>,
	<T as crate::pallet::Config>::Asset: From<u32>,
	<T as crate::pallet::Config>::Asset: Into<u32>,
	<T as pallet_omnipool::Config>::AssetId: Into<<T as crate::pallet::Config>::Asset>,
	<T as pallet_omnipool::Config>::AssetId: From<<T as crate::pallet::Config>::Asset>,
{
	let schedule1: Schedule<T::Asset, T::BlockNumber> = Schedule {
		period: 3u32.into(),
		recurrence: Recurrence::Perpetual,
		order: Order::Buy {
			asset_in: 2u32.into(),
			asset_out: 3u32.into(),
			amount_out: ONE,
			max_limit: Balance::MAX,
			route: create_bounded_vec::<T>(vec![]),
		},
	};
	schedule1
}
pub fn create_bounded_vec<T: Config>(trades: Vec<Trade<T::Asset>>) -> BoundedVec<Trade<T::Asset>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<T::Asset>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

benchmarks! {
	 where_clause {  where
		T: crate::pallet::Config,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<u32>,
		<T as crate::pallet::Config>::Asset: From<u32>,
		<T as crate::pallet::Config>::Asset: Into<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<<T as crate::pallet::Config>::Asset>,
		<T as pallet_omnipool::Config>::AssetId: From<<T as crate::pallet::Config>::Asset>
	}

	schedule{
		let caller: T::AccountId = account("provider", 1, 1);
		let schedule1 = schedule_fake::<T>();

		assert!(true);

	}: _(RawOrigin::Signed(caller.clone()), schedule1, Option::None)
	verify {

	}


}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
