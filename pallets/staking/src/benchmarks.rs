// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_system::{Pallet as System, RawOrigin};
use orml_traits::{MultiCurrencyExtended, MultiLockableCurrency};

const UNIT: u128 = 1_000_000_000_000;

fn init_staking<T: Config>(non_dustable_balance: Balance) -> DispatchResult
where
	T::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
{
	let pot = Pallet::<T>::pot_account_id();
	let hdx = T::HdxAssetId::get();

	T::Currency::update_balance(hdx, &pot, non_dustable_balance as i128)?;
	Pallet::<T>::initialize_staking(RawOrigin::Root.into())
}

fn add_staking_rewards<T: Config>(rewards: Balance) -> DispatchResult
where
	T::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
{
	let pot = Pallet::<T>::pot_account_id();
	let hdx = T::HdxAssetId::get();

	T::Currency::update_balance(hdx, &pot, rewards as i128)
}

fn add_periods<T: Config>(periods: u32) {
	let to = T::PeriodLength::get() * periods.into() + System::<T>::block_number();

	while System::<T>::block_number() < to {
		let b = System::<T>::block_number();

		System::<T>::on_finalize(b);
		System::<T>::on_initialize(b + 1_u32.into());

		System::<T>::set_block_number(b + 1_u32.into());
	}
}

benchmarks! {
	 where_clause { where
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config,
	}

	initialize_staking {
		let pot = Pallet::<T>::pot_account_id();
		let hdx = T::HdxAssetId::get();
		let non_dustable_balance = 1_000 * UNIT;

		T::Currency::update_balance(hdx, &pot, non_dustable_balance as i128)?;

		let successful_origin = T::AuthorityOrigin::try_successful_origin().unwrap();
	}: _<T::RuntimeOrigin>(successful_origin)
	verify {
		assert_eq!(Pallet::<T>::staking(), StakingData {
			accumulated_claimable_rewards: non_dustable_balance,
			..Default::default()
		});
	}

	stake {
		let caller_0: T::AccountId = account("caller", 0, 1);
		let caller_1: T::AccountId = account("caller", 1, 1);
		let hdx = T::HdxAssetId::get();
		let amount = 30_000 * UNIT;

		T::Currency::update_balance(hdx, &caller_0, (100_000 * UNIT) as i128)?;
		T::Currency::update_balance(hdx, &caller_1, (100_000 * UNIT) as i128)?;

		init_staking::<T>(1_000 * UNIT)?;
		Pallet::<T>::stake(RawOrigin::Signed(caller_0).into(), 50_000 * UNIT)?;

		add_staking_rewards::<T>(20_000 * UNIT);
		add_periods::<T>(2);

	}: _(RawOrigin::Signed(caller_1.clone()), amount)
	verify {
		assert!(Pallet::<T>::get_user_position_id(&caller_1)?.is_some())
	}


	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
