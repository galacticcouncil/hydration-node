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
#![allow(unused_assignments)] // At test `on_initialize_with_empty_block` it does not recognize the assignment in the Act block
#![allow(dead_code)] //TODO: once we have oracle, use stableswap in the tests, then remove this tag and possible non used code

use super::*;

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_system::{Pallet as System, RawOrigin};
use hydradx_traits::router::PoolType;
use hydradx_traits::Registry;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_stableswap::types::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use scale_info::prelude::vec::Vec;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

pub type AssetId = u32;
pub type AccountId = u64;

pub const TVL_CAP: Balance = 222_222_000_000_000_000_000_000_000;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;

pub const ONE: Balance = 1_000_000_000_000;

// This is the the sum of all "randomly" generated radiuses.
// In tests the radiuses are always the same as we use a fixed parent hash for generation,
// so it will always generate the same values
pub const DELAY_AFTER_LAST_RADIUS: u32 = 1854;

pub const RETRY_TO_SEARCH_FOR_FREE_BLOCK: u32 = 10;

fn schedule_fake<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(
	owner: T::AccountId,
	asset_in: <T as pallet_route_executor::Config>::AssetId,
	asset_out: <T as pallet_route_executor::Config>::AssetId,
	amount: Balance,
) -> Schedule<T::AccountId, <T as pallet_route_executor::Config>::AssetId, T::BlockNumber> {
	let schedule1: Schedule<T::AccountId, <T as pallet_route_executor::Config>::AssetId, T::BlockNumber> = Schedule {
		owner,
		period: 3u32.into(),
		total_amount: 1100 * ONE,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(15)),
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_amount_in: Balance::MAX,
			route: create_bounded_vec::<T>(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]),
		},
	};
	schedule1
}

fn get_named_reseve_balance<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(
	token_id: <T as pallet_route_executor::Config>::AssetId,
	seller: T::AccountId,
) -> Balance {
	<T as Config>::Currencies::reserved_balance_named(&T::NamedReserveId::get(), token_id, &seller)
}

fn schedule_buy_fake<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(
	owner: T::AccountId,
	asset_in: <T as pallet_route_executor::Config>::AssetId,
	asset_out: <T as pallet_route_executor::Config>::AssetId,
	amount: Balance,
	pool: PoolType<<T as pallet_route_executor::Config>::AssetId>,
) -> Schedule<T::AccountId, <T as pallet_route_executor::Config>::AssetId, T::BlockNumber> {
	let schedule1: Schedule<T::AccountId, <T as pallet_route_executor::Config>::AssetId, T::BlockNumber> = Schedule {
		owner,
		period: 3u32.into(),
		total_amount: 2000 * ONE,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(15)),
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_amount_in: Balance::MAX,
			route: create_bounded_vec::<T>(vec![Trade {
				pool,
				asset_in,
				asset_out,
			}]),
		},
	};
	schedule1
}

fn schedule_sell_fake<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(
	owner: T::AccountId,
	asset_in: <T as pallet_route_executor::Config>::AssetId,
	asset_out: <T as pallet_route_executor::Config>::AssetId,
	amount: Balance,
	pool: PoolType<<T as pallet_route_executor::Config>::AssetId>,
) -> Schedule<T::AccountId, <T as pallet_route_executor::Config>::AssetId, T::BlockNumber> {
	let schedule1: Schedule<T::AccountId, <T as pallet_route_executor::Config>::AssetId, T::BlockNumber> = Schedule {
		owner,
		period: 3u32.into(),
		total_amount: 2000 * ONE,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(100)),
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_amount_out: Balance::MIN,
			route: create_bounded_vec::<T>(vec![Trade {
				pool,
				asset_in,
				asset_out,
			}]),
		},
	};
	schedule1
}

fn set_period<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(to: u32)
where
	T: pallet_ema_oracle::Config,
	OmnipoolCurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	<T as pallet_route_executor::Config>::AssetId: From<u32>,
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

pub fn create_bounded_vec<T: Config>(
	trades: Vec<Trade<<T as pallet_route_executor::Config>::AssetId>>,
) -> BoundedVec<Trade<<T as pallet_route_executor::Config>::AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<<T as pallet_route_executor::Config>::AssetId>, sp_runtime::traits::ConstU32<5>> =
		trades.try_into().unwrap();
	bounded_vec
}

