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

benchmarks! {
	 where_clause {  where T::AssetId: From<u32>,
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config
	}

	initialize_pool{
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

	}: _(RawOrigin::Root, stable_amount, native_amount, stable_price, native_price)
	verify {
		assert!(<Assets<T>>::get(T::StableCoinAssetId::get()).is_some());
		assert!(<Assets<T>>::get(T::NativeAssetId::get()).is_some());
	}

	add_token{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);
		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount, native_amount, stable_price, native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let caller: T::AccountId = account("caller", 0, 1);

		let token_price: FixedU128= FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		let current_position_id = <PositionInstanceSequencer<T>>::get();

	}: _(RawOrigin::Signed(caller), token_id, token_amount, token_price)
	verify {
		assert!(<Positions<T>>::get(current_position_id).is_some());
		assert!(<Assets<T>>::get(token_id).is_some());
	}

	add_liquidity{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount, native_amount, stable_price, native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let caller: T::AccountId = account("caller", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Signed(caller).into(), token_id,token_amount, token_price)?;

		// Create LP provider account with correct balance
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <PositionInstanceSequencer<T>>::get();

	}: _(RawOrigin::Signed(lp_provider), token_id, liquidity_added)
	verify {
		assert!(<Positions<T>>::get(current_position_id).is_some());
	}

	remove_liquidity{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let caller: T::AccountId = account("caller", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Signed(caller).into(), token_id,token_amount, token_price)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <PositionInstanceSequencer<T>>::get();

		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), token_id, liquidity_added)?;

		// to ensure worst case - Let's do a trade to make sure price changes, so LP provider receives some LRNA ( which does additional transfer)
		let buyer: T::AccountId = account("buyer", 2, 1);
		T::Currency::update_balance(T::StableCoinAssetId::get(), &buyer, 500_000_000_000_000i128)?;
		crate::Pallet::<T>::buy(RawOrigin::Signed(buyer).into(), token_id, T::StableCoinAssetId::get(), 30_000_000_000_000u128, 100_000_000_000_000u128)?;

	}: _(RawOrigin::Signed(lp_provider.clone()), current_position_id, liquidity_added)
	verify {
		// Ensure NFT instance was burned
		assert!(<Positions<T>>::get(current_position_id).is_none());

		// Ensure lp provider received LRNA
		assert!(T::Currency::free_balance(T::HubAssetId::get(), &lp_provider) > Balance::zero());
	}

	sell{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let caller: T::AccountId = account("caller", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Signed(caller).into(), token_id,token_amount, token_price)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <PositionInstanceSequencer<T>>::get();

		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

		let buyer: T::AccountId = account("buyer", 2, 1);
		T::Currency::update_balance(T::StableCoinAssetId::get(), &buyer, 500_000_000_000_000i128)?;
		crate::Pallet::<T>::buy(RawOrigin::Signed(buyer).into(), token_id, T::StableCoinAssetId::get(), 30_000_000_000_000u128, 100_000_000_000_000u128)?;

		let seller: T::AccountId = account("seller", 3, 1);
		T::Currency::update_balance(token_id, &seller, 500_000_000_000_000i128)?;

		let amount_sell = 100_000_000_000_000u128;
		let buy_min_amount = 10_000_000_000_000u128;

	}: _(RawOrigin::Signed(seller.clone()), token_id, T::StableCoinAssetId::get(), amount_sell, buy_min_amount)
	verify {
		assert!(T::Currency::free_balance(T::StableCoinAssetId::get(), &seller) >= buy_min_amount);
	}

	buy{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let caller: T::AccountId = account("caller", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &caller, 500_000_000_000_000i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Signed(caller).into(), token_id,token_amount, token_price)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <PositionInstanceSequencer<T>>::get();

		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

		let buyer: T::AccountId = account("buyer", 2, 1);
		T::Currency::update_balance(T::StableCoinAssetId::get(), &buyer, 500_000_000_000_000i128)?;
		crate::Pallet::<T>::buy(RawOrigin::Signed(buyer).into(), token_id, T::StableCoinAssetId::get(), 30_000_000_000_000u128, 100_000_000_000_000u128)?;

		let seller: T::AccountId = account("seller", 3, 1);
		T::Currency::update_balance(token_id, &seller, 500_000_000_000_000i128)?;

		let amount_buy = 10_000_000_000_000u128;
		let sell_max_limit = 200_000_000_000_000u128;

	}: _(RawOrigin::Signed(seller.clone()), T::StableCoinAssetId::get(), token_id, amount_buy, sell_max_limit)
	verify {
		assert!(T::Currency::free_balance(T::StableCoinAssetId::get(), &seller) >= Balance::zero());
	}

	set_asset_tradable_state{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_amount,native_amount,stable_price,native_price)?;

	}: _(RawOrigin::Root, T::StableCoinAssetId::get(), Tradable::BuyOnly)
	verify {
		let asset_state = <Assets<T>>::get(T::StableCoinAssetId::get()).unwrap();
		assert!(asset_state.tradable == Tradable::BuyOnly);
	}
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
