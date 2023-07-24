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

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use sp_std::vec::Vec;

use hydradx_traits::Registry;
use primitives::constants::time::unix_time::MONTH;

pub const NOW: Moment = 1689844300000; // unix time in milliseconds
pub const ONE: u128 = 1_000_000_000_000;
pub const HDX: u32 = 0;

benchmarks! {
	 where_clause {
		where
		T::AssetRegistry: Registry<T::AssetId, Vec<u8>, T::Balance, DispatchError>,
		T: Config,
		T: pallet_timestamp::Config,
		T::AssetId: From<u32> + Into<u32>,
		T::Balance: From<u32> + From<u128>,
		T::TimestampProvider: Time<Moment = Moment>,
		T::Moment: From<u64>
	}

	issue {
		pallet_timestamp::Pallet::<T>::set_timestamp(NOW.into());
		let issuer: T::AccountId = account("caller", 0, 1);
		let amount: T::Balance = (200 * ONE).into();
		let maturity = 1893452400000u64; // 1.1.2030
		let maturity = NOW + T::MinMaturity::get();

		T::Currency::deposit(HDX.into(), &issuer, amount)?;

	}: _(RawOrigin::Signed(issuer), HDX.into(), (100 * ONE).into(), maturity)
	verify {
		let bond_id = RegisteredBonds::<T>::iter_keys().next().unwrap();
		assert!(crate::Pallet::<T>::bonds(bond_id).is_some());
	}

	redeem {
		pallet_timestamp::Pallet::<T>::set_timestamp(NOW.into());

		let issuer: T::AccountId = account("caller", 0, 1);
		let amount: T::Balance = (200 * ONE).into();
		T::Currency::deposit(HDX.into(), &issuer, amount)?;

		let maturity = NOW + T::MinMaturity::get();

		assert_ok!(crate::Pallet::<T>::issue(RawOrigin::Signed(issuer.clone()).into(), HDX.into(), amount, maturity));

		let fee = <T as Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: T::Balance = amount.checked_sub(&fee).unwrap();

		pallet_timestamp::Pallet::<T>::set_timestamp((NOW + MONTH).into());

		let bond_id = RegisteredBonds::<T>::iter_keys().next().unwrap();

	}: _(RawOrigin::Signed(issuer), bond_id, amount_without_fee)
	verify {
		assert!(crate::Pallet::<T>::bonds(bond_id).is_none());
	}

	unlock {
		pallet_timestamp::Pallet::<T>::set_timestamp(NOW.into());

		let issuer: T::AccountId = account("caller", 0, 1);
		let amount: T::Balance = (200 * ONE).into();
		T::Currency::deposit(HDX.into(), &issuer, amount)?;

		let maturity = NOW + T::MinMaturity::get();

		assert_ok!(crate::Pallet::<T>::issue(RawOrigin::Signed(issuer).into(), HDX.into(), amount, maturity));

		let fee = <T as Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: T::Balance = amount.checked_sub(&fee).unwrap();

		pallet_timestamp::Pallet::<T>::set_timestamp((NOW + T::MinMaturity::get() / 2).into());

		let bond_id = RegisteredBonds::<T>::iter_keys().next().unwrap();

	}: _(RawOrigin::Root, bond_id)
	verify {
		let bond_data = crate::Pallet::<T>::bonds(bond_id).unwrap();
		assert_eq!(bond_data.maturity, NOW + T::MinMaturity::get() / 2);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