type StableswapCurrencyOf<T> = <T as pallet_stableswap::Config>::Currency;
type OmnipoolCurrencyOf<T> = <T as pallet_omnipool::Config>::Currency;
type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;
type StableswapPallet<T> = pallet_stableswap::Pallet<T>;

fn initialize_omnipool<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: pallet_ema_oracle::Config,
	<T as pallet_route_executor::Config>::AssetId: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let stable_amount: Balance = 5_000_000_000_000_000_000_000_000u128;
	let native_amount: Balance = 5_000_000_000_000_000_000_000_000u128;
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

pub fn init_stableswap<
	T: Config
		+ pallet_asset_registry::Config
		+ pallet_route_executor::Config
		+ pallet_stableswap::Config
		+ pallet_omnipool::Config,
>() -> Result<(<T as pallet_stableswap::Config>::AssetId, AssetId, AssetId), DispatchError>
where
	<T as pallet_stableswap::Config>::AssetId: From<u32>,
	<T as pallet_stableswap::Config>::AssetId: Into<u32>,
	<T as pallet_stableswap::Config>::AssetId: From<<T as pallet_omnipool::Config>::AssetId>,
	<T as pallet_stableswap::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_asset_registry::Config>::Balance: From<u128>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	<T as pallet_stableswap::Config>::AssetId: From<u32>,
	<T as pallet_route_executor::Config>::AssetId: From<u32>,
	<T as pallet_stableswap::Config>::AssetId: From<<T as pallet_asset_registry::Config>::AssetId>,
{
	let caller: T::AccountId = account("caller", 0, 1);
	let lp_provider: T::AccountId = account("provider", 0, 1);
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<T as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<T as pallet_stableswap::Config>::AssetId>> = vec![];

	let mut asset_ids: Vec<<T as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		//let asset_id = regi_asset(name.clone(), 1_000_000, 10000 + idx as u32)?;
		//let asset_id = <T as pallet_omnipool::Config>::AssetRegistry::create_asset(&name, 1u128)?;
		let asset_id = pallet_asset_registry::Pallet::<T>::create_asset(&name, 1u128.into())?;
		pallet_asset_registry::Pallet::<T>::set_metadata(RawOrigin::Root.into(), asset_id, b"xDUM".to_vec(), 18u8)?;
		asset_ids.push(asset_id.into());
		<T as pallet_stableswap::Config>::Currency::update_balance(
			asset_id.into(),
			&caller.clone(),
			1_000_000_000_000_000i128,
		)?;
		<T as pallet_stableswap::Config>::Currency::update_balance(
			asset_id.into(),
			&lp_provider.clone(),
			1_000_000_000_000_000i128,
		)?;
		/*<T as pallet_stableswap::Config>::Currency::update_balance(
			RawOrigin::Root.into(),
			caller.clone(),
			asset_id,
			1_000_000_000_000_000i128,
		)?;
		<T as pallet_stableswap::Config>::Currency::update_balance(
			RawOrigin::Root.into(),
			lp_provider.clone(),
			asset_id,
			1_000_000_000_000_000_000_000i128,
		)?;*/
		initial.push(AssetAmount::new(asset_id.into(), initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id.into(), liquidity_added));
	}
	let pool_id = pallet_asset_registry::Pallet::<T>::create_asset(&b"pool".to_vec(), 1u128.into())?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);
	let asset_in: AssetId = (*asset_ids.last().unwrap()).into();
	let asset_out: AssetId = (*asset_ids.first().unwrap()).into();

	let successful_origin = <T as pallet_stableswap::Config>::AuthorityOrigin::try_successful_origin().unwrap();
	StableswapPallet::<T>::create_pool(successful_origin, pool_id.into(), asset_ids, amplification, fee)?;

	StableswapPallet::<T>::add_liquidity(RawOrigin::Signed(caller).into(), pool_id.into(), initial)?;

	//let seller: AccountId = account("seller", 0, 1);
	//let amount_sell = 100_000_000_000_000u128;

	//T::update_balance(asset_in.into(), &seller.clone(), amount_sell as i128)?;

	/*<T as pallet_stableswap::Config>::Currency::update_balance(
		RawOrigin::Root.into(),
		seller,
		asset_in,
		amount_sell as i128,
	)?;*/

	// Worst case is when amplification is changing
	StableswapPallet::<T>::update_amplification(
		RawOrigin::Root.into(),
		pool_id.into(),
		1000,
		100u32.into(),
		1000u32.into(),
	)?;

	do_trade_in_stable::<T>(pool_id.into(), asset_in.into(), asset_out.into())?;

	Ok((pool_id.into(), asset_in, asset_out))
}

