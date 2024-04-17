// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;

pub const ONE: Balance = 1_000_000_000_000;
pub const HDX: u32 = 0;
pub const DAI: u32 = 2;

benchmarks! {
	where_clause { where
		AssetIdOf<T>: From<u32>,
		<T as crate::Config>::Currency: MultiCurrency<T::AccountId>,
		T: crate::Config,
		T: pallet_otc::Config,
		u32: From<AssetIdOf<T>>,
		<T as pallet_otc::Config>::AssetRegistry: Create<Balance, Error=DispatchError, AssetId = AssetIdOf<T>>,
	}
	settle_otc_order {
		let (dot, dai) = (HDX, DAI);
		let account: T::AccountId = account("acc", 1, 1);


		<T as crate::Config>::Currency::deposit(dot.into(), &account, 1_000_000_000 * ONE)?;
		<T as crate::Config>::Currency::deposit(dai.into(), &account, 1_000_000_000 * ONE)?;

		assert_ok!(
			pallet_otc::Pallet::<T>::place_order(RawOrigin::Signed(account).into(), dot.into(), dai.into(), 100_000_000 * ONE, 200_000_001 * ONE, true)
		);

		let route = <T as crate::Config>::Router::get_route(AssetPair {
			asset_in: dai.into(),
			asset_out: dot.into(),
		});

  }:  _(RawOrigin::None, 0u32, 2 * ONE, route)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build().0, super::Test);
}
