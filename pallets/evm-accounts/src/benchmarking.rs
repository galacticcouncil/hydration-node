// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

benchmarks! {
	where_clause {
		where T::AccountId: AsRef<[u8; 32]> + frame_support::pallet_prelude::IsType<AccountId32>,
	}

	bind_evm_address {
		let user: T::AccountId = account("user", 0, 1);
		let evm_address = Pallet::<T>::evm_address(&user);
		assert!(!BoundAccount::<T>::contains_key(evm_address));

	}: _(RawOrigin::Signed(user.clone()))
	verify {
		let evm_address = Pallet::<T>::evm_address(&user);
		assert!(BoundAccount::<T>::contains_key(evm_address));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
