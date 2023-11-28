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
use frame_support::traits::tokens::fungibles::Mutate;
use frame_system::RawOrigin;
use sp_std::vec;

benchmarks! {
	where_clause { where
		T::Currency: Mutate<T::AccountId>,
		T::AssetId: From<u32>,
	}

	register_code{
		let caller: T::AccountId = account("caller", 0, 1);
		let code = vec![b'x'; T::CodeLength::get() as usize];
		let (asset, fee, _) = T::RegistrationFee::get();
		T::Currency::mint_into(asset, &caller, fee)?;

	}: _(RawOrigin::Signed(caller.clone()), code.clone(), caller.clone())
	verify {
		let entry = Pallet::<T>::referrer_level(caller.clone());
		assert_eq!(entry, Some((Level::Novice, 0)));
		let c = Pallet::<T>::normalize_code(ReferralCode::<T::CodeLength>::truncate_from(code));
		let entry = Pallet::<T>::referral_account(c);
		assert_eq!(entry, Some(caller));
	}

	link_code{
		let caller: T::AccountId = account("caller", 0, 1);
		let user: T::AccountId = account("user", 0, 1);
		let code = vec![b'x'; T::CodeLength::get() as usize];
		let (asset, fee, _) = T::RegistrationFee::get();
		T::Currency::mint_into(asset, &caller, fee)?;
		Pallet::<T>::register_code(RawOrigin::Signed(caller.clone()).into(), code.clone(), caller.clone())?;
	}: _(RawOrigin::Signed(user.clone()), code.clone())
	verify {
		let entry = Pallet::<T>::linked_referral_account(user);
		assert_eq!(entry, Some(caller));
	}

	convert{
		let caller: T::AccountId = account("caller", 0, 1);
		let (asset_id, amount) = T::BenchmarkHelper::prepare_convertible_asset_and_amount();
		T::Currency::mint_into(asset_id.into(), &Pallet::<T>::pot_account_id(), amount)?;
	}: _(RawOrigin::Signed(caller), asset_id.into())
	verify {
	}

}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
