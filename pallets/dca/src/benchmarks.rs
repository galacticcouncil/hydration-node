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
use frame_system::{Pallet as System, RawOrigin};
use hydradx_traits::Registry;
use orml_traits::MultiCurrencyExtended;
use scale_info::prelude::vec::Vec;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

pub type AssetId = u32;

pub const TVL_CAP: Balance = 222_222_000_000_000_000_000_000;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const BTC: AssetId = 3;

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
		total_amount: 2000 * ONE,
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

fn set_period<T: Config + pallet_omnipool::Config>(to: u32)
where
	T: pallet_ema_oracle::Config,
	CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	<T as pallet::Config>::Asset: From<u32>,
{
	while System::<T>::block_number() < to.into() {
		let b = System::<T>::block_number();

		System::<T>::on_finalize(b);
		pallet_ema_oracle::Pallet::<T>::on_finalize(b);

		System::<T>::on_initialize(b + 1_u32.into());
		pallet_ema_oracle::Pallet::<T>::on_initialize(b + 1_u32.into());

		System::<T>::set_block_number(b + 1_u32.into());
	}
}

pub fn create_bounded_vec<T: Config>(trades: Vec<Trade<T::Asset>>) -> BoundedVec<Trade<T::Asset>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<T::Asset>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

type CurrencyOf<T> = <T as pallet_omnipool::Config>::Currency;
type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;
type FrameSystem<T> = frame_system::Pallet<T>;
type EmaOracle<T> = pallet_ema_oracle::Pallet<T>;

fn initialize_omnipool<T: Config + pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: pallet_ema_oracle::Config,
	T::Asset: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let stable_amount: Balance = 1_000_000_000_000_000_000_u128;
	let native_amount: Balance = 1_000_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);
	let acc = OmnipoolPallet::<T>::protocol_account();

	OmnipoolPallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

	<T as pallet_omnipool::Config>::Currency::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128)?;
	<T as pallet_omnipool::Config>::Currency::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128)?;

	OmnipoolPallet::<T>::initialize_pool(
		RawOrigin::Root.into(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(100),
	)?;

	//NOTE: This is necessary for oracle to provide price.
	do_lrna_hdx_trade::<T>()?;
	do_lrna_dai_trade::<T>()?;

	set_period::<T>(10);

	do_lrna_dai_trade::<T>()?;
	do_lrna_hdx_trade::<T>()
}

const SEED: u32 = 0;
fn create_funded_account<T: Config + pallet_omnipool::Config>(
	name: &'static str,
	index: u32,
	amount: Balance,
	currency: <T as pallet_omnipool::Config>::AssetId,
) -> T::AccountId
where
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let caller: T::AccountId = account(name, index, SEED);

	fund::<T>(caller.clone(), currency, amount).unwrap();

	caller
}

fn fund<T: Config + pallet_omnipool::Config>(
	to: T::AccountId,
	currency: <T as pallet_omnipool::Config>::AssetId,
	amount: Balance,
) -> DispatchResult {
	CurrencyOf::<T>::deposit(currency, &to, amount)
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_hdx_trade<T: Config + pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T::Asset: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, HDX.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), HDX.into(), ONE, 0)
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_dai_trade<T: Config + pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T::Asset: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, DAI.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), DAI.into(), ONE, 0)
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
		T: crate::pallet::Config + pallet_omnipool::Config + pallet_ema_oracle::Config,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as Config>::Asset: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<<T as crate::pallet::Config>::Asset>,
		<T as pallet_omnipool::Config>::AssetId: From<<T as crate::pallet::Config>::Asset>,
	}

	on_initialize{
		//Prepare omnipool
		initialize_omnipool::<T>()?;
		set_period::<T>(1000);
		let seller: T::AccountId = account("seller", 3, 1);

		let amount_sell = 20 * ONE;

		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &seller, 20_000_000_000_000_000_000_000i128)?;
		<T as pallet_omnipool::Config>::Currency::update_balance(0u32.into(), &seller, 500_000_000_000_000i128)?;

		let schedule1 = schedule_sell_fake::<T>(seller.clone(), HDX.into(), DAI.into(), amount_sell);
		let execution_block = 1001u32;

		let max_schedules_per_block: u128 = T::MaxSchedulePerBlock::get().into();

		for _ in 0..max_schedules_per_block {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));
		}

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller.clone()),0);
		let reserved_balance = get_named_reseve_balance::<T>(HDX.into(), seller.clone());

		let init_reserved_balance = 40000000000000000;
		assert_eq!(init_reserved_balance, reserved_balance);

		let init_native_balance = 0;
		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(DAI.into(), &seller), init_native_balance);


	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		let reserved_balance = get_named_reseve_balance::<T>(HDX.into(), seller.clone());
		let asset_in_spent_on_all_trades = max_schedules_per_block * amount_sell;
		assert_eq!(init_reserved_balance - asset_in_spent_on_all_trades, reserved_balance);

		let init_native_balance = 0;
		assert!(<T as pallet_omnipool::Config>::Currency::free_balance(HDX.into(), &seller) > init_native_balance);
	}


	on_initialize_with_empty_block{
		initialize_omnipool::<T>()?;

		let seller: T::AccountId = account("seller", 3, 1);

		let execution_block = 100u32;
		assert_eq!(crate::Pallet::<T>::schedules::<ScheduleId>(execution_block.into()).len(), 0);
		let mut weight = Weight::from_ref_time(0);
	}: {
		weight = crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		assert!(weight.ref_time() > 0u64);
	}

	schedule{
		initialize_omnipool::<T>()?;

		let caller: T::AccountId = create_account_with_native_balance::<T>()?;
		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &caller, 100_000_000_000_000_000i128)?;

		let amount_sell = 20_000_000_000_000u128;
		let schedule1 = schedule_fake::<T>(caller.clone(), HDX.into(), DAI.into(), amount_sell);
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
