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

use crate::{AccountId, AssetId, Balance, Currencies, Omnipool, Runtime, Stableswap, Tokens};

use super::*;
use codec::alloc::string::ToString;
use frame_benchmarking::account;
use frame_support::traits::EnsureOrigin;
use frame_system::{Pallet as System, RawOrigin};
use hydradx_traits::Registry;
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

fn generate_trades_with_pools(number_of_trades: u32) -> Result<(AssetId, AssetId, Vec<Trade<AssetId>>), DispatchError> {
	let (stable_pool_id, stable_asset_in, stable_asset_out) = init_stableswap()?;
	initialize_omnipool()?;

	let owner: AccountId = account("caller", 0, 1);

	add_omnipool_token(stable_asset_in, owner.clone())?;
	add_omnipool_token(stable_asset_out, owner.clone())?;

	let asset_in = DAI;
	let mut asset_out = HDX;

	let trades = match number_of_trades {
		1 => {
			vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]
		}
		2 => {
			asset_out = stable_asset_out;

			vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in,
					asset_out: stable_asset_in,
				},
				Trade {
					pool: PoolType::Stableswap(stable_pool_id),
					asset_in: stable_asset_in,
					asset_out,
				},
			]
		}
		3 => {
			vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: DAI,
					asset_out: stable_asset_in,
				},
				Trade {
					pool: PoolType::Stableswap(stable_pool_id),
					asset_in: stable_asset_in,
					asset_out: stable_asset_out,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: stable_asset_out,
					asset_out: HDX,
				},
			]
		}
		_ => {
			todo!("the given number of trades not supported. Add support once we add more pools to hydra such as xyk")
		}
	};

	let trades = trades.iter().take(number_of_trades as usize).cloned().collect();

	Ok((asset_in, asset_out, trades))
}

fn initialize_omnipool() -> DispatchResult {
	let stable_amount: Balance = 1_000_000_000_000_000_000_u128;
	let native_amount: Balance = 1_000_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);
	let acc = Omnipool::protocol_account();

	Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

	let _ = regi_asset(b"HDX".to_vec(), 1_000_000, HDX);
	let _ = regi_asset(b"LRNA".to_vec(), 1_000_000, LRNA);
	let _ = regi_asset(b"DAI".to_vec(), 1_000_000, DAI);

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

fn add_omnipool_token(token_id: AssetId, owner: AccountId) -> DispatchResult {
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		Omnipool::protocol_account(),
		token_id,
		3000 * UNITS as i128,
	));

	assert_ok!(Omnipool::add_token(
		RawOrigin::Root.into(),
		token_id,
		FixedU128::from((6, 7)),
		Permill::from_percent(100),
		owner.clone()
	));

	Ok(())
}

pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let caller: AccountId = account("caller", 0, 1);
	let lp_provider: AccountId = account("provider", 0, 1);
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];

	let mut asset_ids: Vec<<Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let asset_id = regi_asset(name.clone(), 1, 10000 + idx as u32)?;
		//let asset_id = AssetRegistry::create_asset(&name, 1u128)?;
		asset_ids.push(asset_id);
		Currencies::update_balance(
			RawOrigin::Root.into(),
			caller.clone(),
			asset_id,
			1_000_000_000_000_000i128,
		)?;
		Currencies::update_balance(
			RawOrigin::Root.into(),
			lp_provider.clone(),
			asset_id,
			1_000_000_000_000_000_000_000i128,
		)?;
		initial.push(AssetAmount::new(asset_id, initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
	}
	let pool_id = AssetRegistry::create_asset(&b"pool".to_vec(), 1u128)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	let successful_origin = <Runtime as pallet_stableswap::Config>::AuthorityOrigin::try_successful_origin().unwrap();
	Stableswap::create_pool(successful_origin, pool_id, asset_ids, amplification, fee)?;

	Stableswap::add_liquidity(RawOrigin::Signed(caller).into(), pool_id, initial)?;

	let seller: AccountId = account("seller", 0, 1);
	let amount_sell = 100_000_000_000_000u128;

	Currencies::update_balance(RawOrigin::Root.into(), seller, asset_in, amount_sell as i128)?;

	// Worst case is when amplification is changing
	Stableswap::update_amplification(RawOrigin::Root.into(), pool_id, 1000, 100u32.into(), 1000u32.into())?;

	Ok((pool_id, asset_in, asset_out))
}

pub fn regi_asset(name: Vec<u8>, deposit: Balance, asset_id: AssetId) -> Result<AssetId, DispatchError> {
	let name = AssetRegistry::to_bounded_name(name)?;
	let asset_id = AssetRegistry::register_asset(
		name,
		pallet_asset_registry::AssetType::<AssetId>::Token,
		deposit,
		Some(asset_id),
		None,
	)?;
	AssetRegistry::set_metadata(RawOrigin::Root.into(), asset_id, b"DUM".to_vec(), 18u8)?;

	Ok(asset_id)
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
use hydradx_traits::router::PoolType;
use pallet_route_executor::Trade;
use pallet_stableswap::types::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
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

runtime_benchmarks! {
	{ Runtime, pallet_route_executor}


	sell_omnipool {
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

	buy_omnipool {
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
		assert!(<Currencies as MultiCurrency<_>>::total_balance(asset_in, &caller) < 100 * UNITS);
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_out, &caller), amount_to_buy);
	}

	sell_stableswap {
		let (pool_id, asset_in, asset_out) = init_stableswap()?;

		let trades = vec![Trade {
			pool: PoolType::Stableswap(pool_id),
			asset_in: asset_in,
			asset_out: pool_id
		}];

		let caller: AccountId = create_funded_account::<Runtime>("trader", 0, 100 * UNITS, asset_in);
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_in, &caller), 100 * UNITS);

		let amount_to_sell = 10 * UNITS;
	}: {
		RouteExecutor::<Runtime>::sell(RawOrigin::Signed(caller.clone()).into(), asset_in, pool_id, amount_to_sell, 0u128, trades)?
	}
	verify{
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_in, &caller), 100 * UNITS -  amount_to_sell);
		assert!(<Currencies as MultiCurrency<_>>::total_balance(pool_id, &caller) > 0);
	}

	buy_stableswap {
		let (pool_id, asset_in, asset_out) = init_stableswap()?;

		let trades = vec![Trade {
			pool: PoolType::Stableswap(pool_id),
			asset_in: pool_id,
			asset_out: asset_out
		}];

		let caller: AccountId = create_funded_account::<Runtime>("trader", 0, 100 * UNITS, pool_id);

		let amount_to_buy = 10 * UNITS;
	}: {
		RouteExecutor::<Runtime>::buy(RawOrigin::Signed(caller.clone()).into(), pool_id, asset_out, amount_to_buy, u128::MAX, trades)?
	}
	verify{
		assert!(<Currencies as MultiCurrency<_>>::total_balance(pool_id, &caller) < 100 * UNITS);
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_out, &caller), amount_to_buy);
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
