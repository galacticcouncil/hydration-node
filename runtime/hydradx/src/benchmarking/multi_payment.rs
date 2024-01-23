// This file is part of Basilisk-node

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

use super::*;
use crate::{AccountId, AssetId, Balance, Currencies, EmaOracle, Runtime, System};
use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_support::assert_ok;
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::RawOrigin;
use hydradx_traits::router::PoolType;
use hydradx_traits::router::RouteProvider;
use hydradx_traits::PriceOracle;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrencyExtended;
use pallet_route_executor::MAX_NUMBER_OF_TRADES;
use primitives::{BlockNumber, Price};
use sp_runtime::traits::SaturatedConversion;
use sp_runtime::FixedU128;

type MultiPaymentPallet<T> = pallet_transaction_multi_payment::Pallet<T>;
type XykPallet<T> = pallet_xyk::Pallet<T>;
type Router<T> = pallet_route_executor::Pallet<T>;
use hydradx_traits::router::AssetPair;
use hydradx_traits::router::Trade;
use hydradx_traits::OraclePeriod;

const SEED: u32 = 1;

const UNITS: Balance = 1_000_000_000_000;

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, pallet_transaction_multi_payment}

	add_currency {
		let price = Price::from(10);
	}: { MultiPaymentPallet::<Runtime>::add_currency(RawOrigin::Root.into(), 10, price)? }
	verify {
		assert_eq!(MultiPaymentPallet::<Runtime>::currencies(10), Some(price));
	}

	remove_currency {
		assert_ok!(MultiPaymentPallet::<Runtime>::add_currency(RawOrigin::Root.into(), 10, Price::from(2)));
	}: { MultiPaymentPallet::<Runtime>::remove_currency(RawOrigin::Root.into(), 10)? }
	verify {
		assert_eq!(MultiPaymentPallet::<Runtime>::currencies(10), None)
	}

	set_currency {
		let maker: AccountId = account("maker", 0, SEED);
		let caller: AccountId = account("caller", 0, SEED);
		let fallback_account: AccountId = account("fallback_account", 1, SEED);

		let asset_id = register_asset(b"TST".to_vec(), 100u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		MultiPaymentPallet::<Runtime>::add_currency(RawOrigin::Root.into(), asset_id, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;

		update_balance(asset_id, &caller,100_000_000_000_000);

	}: { MultiPaymentPallet::<Runtime>::set_currency(RawOrigin::Signed(caller.clone()).into(), asset_id)? }
	verify{
		assert_eq!(MultiPaymentPallet::<Runtime>::get_currency(caller), Some(asset_id));
	}

	get_oracle_price {
		let maker: AccountId = account("maker", 0, SEED);

		let asset_1 = register_asset(b"AS1".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_2 = register_asset(b"AS2".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_3 = register_asset(b"AS3".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_4 = register_asset(b"AS4".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_5 = register_asset(b"AS5".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_6 = register_asset(b"AS6".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		create_xyk_pool::<Runtime>(asset_1, 1000 * UNITS, asset_2, 1000 * UNITS);
		create_xyk_pool::<Runtime>(asset_2, 1000 * UNITS, asset_3, 1000 * UNITS);
		create_xyk_pool::<Runtime>(asset_3, 1000 * UNITS, asset_4, 1000 * UNITS);
		create_xyk_pool::<Runtime>(asset_4, 1000 * UNITS, asset_5, 1000 * UNITS);
		create_xyk_pool::<Runtime>(asset_5, 1000 * UNITS, asset_6, 1000 * UNITS);

		xyk_sell::<Runtime>(asset_1,asset_2, 10 * UNITS);
		xyk_sell::<Runtime>(asset_2,asset_3, 10 * UNITS);
		xyk_sell::<Runtime>(asset_3,asset_4, 10 * UNITS);
		xyk_sell::<Runtime>(asset_4,asset_5, 10 * UNITS);
		xyk_sell::<Runtime>(asset_5,asset_6, 10 * UNITS);

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
				asset_out: asset_6,
			}
		];

		assert_eq!(route.len(),MAX_NUMBER_OF_TRADES as usize, "Route length should be as big as max number of trades allowed");

		Router::<Runtime>::set_route(RawOrigin::Signed(maker).into(), AssetPair::new(asset_1, asset_6), route)?;

		let mut _price = None;//Named with underscore because clippy thinks that the price in the Act part is unused.

	}: {
		let on_chain_route = <Runtime as pallet_transaction_multi_payment::Config>::RouteProvider::get_route(AssetPair::new(asset_1, asset_6));

		_price = <Runtime as pallet_transaction_multi_payment::Config>::OraclePriceProvider::price(&on_chain_route, OraclePeriod::Short)
			.map(|ratio| FixedU128::from_rational(ratio.n, ratio.d));

		}

	verify{
		assert!(_price.is_some());
	}
}

fn create_xyk_pool<T: pallet_xyk::Config>(asset_a: AssetId, amount_a: Balance, asset_b: AssetId, amount_b: Balance)
where
	<T as frame_system::Config>::RuntimeOrigin: core::convert::From<frame_system::RawOrigin<sp_runtime::AccountId32>>,
{
	let maker: AccountId = account("xyk-maker", 0, SEED);

	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		maker.clone(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		maker.clone(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XykPallet::<T>::create_pool(
		RawOrigin::Signed(maker).into(),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}

fn xyk_sell<T: pallet_xyk::Config>(asset_a: AssetId, asset_b: AssetId, amount_a: Balance)
where
	<T as frame_system::Config>::RuntimeOrigin: core::convert::From<frame_system::RawOrigin<sp_runtime::AccountId32>>,
{
	let maker: AccountId = account("xyk-seller", 0, SEED);

	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		maker.clone(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(XykPallet::<T>::sell(
		RawOrigin::Signed(maker).into(),
		asset_a,
		asset_b,
		amount_a,
		u128::MIN,
		false
	));
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

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap()
			.into()
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