const SEED: u32 = 0;
fn create_funded_account<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(
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
	OmnipoolCurrencyOf::<T>::deposit(currency, &to, amount)
}

fn create_funded_account_stable<T: Config + pallet_route_executor::Config + pallet_stableswap::Config>(
	name: &'static str,
	index: u32,
	amount: Balance,
	currency: <T as pallet_stableswap::Config>::AssetId,
) -> T::AccountId
where
	<T as pallet_stableswap::Config>::AssetId: From<u32>,
{
	let caller: T::AccountId = account(name, index, SEED);

	fund_stable::<T>(caller.clone(), currency, amount).unwrap();

	caller
}

fn fund_stable<T: Config + pallet_stableswap::Config>(
	to: T::AccountId,
	currency: <T as pallet_stableswap::Config>::AssetId,
	amount: Balance,
) -> DispatchResult {
	StableswapCurrencyOf::<T>::deposit(currency, &to, amount)
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_hdx_trade<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_route_executor::Config>::AssetId: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, HDX.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), HDX.into(), ONE, 0)
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_dai_trade<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_route_executor::Config>::AssetId: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, DAI.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), DAI.into(), ONE, 0)
}

//NOTE: This is necessary for oracle to provide price.
fn do_trade_in_stable<T: Config + pallet_route_executor::Config + pallet_stableswap::Config>(
	pool_id: <T as pallet_stableswap::Config>::AssetId,
	asset_a: <T as pallet_stableswap::Config>::AssetId,
	asset_b: <T as pallet_stableswap::Config>::AssetId,
) -> DispatchResult
where
	<T as pallet_route_executor::Config>::AssetId: From<u32>,
	<T as pallet_stableswap::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account_stable::<T>("tmp_trader", 0, 100 * ONE, asset_a);

	fund_stable::<T>(trader.clone(), asset_b, 100 * ONE)?;

	StableswapPallet::<T>::sell(RawOrigin::Signed(trader).into(), pool_id, asset_a, asset_b, ONE, 0)
}

