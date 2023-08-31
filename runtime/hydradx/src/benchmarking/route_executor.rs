// This file is part of Basilisk-node

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
#![allow(clippy::result_large_err)]

use crate::{AccountId, AssetId, Balance, Currencies, Omnipool, Runtime, Tokens};

use super::*;

use frame_benchmarking::account;
use frame_system::{Pallet as System, RawOrigin};
use hydradx_traits::{registry::Create, router::PoolType, AssetKind};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};

pub const TVL_CAP: Balance = 222_222_000_000_000_000_000_000;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;

pub const ONE: Balance = 1_000_000_000_000;

type RouteExecutor<T> = pallet_route_executor::Pallet<T>;
type CurrencyOf<T> = <T as pallet_omnipool::Config>::Currency;
type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;

fn initialize_omnipool() -> DispatchResult {
	let stable_amount: Balance = 1_000_000_000_000_000_000_u128;
	let native_amount: Balance = 1_000_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);
	let acc = Omnipool::protocol_account();

	Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

	let _ = regi_asset(b"HDX".to_vec(), UNITS, HDX);
	let _ = regi_asset(b"LRNA".to_vec(), UNITS, LRNA);
	let _ = regi_asset(b"DAI".to_vec(), UNITS, DAI);

	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DAI,
		stable_amount,
		0
	));
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		acc,
		HDX,
		native_amount as i128,
	));

	Omnipool::initialize_pool(
		RawOrigin::Root.into(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(100),
	)?;

	//NOTE: This is necessary for oracle to provide price.
	do_lrna_hdx_trade::<Runtime>()?;
	do_lrna_dai_trade::<Runtime>()?;

	set_period::<Runtime>(10);

	do_lrna_dai_trade::<Runtime>()?;
	do_lrna_hdx_trade::<Runtime>()?;

	Ok(())
}

pub fn regi_asset(name: Vec<u8>, deposit: Balance, asset_id: AssetId) -> Result<AssetId, DispatchError> {
	AssetRegistry::register_asset(
		Some(asset_id),
		Some(&name),
		AssetKind::Token,
		Some(deposit),
		None,
		None,
		None,
		None,
		false,
	)
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_hdx_trade<T: pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, HDX.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), HDX.into(), ONE, 0)
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_dai_trade<T: pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, DAI.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), DAI.into(), ONE, 0)
}

fn fund<T: pallet_omnipool::Config>(
	to: T::AccountId,
	currency: <T as pallet_omnipool::Config>::AssetId,
	amount: Balance,
) -> DispatchResult {
	CurrencyOf::<T>::deposit(currency, &to, amount)
}

use frame_support::assert_ok;
use frame_support::traits::Hooks;
use pallet_route_executor::Trade;
use sp_runtime::{DispatchError, DispatchResult, FixedU128, Permill};
use sp_std::vec;

const SEED: u32 = 1;
pub const UNITS: Balance = 100_000_000_000;

fn create_funded_account<T: pallet_omnipool::Config>(
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

fn set_period<T: pallet_omnipool::Config>(to: u32)
where
	T: pallet_ema_oracle::Config,
	CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
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

//TODO: Rebenchmark both buy and sell with dynamic length of route once we have other AMMs in hydra

runtime_benchmarks! {
	{ Runtime, pallet_route_executor}

	sell {
		let n in 1..2;

		initialize_omnipool()?;

		let asset_in = HDX;
		let asset_out = DAI;
		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: HDX,
			asset_out: DAI
		}];

		let caller: AccountId = create_funded_account::<Runtime>("caller", 0, 100 * UNITS, HDX);

		let amount_to_sell = 10 * UNITS;
	}: {
		RouteExecutor::<Runtime>::sell(RawOrigin::Signed(caller.clone()).into(), asset_in, asset_out, amount_to_sell, 0u128, trades)?
	}
	verify{
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_in, &caller), 100 * UNITS -  amount_to_sell);
		assert!(<Currencies as MultiCurrency<_>>::total_balance(asset_out, &caller) > 0);
	}

	buy {
		let n in 1..2;

		initialize_omnipool()?;

		let asset_in = HDX;
		let asset_out = DAI;
		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: HDX,
			asset_out: DAI
		}];

		let caller: AccountId = create_funded_account::<Runtime>("caller", 0, 100 * UNITS, HDX);

		let amount_to_buy = 10 * UNITS;
	}: {
		RouteExecutor::<Runtime>::buy(RawOrigin::Signed(caller.clone()).into(), asset_in, asset_out, amount_to_buy, u128::MAX, trades)?
	}
	verify{
		assert!(<Currencies as MultiCurrency<_>>::total_balance(asset_out, &caller) < 100 * UNITS);
		assert!(<Currencies as MultiCurrency<_>>::total_balance(asset_out, &caller) > 0);
	}



}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;

	fn new_test_ext() -> sp_io::TestExternalities {
		let t: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<crate::Runtime>()
			.unwrap()
			.into();
		t
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
