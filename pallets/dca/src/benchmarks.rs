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

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_traits::Registry;
use orml_traits::MultiCurrencyExtended;
use scale_info::prelude::vec::Vec;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

pub const ONE: Balance = 1_000_000_000_000;

fn schedule_fake<T: Config + pallet_omnipool::Config>(
	owner: T::AccountId,
	asset_in: T::Asset,
	asset_out: T::Asset,
	amount: Balance,
) -> Schedule<T::AccountId, T::Asset, T::BlockNumber> {
	let schedule1: Schedule<T::AccountId, T::Asset, T::BlockNumber> = Schedule {
		owner,
		period: 3u32.into(),
		total_amount: 500 * ONE,
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_limit: Balance::MAX,
			route: create_bounded_vec::<T>(vec![]),
		},
	};
	schedule1
}

fn get_named_reseve_balance<T: Config + pallet_omnipool::Config>(token_id: T::Asset, seller: T::AccountId) -> Balance {
	<T as Config>::Currency::reserved_balance_named(&T::NamedReserveId::get(), token_id, &seller.clone())
}

fn schedule_sell_fake<T: Config + pallet_omnipool::Config>(
	owner: T::AccountId,
	asset_in: T::Asset,
	asset_out: T::Asset,
	amount: Balance,
) -> Schedule<T::AccountId, T::Asset, T::BlockNumber> {
	let schedule1: Schedule<T::AccountId, T::Asset, T::BlockNumber> = Schedule {
		owner,
		period: 3u32.into(),
		total_amount: 500 * ONE,
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_limit: Balance::MIN,
			route: create_bounded_vec::<T>(vec![]),
		},
	};
	schedule1
}

pub fn create_bounded_vec<T: Config>(trades: Vec<Trade<T::Asset>>) -> BoundedVec<Trade<T::Asset>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<T::Asset>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

type CurrencyOf<T> = <T as pallet_omnipool::Config>::Currency;
type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;

fn prepare_omnipool<T: pallet_omnipool::Config>() -> Result<T::AssetId, DispatchError>
where
	CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	OmnipoolPallet::<T>::set_tvl_cap(RawOrigin::Root.into(), u128::MAX)?;

	// Initialize pool
	let stable_amount: Balance = 1_000_000_000_000_000u128;
	let native_amount: Balance = 1_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);
	let acc = OmnipoolPallet::<T>::protocol_account();

	<T as pallet_omnipool::Config>::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
	<T as pallet_omnipool::Config>::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

	OmnipoolPallet::<T>::initialize_pool(
		RawOrigin::Root.into(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(100),
	)?;

	// Register new asset in asset registry
	let token_id = T::AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

	// Create account for token provider and set balance
	let owner: T::AccountId = account("owner", 0, 1);

	let token_price = FixedU128::from((1, 5));
	let token_amount = 200_000_000_000_000u128;

	<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &acc, token_amount as i128)?;

	// Add the token to the pool
	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		token_id,
		token_price,
		Permill::from_percent(100),
		owner,
	)?;

	// Create LP provider account with correct balance aand add some liquidity
	let lp_provider: T::AccountId = account("provider", 1, 1);
	<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &lp_provider, 500_000_000_000_000i128)?;

	let liquidity_added = 300_000_000_000_000u128;

	OmnipoolPallet::<T>::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

	let buyer: T::AccountId = account("buyer", 2, 1);
	<T as pallet_omnipool::Config>::Currency::update_balance(
		T::StableCoinAssetId::get(),
		&buyer,
		500_000_000_000_000i128,
	)?;
	OmnipoolPallet::<T>::buy(
		RawOrigin::Signed(buyer).into(),
		token_id,
		T::StableCoinAssetId::get(),
		30_000_000_000_000u128,
		100_000_000_000_000u128,
	)?;

	Ok(token_id)
}

