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

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, 1);
	caller
}

benchmarks! {
	 where_clause {  where T::AssetId: From<u32>,
		T::Balance: From<u128>,
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config
	}

	initialize_pool{
		let stable_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let native_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

	}: _(RawOrigin::Root, stable_amount, native_amount, stable_price, native_price)
	verify {
	}

	add_token{
		// Initialize pool
		let stable_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let native_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);
		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), T::Balance::from(1u128))?;

		// Create account for token provider and set balance
		let caller = funded_account::<T>("caller", 0);

		let token_price: FixedU128= FixedU128::from((1,5));
		let token_amount = T::Balance::from(200_000_000_000_000u128);

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

	}: _(RawOrigin::Signed(caller), token_id, token_amount, token_price)
	verify {
	}

	add_liquidity{
		// Initialize pool
		let stable_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let native_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), T::Balance::from(1u128))?;

		// Create account for token provider and set balance
		let caller = funded_account::<T>("caller", 0);

		let token_price = FixedU128::from((1,5));
		let token_amount = T::Balance::from(200_000_000_000_000u128);

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Signed(caller).into(), token_id,token_amount, token_price)?;

		// Create LP provider account with correct balance
		let lp_provider = funded_account::<T>("provider", 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = T::Balance::from(300_000_000_000_000u128);

	}: _(RawOrigin::Signed(lp_provider), token_id, liquidity_added)
	verify {
	}

	remove_liquidity{
		// Initialize pool
		let stable_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let native_amount: T::Balance = T::Balance::from(1_000_000_000_000_000u128);
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), T::Balance::from(1u128))?;

		// Create account for token provider and set balance
		let caller = funded_account::<T>("caller", 0);

		let token_price = FixedU128::from((1,5));
		let token_amount = T::Balance::from(200_000_000_000_000u128);

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Signed(caller).into(), token_id,token_amount, token_price)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider = funded_account::<T>("provider", 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = T::Balance::from(300_000_000_000_000u128);

		let current_position_id = <PositionInstanceSequencer<T>>::get();

		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), token_id, liquidity_added)?;


	}: _(RawOrigin::Signed(lp_provider), current_position_id, liquidity_added)
	verify {
		// Ensre NFT instance was burned
		assert!(<Positions<T>>::get(current_position_id).is_none());
	}
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
