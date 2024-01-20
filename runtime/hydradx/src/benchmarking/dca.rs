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
#![allow(unused_assignments)] // At test `on_initialize_with_empty_block` it does not recognize the assignment in the Act block

use crate::{
	AccountId, AssetId, Balance, BlockNumber, Currencies, EmaOracle, MaxSchedulesPerBlock, MultiTransactionPayment,
	NamedReserveId, Router, Runtime, System, DCA, XYK,
};

use crate::benchmarking::register_asset;
use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_support::{
	assert_ok,
	traits::{Hooks, Len},
	weights::Weight,
	BoundedVec,
};
use frame_system::RawOrigin;
use hydradx_traits::router::AssetPair;
use hydradx_traits::router::PoolType;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended, NamedMultiReservableCurrency};
use pallet_dca::types::{Order, Schedule, ScheduleId};
use pallet_dca::{ScheduleIdsPerBlock, Schedules};
use pallet_route_executor::Trade;
use pallet_route_executor::MAX_NUMBER_OF_TRADES;
use scale_info::prelude::vec::Vec;
use sp_runtime::traits::ConstU32;
use sp_runtime::FixedU128;
use sp_runtime::{DispatchError, Permill};
use sp_std::vec;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 2;

pub const ONE: Balance = 1_000_000_000_000;

// This is the sum of all "randomly" generated radiuses.
// In tests the radiuses are always the same as we use a fixed parent hash for generation,
// so it will always generate the same values
pub const DELAY_AFTER_LAST_RADIUS: u32 = 1854;

pub const RETRY_TO_SEARCH_FOR_FREE_BLOCK: u32 = 10;

fn schedule_fake(
	owner: AccountId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
) -> Schedule<AccountId, AssetId, BlockNumber> {
	let schedule1: Schedule<AccountId, AssetId, BlockNumber> = Schedule {
		owner,
		period: 3u32,
		total_amount: 1100 * ONE,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(15)),
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_amount_in: Balance::MAX,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]),
		},
	};
	schedule1
}

fn get_named_reseve_balance(token_id: AssetId, seller: AccountId) -> Balance {
	Currencies::reserved_balance_named(&NamedReserveId::get(), token_id, &seller)
}

fn schedule_buy_fake(
	owner: AccountId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
) -> Schedule<AccountId, AssetId, BlockNumber> {
	let schedule1: Schedule<AccountId, AssetId, BlockNumber> = Schedule {
		owner,
		period: 3u32,
		total_amount: 2000 * ONE,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(15)),
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_amount_in: Balance::MAX,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]),
		},
	};
	schedule1
}

fn schedule_sell_fake(
	owner: AccountId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
) -> Schedule<AccountId, AssetId, BlockNumber> {
	let schedule1: Schedule<AccountId, AssetId, BlockNumber> = Schedule {
		owner,
		period: 3u32,
		total_amount: 2000 * ONE,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(100)),
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_amount_out: Balance::MIN,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]),
		},
	};
	schedule1
}