fn create_account_with_native_balance<T: Config + pallet_route_executor::Config + pallet_omnipool::Config>(
) -> Result<T::AccountId, DispatchError>
where
	OmnipoolCurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
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
		OmnipoolCurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
		<T as pallet_stableswap::Config>::Currency : MultiCurrencyExtended<T::AccountId, Amount = i128>,
		T: crate::pallet::Config + pallet_omnipool::Config + pallet_ema_oracle::Config + pallet_route_executor::Config + pallet_stableswap::Config + pallet_asset_registry::Config,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_stableswap::Config>::AssetId: From<u32> + Into<u32>,
		<T as pallet_route_executor::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<u32>,
		<T as pallet_stableswap::Config>::AssetId: From<<T as pallet_omnipool::Config>::AssetId>,
		<T as pallet_omnipool::Config>::AssetId: Into<<T as pallet_route_executor::Config>::AssetId>,
		<T as pallet_omnipool::Config>::AssetId: From<<T as pallet_route_executor::Config>::AssetId>,
			<T as pallet_asset_registry::Config>::Balance: From<u128>,
	<T as pallet_stableswap::Config>::AssetId: From<<T as pallet_asset_registry::Config>::AssetId>,
		u128: From<<T as pallet_route_executor::Config>::Balance>,
		<T as pallet_route_executor::Config>::AssetId: From<<T as pallet_route_executor::Config>::AssetId>,
		<T as pallet_route_executor::Config>::Balance: From<u128>
	}

	on_initialize_with_buy_trade{
		//TODO: Rebenchmark it with dynamic length of route once we have other AMMs in hydra
		initialize_omnipool::<T>()?;
		set_period::<T>(1000);
		let seller: T::AccountId = account("seller", 3, 1);
		let other_seller: T::AccountId = account("seller", 3, 1);

		let amount_buy = 200 * ONE;

		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &seller, 20_000_000_000_000_000_000_000i128)?;
		<T as pallet_omnipool::Config>::Currency::update_balance(0u32.into(), &seller, 500_000_000_000_000i128)?;

		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &other_seller, 20_000_000_000_000_000_000_000i128)?;

		let schedule1 = schedule_buy_fake::<T>(seller.clone(), HDX.into(), DAI.into(), amount_buy, PoolType::Omnipool);
		let execution_block = 1001u32;

		assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller),0);
		let reserved_balance = get_named_reseve_balance::<T>(HDX.into(), seller.clone());

		let init_reserved_balance = 2000 * ONE;
		assert_eq!(init_reserved_balance, reserved_balance);

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(DAI.into(), &seller), 0);

		//Make sure that we have other schedules planned in the block where the benchmark schedule is planned, leading to worst case
		//We leave only one slot
		let schedule_period = 3;
		let next_block_to_replan = execution_block + schedule_period;
		let number_of_all_schedules = T::MaxSchedulePerBlock::get() + T::MaxSchedulePerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(other_seller.clone()).into(), schedule1.clone(), Option::Some(next_block_to_replan.into())));
		}

		assert_eq!((T::MaxSchedulePerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((next_block_to_replan + DELAY_AFTER_LAST_RADIUS).into()).len());
	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		let new_dai_balance = <T as pallet_omnipool::Config>::Currency::free_balance(DAI.into(), &seller);
		assert_eq!(new_dai_balance, amount_buy);
		assert_eq!((T::MaxSchedulePerBlock::get()) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((next_block_to_replan + DELAY_AFTER_LAST_RADIUS).into()).len());
	}

	on_initialize_with_sell_trade{
		//TODO: Rebenchmark it with dynamic length of route once we have other AMMs in hydra
		initialize_omnipool::<T>()?;
		set_period::<T>(1000);
		let seller: T::AccountId = account("seller", 3, 1);
		let other_seller: T::AccountId = account("seller", 3, 1);

		let amount_sell = 100 * ONE;

		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &seller, 20_000_000_000_000_000i128)?;

		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &other_seller, 20_000_000_000_000_000_000_000i128)?;

		let schedule1 = schedule_sell_fake::<T>(seller.clone(), HDX.into(), DAI.into(), amount_sell, PoolType::Omnipool);
		let execution_block = 1001u32;

		assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller),0);
		let reserved_balance = get_named_reseve_balance::<T>(HDX.into(), seller.clone());

		let init_reserved_balance = 2000 * ONE;
		assert_eq!(init_reserved_balance, reserved_balance);

		assert_eq!(<T as pallet_omnipool::Config>::Currency::free_balance(DAI.into(), &seller), 0);

		//Make sure that we have other schedules planned in the block where the benchmark schedule is planned, leading to worst case
		//We leave only one slot
		let schedule_period = 3;
		let next_block_to_replan = execution_block + schedule_period;
		let number_of_all_schedules = T::MaxSchedulePerBlock::get() + T::MaxSchedulePerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(other_seller.clone()).into(), schedule1.clone(), Option::Some(next_block_to_replan.into())));
		}
		assert_eq!((T::MaxSchedulePerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((next_block_to_replan + DELAY_AFTER_LAST_RADIUS).into()).len());
	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		let new_dai_balance = <T as pallet_omnipool::Config>::Currency::free_balance(T::StableCoinAssetId::get(), &seller);
		assert!(new_dai_balance > 0);
		assert_eq!((T::MaxSchedulePerBlock::get()) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((next_block_to_replan + DELAY_AFTER_LAST_RADIUS).into()).len());
	}

	//TODO: continue once we have oracle for stableswap
	/*on_initialize_with_sell_trade_stableswap{
		let (pool_id, asset_in, asset_out) = init_stableswap::<T>()?;
		set_period::<T>(1000);
		let seller: T::AccountId = account("seller", 3, 1);

		let amount_sell = 100 * ONE;

		<T as pallet_stableswap::Config>::Currency::update_balance(asset_in.into(), &seller, 20_000_000_000_000_000_000_000i128)?;

		let schedule1 = schedule_sell_fake::<T>(seller.clone(), asset_in.into(), asset_out.into(), amount_sell, PoolType::Stableswap(pool_id.into().into()));
		let execution_block = 1001u32;

		assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));

		let init_reserved_balance = 2000 * ONE;
		let reserved_balance = get_named_reseve_balance::<T>(asset_in.into(), seller.clone());
		assert_eq!(init_reserved_balance, reserved_balance);

		assert_eq!(<T as pallet_stableswap::Config>::Currency::free_balance(asset_out.into(), &seller), 0);

		//Make sure that we have other schedules planned in the block where the benchmark schedule is planned, leading to worst case
		//We leave only one slot
		let other_seller: T::AccountId = account("seller2", 3, 1);
		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &other_seller, 20_000_000_000_000_000_000_000i128)?;
		let schedule_period = 3;
		let next_block_to_replan = execution_block + schedule_period;
		let number_of_all_schedules = T::MaxSchedulePerBlock::get() + T::MaxSchedulePerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		let schedule2 = schedule_buy_fake::<T>(other_seller.clone(), HDX.into(), DAI.into(), amount_sell, PoolType::Omnipool);
		for i in 0..number_of_all_schedules {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(other_seller.clone()).into(), schedule2.clone(), Option::Some(next_block_to_replan.into())));
		}

		assert_eq!((T::MaxSchedulePerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((next_block_to_replan + DELAY_AFTER_LAST_RADIUS).into()).len());;
	}: {
		crate::Pallet::<T>::on_initialize(execution_block.into());
	}
	verify {
		let new_asset_out_balance = <T as pallet_stableswap::Config>::Currency::free_balance(asset_out.into(), &seller);
		assert!(new_asset_out_balance > 0);
		assert_eq!((T::MaxSchedulePerBlock::get()) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((next_block_to_replan + DELAY_AFTER_LAST_RADIUS).into()).len());
	}*/

	on_initialize_with_empty_block{
		initialize_omnipool::<T>()?;

		let seller: T::AccountId = account("seller", 3, 1);

		let execution_block = 100u32;
		assert_eq!(crate::Pallet::<T>::schedules::<ScheduleId>(execution_block).len(), 0);
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
		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &caller, 100_000_000_000_000_000_000_000i128)?;

		let amount_sell = 200 * ONE;
		let schedule1 = schedule_fake::<T>(caller.clone(), HDX.into(), DAI.into(), amount_sell);
		let execution_block = 100u32;

		//We fill blocks with schedules leaving only one place
		let number_of_all_schedules = T::MaxSchedulePerBlock::get() + T::MaxSchedulePerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(caller.clone()).into(), schedule1.clone(), Option::Some(execution_block.into())));
		}

		let schedule_id : ScheduleId = number_of_all_schedules;

		assert_eq!((T::MaxSchedulePerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + DELAY_AFTER_LAST_RADIUS).into()).len());

	}: _(RawOrigin::Signed(caller.clone()), schedule1, Option::Some(execution_block.into()))
	verify {
		assert!(<Schedules<T>>::get::<ScheduleId>(schedule_id).is_some());

		assert_eq!((T::MaxSchedulePerBlock::get()) as usize, <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>((execution_block + DELAY_AFTER_LAST_RADIUS).into()).len());
	}

	terminate {
		initialize_omnipool::<T>()?;
		let caller: T::AccountId = create_account_with_native_balance::<T>()?;
		<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &caller, 100_000_000_000_000_000i128)?;

		let amount_sell = 200 * ONE;
		let schedule1 = schedule_fake::<T>(caller.clone(), HDX.into(), DAI.into(), amount_sell);
		let schedule_id : ScheduleId = 0;

		set_period::<T>(99);
		let execution_block = 100u32;
		assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(caller).into(), schedule1, Option::Some(execution_block.into())));

	}: _(RawOrigin::Root, schedule_id, None)
	verify {
		assert!(<Schedules<T>>::get::<ScheduleId>(schedule_id).is_none());
	}

}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
