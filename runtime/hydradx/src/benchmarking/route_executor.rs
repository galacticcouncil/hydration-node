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
#![allow(dead_code)] //TODO: temp allow, remove before merging this PR

use crate::{AccountId, AssetId, Balance, Currencies, Omnipool, Runtime, Tokens};

use super::*;

use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_system::{Pallet as System, RawOrigin};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::SaturatedConversion;

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

	regi_asset(b"HDX".to_vec(), UNITS, HDX);
	regi_asset(b"LRNA".to_vec(), UNITS, LRNA);
	regi_asset(b"DAI".to_vec(), UNITS, DAI);

	//update_balance(StableAssetId::get(), &acc, stable_amount);
	//update_balance(NativeAssetId::get(), &acc, native_amount);
	/**/

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

fn initialize_omnipool2<T: pallet_omnipool::Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: pallet_ema_oracle::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let stable_amount: Balance = 1_000_000_000_000_000_000_u128;
	let native_amount: Balance = 1_000_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);
	let acc = OmnipoolPallet::<T>::protocol_account();

	OmnipoolPallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

	/*reg_asset(b"HDX".to_vec(), 1u128, HDX).map_err(|_| DispatchError::Other("Failed to register asset"))?;
	reg_asset(b"LRNA".to_vec(), 1u128, LRNA).map_err(|_| DispatchError::Other("Failed to register asset"))?;
	reg_asset(b"DAI".to_vec(), 1u128, DAI).map_err(|_| DispatchError::Other("Failed to register asset"))?;*/

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

pub fn reg_asset(name: Vec<u8>, deposit: Balance, asset_id: AssetId) -> Result<AssetId, ()> {
	AssetRegistry::register_asset(
		AssetRegistry::to_bounded_name(name).map_err(|_| ())?,
		pallet_asset_registry::AssetType::<AssetId>::Token,
		deposit,
		Some(asset_id),
	)
	.map_err(|_| ())
}
pub fn regi_asset(name: Vec<u8>, deposit: Balance, asset_id: AssetId) -> Result<AssetId, DispatchError> {
	let name = AssetRegistry::to_bounded_name(name)?;
	AssetRegistry::register_asset(
		name,
		pallet_asset_registry::AssetType::<AssetId>::Token,
		deposit,
		Some(asset_id),
	)
}

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
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

fn create_account_with_native_balance<T: pallet_omnipool::Config>() -> Result<T::AccountId, DispatchError>
where
	CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: pallet_omnipool::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let caller: T::AccountId = account("provider", 1, 1);
	let token_amount = 200 * ONE;
	<T as pallet_omnipool::Config>::Currency::update_balance(0.into(), &caller, token_amount as i128)?;

	Ok(caller)
}

use frame_support::assert_ok;
use frame_support::traits::Hooks;
use hydradx_traits::router::PoolType;
use pallet_route_executor::Trade;
use sp_runtime::traits::Get;
use sp_runtime::{DispatchError, DispatchResult, FixedU128, Permill};
use sp_std::vec;

const SEED: u32 = 1;
pub const UNITS: Balance = 100_000_000_000;
const MAX_NUMBER_OF_TRADES: u32 = 5;

pub fn register_asset_with_name(name_as_bye_string: &[u8]) -> Result<AssetId, BenchmarkError> {
	register_asset(name_as_bye_string.to_vec(), 0u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))
}

pub fn create_account(name: &'static str) -> AccountId {
	account(name, 0, SEED)
}

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

runtime_benchmarks! {
	{ Runtime, pallet_route_executor}

	sell {
		let n in 1..2;

		initialize_omnipool()?;
		//initialize_omnipool2::<T>()?;

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

	/*buy {
		let n in 1..MAX_NUMBER_OF_TRADES;
		let (asset_in, asset_out, trades) = generate_trades(n).unwrap();

		let caller: AccountId = create_account("caller");

		let amount_to_buy = UNITS;
		let caller_asset_in_balance = 2000 * UNITS;

		update_balance(asset_in, &caller, caller_asset_in_balance);
	}: {
		RouteExecutor::<Runtime>::buy(RawOrigin::Signed(caller.clone()).into(), asset_in, asset_out, amount_to_buy, 10000u128 * UNITS, trades)?
	}
	verify{
		assert!(<Currencies as MultiCurrency<_>>::total_balance(asset_in, &caller) < caller_asset_in_balance);
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_out, &caller), amount_to_buy);
	}*/



}

#[cfg(test)]
mod tests {
	//use super::mock::Test;
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::default()
			.build_storage::<crate::Runtime>()
			.unwrap()
			.into();

		t.execute_with(|| {
			let id = regi_asset(b"HDX".to_vec(), UNITS, HDX).unwrap();
			assert_eq!(id, HDX);
			let id = regi_asset(b"LRNA".to_vec(), UNITS, LRNA).unwrap();
			assert_eq!(id, LRNA);
			let id = regi_asset(b"DAI".to_vec(), UNITS, DAI).unwrap();
			assert_eq!(id, DAI);
		});

		t
	}

	/*fn new_test_ext_mock() -> sp_io::TestExternalities {
		super::mock::ExtBuilder::default().build()
	}*/

	impl_benchmark_test_suite!(new_test_ext(),);
	//impl_benchmark_test_suite!(super::mock::ExtBuilder::default().build(),);
}