fn set_period(to: u32) {
	while System::block_number() < Into::<BlockNumber>::into(to) {
		let b = System::block_number();

		System::on_finalize(b);
		EmaOracle::on_finalize(b);

		System::on_initialize(b + 1_u32);
		EmaOracle::on_initialize(b + 1_u32);

		System::set_block_number(b + 1_u32);
	}
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

fn create_account_with_native_balance() -> Result<AccountId, DispatchError> {
	let caller: AccountId = account("provider", 1, 1);
	let token_amount = 200 * ONE;
	<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(0u32, &caller, token_amount as i128)?;

	Ok(caller)
}

runtime_benchmarks! {
	{Runtime, pallet_dca}

	on_initialize_with_buy_trade{
		let seller: AccountId = account("seller", 3, 1);
		let other_seller: AccountId = account("seller", 3, 1);

		set_period(1000);

		let amount_buy = 200 * ONE;

		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &seller, 500_000_000_000_000i128)?;

		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &other_seller, 20_000_000_000_000_000_000_000i128)?;

		let schedule1 = schedule_buy_fake(seller.clone(), HDX, DAI, amount_buy);
		let execution_block = 1001u32;

		assert_ok!(DCA::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block)));

		assert_eq!(Currencies::free_balance(DAI, &seller),0);
		let reserved_balance = get_named_reseve_balance(HDX, seller.clone());

		let init_reserved_balance = 2000 * ONE;
		assert_eq!(init_reserved_balance, reserved_balance);

		assert_eq!(Currencies::free_balance(DAI, &seller), 0);

		//Make sure that we have other schedules planned in the block where the benchmark schedule is planned, leading to worst case
		//We leave only one slot
		let schedule_period = 3;
		let next_block_to_replan = execution_block + schedule_period;
		let number_of_all_schedules = MaxSchedulesPerBlock::get() + MaxSchedulesPerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(DCA::schedule(RawOrigin::Signed(other_seller.clone()).into(), schedule1.clone(), Option::Some(next_block_to_replan)));
		}

		assert_eq!((MaxSchedulesPerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<Runtime>>::get::<BlockNumber>(next_block_to_replan + DELAY_AFTER_LAST_RADIUS).len());
	}: {
		DCA::on_initialize(execution_block);
	}
	verify {
		assert_eq!((MaxSchedulesPerBlock::get()) as usize, <ScheduleIdsPerBlock<Runtime>>::get::<BlockNumber>(next_block_to_replan + DELAY_AFTER_LAST_RADIUS).len());
	}

	on_initialize_with_sell_trade{
		set_period(1000);
		let seller: AccountId = account("seller", 3, 1);
		let other_seller: AccountId = account("seller", 3, 1);

		let amount_sell = 100 * ONE;

		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &seller, 20_000_000_000_000_000i128)?;

		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &other_seller, 20_000_000_000_000_000_000_000i128)?;

		let schedule1 = schedule_sell_fake(seller.clone(), HDX, DAI, amount_sell);
		let execution_block = 1001u32;

		assert_ok!(DCA::schedule(RawOrigin::Signed(seller.clone()).into(), schedule1.clone(), Option::Some(execution_block)));

		assert_eq!(Currencies::free_balance(DAI, &seller),0);
		let reserved_balance = get_named_reseve_balance(HDX, seller.clone());

		let init_reserved_balance = 2000 * ONE;
		assert_eq!(init_reserved_balance, reserved_balance);

		assert_eq!(Currencies::free_balance(DAI, &seller), 0);

		//Make sure that we have other schedules planned in the block where the benchmark schedule is planned, leading to worst case
		//We leave only one slot
		let schedule_period = 3;
		let next_block_to_replan = execution_block + schedule_period;
		let number_of_all_schedules = MaxSchedulesPerBlock::get() + MaxSchedulesPerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(DCA::schedule(RawOrigin::Signed(other_seller.clone()).into(), schedule1.clone(), Option::Some(next_block_to_replan)));
		}
		assert_eq!((MaxSchedulesPerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<Runtime>>::get::<BlockNumber>(next_block_to_replan + DELAY_AFTER_LAST_RADIUS).len());
	}: {
		DCA::on_initialize(execution_block);
	}
	verify {
		assert_eq!((MaxSchedulesPerBlock::get()) as usize, <ScheduleIdsPerBlock<Runtime>>::get::<BlockNumber>(next_block_to_replan + DELAY_AFTER_LAST_RADIUS).len());
	}

	on_initialize_with_empty_block{
		let seller: AccountId = account("seller", 3, 1);

		let execution_block = 100u32;
		assert_eq!(DCA::schedules::<ScheduleId>(execution_block), None);
		let r = DCA::schedules::<ScheduleId>(execution_block);
		let mut weight = Weight::zero();
	}: {
		weight = DCA::on_initialize(execution_block);
	}
	verify {
		assert!(weight.ref_time() > 0u64);
	}


	schedule{
		let caller: AccountId = create_account_with_native_balance()?;

		let asset_1 = register_asset(b"AS1".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_2 = register_asset(b"AS2".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_3 = register_asset(b"AS3".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_4 = register_asset(b"AS4".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_5 = register_asset(b"AS5".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		create_xyk_pool(asset_1, asset_2);
		create_xyk_pool(asset_2, asset_3);
		create_xyk_pool(asset_3, asset_4);
		create_xyk_pool(asset_4, asset_5);
		create_xyk_pool(asset_5, HDX);

		set_period(10);

		let route = vec![
			Trade {
				pool: PoolType::XYK,
				asset_in: asset_1,
				asset_out: asset_2,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: asset_2,
				asset_out: asset_3,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: asset_3,
				asset_out: asset_4,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: asset_4,
				asset_out: asset_5,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: asset_5,
				asset_out: HDX,
			}
		];

		assert_eq!(route.len(),MAX_NUMBER_OF_TRADES as usize, "Route length should be as big as max number of trades allowed");

		let maker: AccountId = account("maker", 0, 0);
		Router::set_route(RawOrigin::Signed(maker).into(), AssetPair::new(asset_1, HDX), route)?;

		assert_ok!(MultiTransactionPayment::add_currency(
				RawOrigin::Root.into(),
				asset_1,
				FixedU128::from_rational(88, 100),
			));

		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &caller, 100_000_000_000_000_000_000_000i128)?;
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(asset_1, &caller, 100_000_000_000_000_000_000_000i128)?;
		let amount_sell = 200 * ONE;

		let schedule1: Schedule<AccountId, AssetId, BlockNumber> = Schedule {
			owner:caller.clone() ,
			period: 3u32,
			total_amount: 1100 * ONE,
			max_retries: None,
			stability_threshold: None,
			slippage: Some(Permill::from_percent(15)),
			order: Order::Buy {
				asset_in: asset_1,
				asset_out: DAI,
				amount_out: amount_sell,
				max_amount_in: Balance::MAX,
				route: create_bounded_vec(vec![Trade {
					pool: PoolType::Omnipool,
					asset_in: asset_1,
					asset_out: DAI,
				}]),
			},
		};

		let execution_block = 100u32;

		//We fill blocks with schedules leaving only one place
		let schedule_2 = schedule_fake(caller.clone(), HDX, DAI, amount_sell);
		let number_of_all_schedules = MaxSchedulesPerBlock::get() + MaxSchedulesPerBlock::get() * RETRY_TO_SEARCH_FOR_FREE_BLOCK - 1;
		for i in 0..number_of_all_schedules {
			assert_ok!(DCA::schedule(RawOrigin::Signed(caller.clone()).into(), schedule_2.clone(), Option::Some(execution_block)));
		}

		let schedule_id : ScheduleId = number_of_all_schedules;

		assert_eq!((MaxSchedulesPerBlock::get() - 1) as usize, <ScheduleIdsPerBlock<Runtime>>::get::<BlockNumber>(execution_block + DELAY_AFTER_LAST_RADIUS).len());

	}: _(RawOrigin::Signed(caller.clone()), schedule1, Option::Some(execution_block))
	verify {
		assert!(<Schedules<Runtime>>::get::<ScheduleId>(schedule_id).is_some());

		assert_eq!((MaxSchedulesPerBlock::get()) as usize, <ScheduleIdsPerBlock<Runtime>>::get::<BlockNumber>(execution_block + DELAY_AFTER_LAST_RADIUS).len());
	}

	terminate {
		let caller: AccountId = create_account_with_native_balance()?;

		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &caller, 100_000_000_000_000_000i128)?;

		let amount_sell = 200 * ONE;
		let schedule1 = schedule_fake(caller.clone(), HDX, DAI, amount_sell);
		let schedule_id : ScheduleId = 0;

		set_period(99);
		let execution_block = 100u32;
		assert_ok!(DCA::schedule(RawOrigin::Signed(caller).into(), schedule1, Option::Some(execution_block)));

	}: _(RawOrigin::Root, schedule_id, None)
	verify {
		assert!(<Schedules<Runtime>>::get::<ScheduleId>(schedule_id).is_none());
	}

}

pub const INITIAL_BALANCE: Balance = 10_000_000 * ONE;

fn funded_account(name: &'static str, index: u32, assets: &[AssetId]) -> AccountId {
	let account: AccountId = account(name, index, 0);
	for asset in assets {
		assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
			*asset,
			&account,
			INITIAL_BALANCE.try_into().unwrap(),
		));
	}
	account
}

fn create_xyk_pool(asset_a: u32, asset_b: u32) {
	let caller: AccountId = funded_account("caller", 0, &[asset_a, asset_b]);

	let amount = 100000 * ONE;
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		caller.clone(),
		asset_a,
		amount as i128,
	));

	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		caller.clone(),
		asset_b,
		amount as i128,
	));

	assert_ok!(XYK::create_pool(
		RawOrigin::Signed(caller.clone()).into(),
		asset_a,
		amount,
		asset_b,
		amount,
	));

	assert_ok!(XYK::sell(
		RawOrigin::Signed(caller).into(),
		asset_a,
		asset_b,
		10 * ONE,
		0u128,
		false,
	));
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<Runtime> {
			registered_assets: vec![(b"DAI".to_vec(), 1_000u128, Some(DAI))],
			native_asset_name: b"HDX".to_vec(),
			native_existential_deposit: NativeExistentialDeposit::get(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
