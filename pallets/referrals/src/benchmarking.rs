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
use frame_support::traits::tokens::fungibles::{Inspect, Mutate};
use frame_system::RawOrigin;
use sp_std::vec;

benchmarks! {
	where_clause { where
		T::Currency: Mutate<T::AccountId>,
		T::AssetId: From<u32>,
	}

	register_code{
		let caller: T::AccountId = account("caller", 0, 1);
		let code: ReferralCode<T::CodeLength> = vec![b'x'; T::CodeLength::get() as usize].try_into().unwrap();
		let (asset, fee, _) = T::RegistrationFee::get();
		T::Currency::mint_into(asset, &caller, 2 * fee)?;

	}: _(RawOrigin::Signed(caller.clone()), code.clone())
	verify {
		let entry = Pallet::<T>::referrer_level(caller.clone());
		assert_eq!(entry, Some((Level::Tier0, 0)));
		let c = Pallet::<T>::normalize_code(code);
		let entry = Pallet::<T>::referral_account(c);
		assert_eq!(entry, Some(caller));
	}

	link_code{
		let caller: T::AccountId = account("caller", 0, 1);
		let user: T::AccountId = account("user", 0, 1);
		let code: ReferralCode<T::CodeLength> = vec![b'x'; T::CodeLength::get() as usize].try_into().unwrap();
		let (asset, fee, _) = T::RegistrationFee::get();
		T::Currency::mint_into(asset, &caller, 2 * fee)?;
		Pallet::<T>::register_code(RawOrigin::Signed(caller.clone()).into(), code.clone())?;
	}: _(RawOrigin::Signed(user.clone()), code)
	verify {
		let entry = Pallet::<T>::linked_referral_account(user);
		assert_eq!(entry, Some(caller));
	}

	convert{
		let caller: T::AccountId = account("caller", 0, 1);
		let (asset_id, amount) = T::BenchmarkHelper::prepare_convertible_asset_and_amount();
		T::Currency::mint_into(asset_id.clone(), &Pallet::<T>::pot_account_id(), amount)?;
		PendingConversions::<T>::insert(asset_id.clone(),());
		let count = PendingConversions::<T>::count();
		assert_eq!(count , 1);
	}: _(RawOrigin::Signed(caller), asset_id.clone())
	verify {
		let count = PendingConversions::<T>::count();
		assert_eq!(count , 0);
		let balance = T::Currency::balance(asset_id, &Pallet::<T>::pot_account_id());
		assert_eq!(balance, 0);
	}

	claim_rewards{
		let caller: T::AccountId = account("caller", 0, 1);
		let code: ReferralCode<T::CodeLength> = vec![b'x'; T::CodeLength::get() as usize].try_into().unwrap();
		let (asset, fee, _) = T::RegistrationFee::get();
		T::Currency::mint_into(asset, &caller, 2 * fee)?;
		Pallet::<T>::register_code(RawOrigin::Signed(caller.clone()).into(), code)?;
		let caller_balance = T::Currency::balance(T::RewardAsset::get(), &caller);

		// The worst case is when referrer account is updated to the top tier in one call
		// So we need to have enough RewardAsset in the pot. And give all the shares to the caller.
		let top_tier_volume = T::LevelVolumeAndRewardPercentages::get(&Level::Tier4).0;
		T::Currency::mint_into(T::RewardAsset::get(), &Pallet::<T>::pot_account_id(), 2 * top_tier_volume + T::SeedNativeAmount::get())?;
		ReferrerShares::<T>::insert(caller.clone(), 1_000_000_000_000);
		TraderShares::<T>::insert(caller.clone(), 1_000_000_000_000);
		TotalShares::<T>::put(2_000_000_000_000);
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		let count = PendingConversions::<T>::count();
		assert_eq!(count , 0);
		let balance = T::Currency::balance(T::RewardAsset::get(), &caller);
		assert!(balance > caller_balance);
		let (level, total) = Referrer::<T>::get(&caller).expect("correct entry");
		assert_eq!(level, Level::Tier4);
		assert_eq!(total, top_tier_volume);
	}

	set_reward_percentage{
		let referrer_percentage = Permill::from_percent(40);
		let trader_percentage = Permill::from_percent(30);
		let external_percentage = Permill::from_percent(30);
	}: _(RawOrigin::Root, T::RewardAsset::get(), Level::Tier2, FeeDistribution{referrer: referrer_percentage, trader: trader_percentage, external: external_percentage})
	verify {
		let entry = Pallet::<T>::asset_rewards(T::RewardAsset::get(), Level::Tier2);
		assert_eq!(entry, Some(FeeDistribution{
			referrer: referrer_percentage,
			trader: trader_percentage,
			external: external_percentage,
		}));
	}
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::*;
	use frame_benchmarking::impl_benchmark_test_suite;
	impl_benchmark_test_suite!(
		Pallet,
		super::ExtBuilder::default().with_default_volumes().build(),
		super::Test
	);
}