fn create_account_with_native_balance<T: Config + pallet_omnipool::Config>() -> Result<T::AccountId, DispatchError>
where
	CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config + pallet_omnipool::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let caller: T::AccountId = account("provider", 1, 1);
	let token_amount = 200 * ONE;
	<T as pallet_omnipool::Config>::Currency::update_balance(0.into(), &caller, token_amount as i128)?;

	Ok(caller)
}

benchmarks! {
	 where_clause {  where
		CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
		T: crate::pallet::Config + pallet_omnipool::Config,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<<T as crate::pallet::Config>::Asset>,
		<T as pallet_omnipool::Config>::AssetId: From<<T as crate::pallet::Config>::Asset>,
	}

	on_initialize_with_empty_block{
		let token_id = prepare_omnipool::<T>()?;

		let seller: T::AccountId = account("seller", 3, 1);

		let amount_sell = 1_000_000_000_000u128;
		let sell_max_limit = 200_000_000_000_000u128;

		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &seller, 2_000_000_000_000_000_000_000_000i128)?;
		<T as pallet_omnipool::Config>::Currency::update_balance(0u32.into(), &seller, 500_000_000_000_000i128)?;

		let execution_block = 100u32;

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller),0);
	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
		//TODO: it fails with buy limit not reached, check out why, maybe the schedule is not correct
	}
	verify {
		//TODO: Add assertion
		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller), 0);
	}

	on_initialize{
		//Prepare omnipool
		let UNITS = 1_000_000_000_000u64;
		let omnipool_account =OmnipoolPallet::<T>::protocol_account();
		let stable_amount = 50_000 * UNITS * 1_000_000;
		let native_amount = 936_329_588_000_000_000u64;
		let dot_amount = 87_719_298_250_000_u128;
		let eth_amount = 63_750_000_000_000_000_000u128;
		let btc_amount = 1_000_000_000u128;

		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);

		<T as pallet_omnipool::Config>::Currency::update_balance(T::StableCoinAssetId::get(), &omnipool_account, stable_amount as i128)?;
		<T as pallet_omnipool::Config>::Currency::update_balance(T::HdxAssetId::get(), &omnipool_account, native_amount as i128)?;

		assert_ok!(OmnipoolPallet::<T>::set_tvl_cap(
			RawOrigin::Root.into(),
			522_222_000_000_000_000_000_000,
		));

		assert_ok!(OmnipoolPallet::<T>::initialize_pool(
			RawOrigin::Root.into(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));

		let token_id = T::AssetRegistry::create_asset(&b"DOT".to_vec(), 1u128)?;
		let token_amount = 200_000_000_000_000u128;

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);
		let owner: T::AccountId = account("owner", 0, 1);
		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &omnipool_account, token_amount as i128)?;

		assert_ok!(OmnipoolPallet::<T>::add_token(
			RawOrigin::Root.into(),
			token_id,
			token_price,
			Permill::from_percent(100),
			owner,
		));

		let token_id = T::AssetRegistry::create_asset(&b"DAI".to_vec(), 1u128)?;
		let token_amount = 200_000_000_000_000u128;

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);
		let owner: T::AccountId = account("owner", 0, 1);
		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &omnipool_account, token_amount as i128)?;

		assert_ok!(OmnipoolPallet::<T>::add_token(
			RawOrigin::Root.into(),
			token_id,
			token_price,
			Permill::from_percent(100),
			owner,
		));

		let seller: T::AccountId = account("seller", 3, 1);

		let amount_sell = 20_000_000_000_000u128;
		let sell_max_limit = 200_000_000_000_000u128;

		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &seller, 20_000_000_000_000_000i128)?;
		<T as pallet_omnipool::Config>::Currency::update_balance(0u32.into(), &seller, 500_000_000_000_000i128)?;

		let schedule1 = schedule_sell_fake::<T>(seller.clone(), token_id.into(),T::StableCoinAssetId::get().into(), amount_sell);
		let execution_block = 100u32;

		let max_schedules_per_block: u128 = T::MaxSchedulePerBlock::get().into();

		for _ in 0..max_schedules_per_block {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));
		}

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller.clone()),0);
		let reserved_balance = get_named_reseve_balance::<T>(token_id.into(), seller.clone());

		let init_reserved_balance = 10000000000000000;
		assert_eq!(init_reserved_balance, reserved_balance);
	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		let reserved_balance = get_named_reseve_balance::<T>(token_id.into(), seller.clone());
		let asset_in_spent_on_all_trades = max_schedules_per_block * amount_sell;
		assert_eq!(init_reserved_balance - asset_in_spent_on_all_trades, reserved_balance);
		assert!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller) > 0);
	}

	/*on_initialize{
		let token_id = prepare_omnipool::<T>()?;

		let seller: T::AccountId = account("seller", 3, 1);

		let amount_sell = 20_000_000_000_000u128;
		let sell_max_limit = 200_000_000_000_000u128;

		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &seller, 2_000_000_000_000_000_000_000_000i128)?;
		<T as pallet_omnipool::Config>::Currency::update_balance(0u32.into(), &seller, 500_000_000_000_000i128)?;

		let schedule1 = schedule_sell_fake::<T>(seller.clone(), token_id.into(),T::StableCoinAssetId::get().into(), amount_sell);
		let execution_block = 100u32;

		for _ in 0..T::MaxSchedulePerBlock::get() {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));
		}
		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller),0);
	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		//TODO: Add assertion
		assert!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller) > 0);
	}*/

	schedule{
		let token_id = prepare_omnipool::<T>()?;

		let caller: T::AccountId = create_account_with_native_balance::<T>()?;
		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &caller, 100_000_000_000_000_000i128)?;

		let amount_sell = 20_000_000_000_000u128;
		let schedule1 = schedule_fake::<T>(caller.clone(), token_id.into(), T::StableCoinAssetId::get().into(), amount_sell);
		let execution_block = 100u32;
		let one_block_after_execution_block = execution_block + 1;

		//We fill blocks with schedule leaving only one place
		let number_of_all_schedules = T::MaxSchedulePerBlock::get() + T::MaxSchedulePerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(caller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));
		}

		let schedule_id : ScheduleId = number_of_all_schedules;

	}: _(RawOrigin::Signed(caller.clone()), schedule1, Option::Some(execution_block.into()))
	verify {
		assert!(<Schedules<T>>::get::<ScheduleId>(schedule_id).is_some());
		assert_eq!(20, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>(execution_block.into()).len());
		assert_eq!(20, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + 1u32).into()).len());
		assert_eq!(20, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + 3u32).into()).len());
		assert_eq!(20, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + 7u32).into()).len());
		assert_eq!(20, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + 15u32).into()).len());
		assert_eq!(20, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + 31u32).into()).len());
	}

	/* TODO: we might add back terminate, so leaving it here as commented
	terminate {
		let token_id = prepare_omnipool::<T>()?;
		let caller: T::AccountId = create_account_with_native_balance::<T>()?;
		<T as pallet_omnipool::Config>::Currency::update_balance(token_id, &caller, 100_000_000_000_000_000i128)?;

		let amount_sell = 20_000_000_000_000u128;
		let schedule1 = schedule_fake::<T>(caller.clone(), token_id.into(), T::StableCoinAssetId::get().into(), amount_sell);
		let schedule_id : ScheduleId = 1;
		let execution_block = 100u32;
		assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(caller.clone()).into(), schedule1, Option::Some(execution_block.into())));

	}: _(RawOrigin::Signed(caller.clone()), schedule_id, Option::Some(execution_block.into()))
	verify {
		assert!(<Schedules<T>>::get::<ScheduleId>(schedule_id).is_none());
	}*/

}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(
		Pallet,
		super::ExtBuilder::default().with_omnipool_trade(true).build(),
		super::Test
	);
}
