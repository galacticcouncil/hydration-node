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

const TVL_CAP: Balance = 222_222_000_000_000_000_000_000;

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

		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

	}: _(RawOrigin::Root, stable_price, native_price, Permill::from_percent(100), Permill::from_percent(100))
	verify {
		assert!(<Assets<T>>::get(T::StableCoinAssetId::get()).is_some());
		assert!(<Assets<T>>::get(T::HdxAssetId::get()).is_some());
	}

	add_token{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);
		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: T::AccountId = account("owner", 0, 1);

		let token_price: FixedU128= FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &acc, token_amount as i128)?;

		let current_position_id = <NextPositionId<T>>::get();

	}: _(RawOrigin::Root, token_id, token_price,Permill::from_percent(100), owner)
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
		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: T::AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &acc, token_amount as i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <NextPositionId<T>>::get();

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
		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price,native_price,Permill::from_percent(100), Permill::from_percent(100))?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let owner: T::AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &acc, token_amount as i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <NextPositionId<T>>::get();

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
		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price,native_price,Permill::from_percent(100), Permill::from_percent(100))?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let owner: T::AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &acc, token_amount as i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <NextPositionId<T>>::get();

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
		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price,native_price,Permill::from_percent(100), Permill::from_percent(100))?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let owner: T::AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &acc, token_amount as i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <NextPositionId<T>>::get();

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
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price,native_price,Permill::from_percent(100), Permill::from_percent(100))?;

	}: _(RawOrigin::Root, T::StableCoinAssetId::get(), Tradability::BUY)
	verify {
		let asset_state = <Assets<T>>::get(T::StableCoinAssetId::get()).unwrap();
		assert!(asset_state.tradable == Tradability::BUY);
	}

	refund_refused_asset{
		let recipient: T::AccountId = account("recipient", 3, 1);

		let asset_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;
		let amount = 1_000_000_000_000_000u128;

		T::Currency::update_balance(asset_id, &Pallet::<T>::protocol_account(), amount as i128)?;

	}: _(RawOrigin::Root, asset_id, amount, recipient.clone() )
	verify {
		assert!(T::Currency::free_balance(asset_id, &recipient) == amount);
	}

	sacrifice_position{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);
		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

		// Register new asset in asset registry
		let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: T::AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		T::Currency::update_balance(token_id, &acc, token_amount as i128)?;

		// Add the token to the pool
		crate::Pallet::<T>::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance
		let lp_provider: T::AccountId = account("provider", 1, 1);
		T::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

		let liquidity_added = 300_000_000_000_000u128;

		let current_position_id = <NextPositionId<T>>::get();

		crate::Pallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), token_id, liquidity_added)?;

	}: _(RawOrigin::Signed(lp_provider), current_position_id)
	verify {
		assert!(<Positions<T>>::get(current_position_id).is_none());
	}

	set_asset_weight_cap{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = crate::Pallet::<T>::protocol_account();

		crate::Pallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		T::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
		T::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

		crate::Pallet::<T>::initialize_pool(RawOrigin::Root.into(), stable_price,native_price,Permill::from_percent(100), Permill::from_percent(100))?;

	}: _(RawOrigin::Root, T::StableCoinAssetId::get(), Permill::from_percent(10))
	verify {
		let asset_state = <Assets<T>>::get(T::StableCoinAssetId::get()).unwrap();
		assert!(asset_state.cap == 100_000_000_000_000_000u128);
	}

}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
