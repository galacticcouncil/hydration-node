// This file is part of Basilisk-node

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

use frame_benchmarking::benchmarks;
use frame_support::{assert_ok, traits::EnsureOrigin};
use frame_system::RawOrigin;

use orml_traits::MultiCurrency;
use primitives::{constants::time::unix_time::MONTH, AssetId, Balance};

pub const NOW: Moment = 1689844300000; // unix time in milliseconds
pub const ONE: Balance = 1_000_000_000_000;
pub const HDX: AssetId = 0;

benchmarks! {
	 where_clause {
		where
		T: Config,
		T: pallet_timestamp::Config,
		T::Balance: From<u32> + From<u128>,
		T::Moment: From<u64>
	}

	issue {
		pallet_timestamp::Pallet::<T>::set_timestamp(NOW.into());

		let origin = T::IssueOrigin::try_successful_origin().unwrap();
		let issuer = T::IssueOrigin::ensure_origin(origin).unwrap();
		let amount: T::Balance = (200 * ONE).into();
		let maturity = NOW + MONTH;

		T::Currency::deposit(HDX, &issuer, amount)?;

	}: _(RawOrigin::Signed(issuer), HDX, (100 * ONE).into(), maturity)
	verify {
		assert!(BondIds::<T>::get::<(AssetId, Moment)>((HDX, maturity)).is_some());
	}

	redeem {
		pallet_timestamp::Pallet::<T>::set_timestamp(NOW.into());

		let origin = T::IssueOrigin::try_successful_origin().unwrap();
		let issuer = T::IssueOrigin::ensure_origin(origin).unwrap();
		let amount: T::Balance = (200 * ONE).into();
		T::Currency::deposit(HDX, &issuer, amount)?;

		let maturity = NOW + MONTH;

		assert_ok!(crate::Pallet::<T>::issue(RawOrigin::Signed(issuer.clone()).into(), HDX, amount, maturity));

		let fee = <T as Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: T::Balance = amount.checked_sub(&fee).unwrap();

		pallet_timestamp::Pallet::<T>::set_timestamp((NOW + 2 * MONTH).into());

		let bond_id = Bonds::<T>::iter_keys().next().unwrap();

	}: _(RawOrigin::Signed(issuer.clone()), bond_id, amount_without_fee)
	verify {
		assert_eq!(T::Currency::free_balance(bond_id, &issuer), 0u32.into());
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
