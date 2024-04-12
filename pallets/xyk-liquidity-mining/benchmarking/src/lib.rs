// This file is part of Basilisk-node.

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
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unnecessary_wraps)]

mod mock;

use pallet_liquidity_mining::{GlobalFarmId, LoyaltyCurve, YieldFarmId};
use pallet_xyk::types::{AssetId, AssetPair, Balance};
use pallet_xyk_liquidity_mining::Pallet as XYKLiquidityMining;

use frame_benchmarking::{account, benchmarks};
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as System, RawOrigin};

use frame_support::dispatch;
use orml_traits::arithmetic::One;
use orml_traits::MultiCurrency;
use sp_arithmetic::FixedU128;
use sp_arithmetic::Perquintill;
use sp_std::convert::From;

use pallet_xyk as xykpool;

pub const GLOBAL_FARM_ID: GlobalFarmId = 1;
pub const GLOBAL_FARM_ID_2: GlobalFarmId = 3;
pub const YIELD_FARM_ID: YieldFarmId = 2;
pub const YIELD_FARM_ID_2: YieldFarmId = 4;
pub const YIELD_FARM_ID_3: YieldFarmId = 4;
pub const DEPOSIT_ID: u128 = 1;

const SEED: u32 = 0;

const BSX: AssetId = 0;
const KSM: AssetId = 1;
const DOT: AssetId = 2;
const ASSET_PAIR: AssetPair = AssetPair {
	asset_in: BSX,
	asset_out: KSM,
};

const INITIAL_BALANCE: Balance = 100_000_000;
const ONE: Balance = 1_000_000_000_000;

pub trait Config: pallet_xyk_liquidity_mining::Config + pallet_xyk::Config + pallet_asset_registry::Config {}

pub struct Pallet<T: Config>(XYKLiquidityMining<T>);

type MultiCurrencyOf<T> = <T as pallet_xyk_liquidity_mining::Config>::MultiCurrency;

fn create_funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);

	<T as pallet_xyk_liquidity_mining::Config>::MultiCurrency::deposit(BSX, &caller, INITIAL_BALANCE * ONE).unwrap();

	<T as pallet_xyk_liquidity_mining::Config>::MultiCurrency::deposit(KSM, &caller, INITIAL_BALANCE * ONE).unwrap();

	<T as pallet_xyk_liquidity_mining::Config>::MultiCurrency::deposit(DOT, &caller, INITIAL_BALANCE * ONE).unwrap();

	caller
}

fn initialize_pool<T: Config>(
	caller: T::AccountId,
	asset_a: AssetId,
	asset_b: AssetId,
	amount_a: Balance,
	amount_b: Balance,
) -> dispatch::DispatchResult {
	xykpool::Pallet::<T>::create_pool(RawOrigin::Signed(caller).into(), asset_a, amount_a, asset_b, amount_b)
}

fn xyk_add_liquidity<T: Config>(
	caller: T::AccountId,
	assets: AssetPair,
	amount_a: Balance,
	amount_b_max: Balance,
) -> dispatch::DispatchResult {
	xykpool::Pallet::<T>::add_liquidity(
		RawOrigin::Signed(caller).into(),
		assets.asset_in,
		assets.asset_out,
		amount_a,
		amount_b_max,
	)
}

fn lm_create_global_farm<T: Config>(
	total_rewards: Balance,
	owner: T::AccountId,
	yield_per_period: Perquintill,
) -> dispatch::DispatchResult {
	XYKLiquidityMining::<T>::create_global_farm(
		RawOrigin::Root.into(),
		total_rewards,
		BlockNumberFor::<T>::from(1_000_000_u32),
		BlockNumberFor::<T>::from(1_u32),
		BSX,
		BSX,
		owner,
		yield_per_period,
		1_000,
		One::one(),
	)
}

fn lm_deposit_shares<T: Config>(caller: T::AccountId, assets: AssetPair, amount: Balance) -> dispatch::DispatchResult {
	XYKLiquidityMining::<T>::deposit_shares(
		RawOrigin::Signed(caller).into(),
		GLOBAL_FARM_ID,
		YIELD_FARM_ID,
		assets,
		amount,
	)
}

fn lm_create_yield_farm<T: Config>(
	caller: T::AccountId,
	farm_id: GlobalFarmId,
	assets: AssetPair,
	multiplier: FixedU128,
) -> dispatch::DispatchResult {
	XYKLiquidityMining::<T>::create_yield_farm(
		RawOrigin::Signed(caller).into(),
		farm_id,
		assets,
		multiplier,
		Some(LoyaltyCurve::default()),
	)
}

fn set_period<T: Config>(block: u32) {
	//NOTE: predefined global farm has period size = 1 block.
	System::<T>::set_block_number(block.into());
}

benchmarks! {
	create_global_farm {
		let total_rewards = 1_000_000 * ONE;
		let caller = create_funded_account::<T>("caller", 0);
		let planned_yielding_periods = BlockNumberFor::<T>::from(1_000_000_u32);
		let yield_per_period = Perquintill::from_percent(20);
		let blocks_per_period = BlockNumberFor::<T>::from(1_u32);
	}: {
		XYKLiquidityMining::<T>::create_global_farm(RawOrigin::Root.into(), total_rewards, planned_yielding_periods, blocks_per_period, BSX, BSX, caller.clone(), yield_per_period, 1_000, One::one())?
	}

	update_global_farm {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		lm_deposit_shares::<T>(liq_provider, ASSET_PAIR, 10 * ONE)?;
		set_period::<T>(200_000);
	}: {
		XYKLiquidityMining::<T>::update_global_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, FixedU128::from_inner(234_456_677_000_000_000_u128))?
	}

	terminate_global_farm {
		let total_rewards = 1_000_000 * ONE;
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		lm_deposit_shares::<T>(liq_provider, ASSET_PAIR, 10 * ONE)?;
		set_period::<T>(100_000);

		XYKLiquidityMining::<T>::stop_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, ASSET_PAIR)?;
		XYKLiquidityMining::<T>::terminate_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, YIELD_FARM_ID, ASSET_PAIR)?;
		set_period::<T>(200_000);
	}: {
		XYKLiquidityMining::<T>::terminate_global_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID)?
	}

	create_yield_farm {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);
		let bsx_dot = AssetPair {
			asset_in:  BSX,
			asset_out: DOT
		};

		initialize_pool::<T>(xyk_caller.clone(), ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		lm_deposit_shares::<T>(liq_provider, ASSET_PAIR, 10 * ONE)?;
		set_period::<T>(100_000);

		initialize_pool::<T>(xyk_caller, bsx_dot.asset_in, bsx_dot.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
	}: {
		XYKLiquidityMining::<T>::create_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, bsx_dot, FixedU128::from(50_000_000_u128), Some(LoyaltyCurve::default()))?
	}

	update_yield_farm {
		let new_multiplier = FixedU128::one();
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		lm_deposit_shares::<T>(liq_provider, ASSET_PAIR, 10 * ONE)?;
		set_period::<T>(100_000);
	}: {
		XYKLiquidityMining::<T>::update_yield_farm(RawOrigin::Signed(caller.clone()).into(), 1, ASSET_PAIR, new_multiplier)?
	}

	stop_yield_farm {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		lm_deposit_shares::<T>(liq_provider, ASSET_PAIR, 10 * ONE)?;
		set_period::<T>(100_000);
	}: {
		XYKLiquidityMining::<T>::stop_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, ASSET_PAIR)?
	}

	terminate_yield_farm {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		lm_deposit_shares::<T>(liq_provider, ASSET_PAIR, 10 * ONE)?;
		set_period::<T>(100_000);

		XYKLiquidityMining::<T>::stop_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, ASSET_PAIR)?;
	}: {
		XYKLiquidityMining::<T>::terminate_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID,YIELD_FARM_ID, ASSET_PAIR)?
	}

	deposit_shares {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		let xyk_id = xykpool::Pallet::<T>::pair_account_from_assets(ASSET_PAIR.asset_in, ASSET_PAIR.asset_out);
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		lm_create_global_farm::<T>(1_000_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller, GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		lm_deposit_shares::<T>(liq_provider.clone(), ASSET_PAIR, 5 * ONE)?;
		set_period::<T>(100_000);
	}: {
		XYKLiquidityMining::<T>::deposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID, YIELD_FARM_ID, ASSET_PAIR, 5 * ONE)?
	}

	redeposit_shares {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);
		let shares_amount = 10 * ONE;

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		//global id: 1, yield id: 2
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		//global id: 3, yield id: 4
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID_2, ASSET_PAIR, FixedU128::one())?;

		//global id: 5, yield id:6
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), 5, ASSET_PAIR, FixedU128::one())?;

		//global id: 7, yield id:8
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), 7, ASSET_PAIR, FixedU128::one())?;

		//global id: 9, yield id:10
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller, 9, ASSET_PAIR, FixedU128::one())?;

		set_period::<T>(200_000);

		XYKLiquidityMining::<T>::deposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID, YIELD_FARM_ID, ASSET_PAIR, shares_amount)?;
		//NOTE: with this redeposits it's like 0.5 µs slower(on my machine) because adding yield
		//farm entry into the deposit is doing search on BoundedVec<YieldFarmEntry, ...>
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID_2, YIELD_FARM_ID_2, ASSET_PAIR, DEPOSIT_ID)?;
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), 5, 6, ASSET_PAIR, DEPOSIT_ID)?;
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), 7, 8, ASSET_PAIR, DEPOSIT_ID)?;
	}: {
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), 9, 10, ASSET_PAIR, DEPOSIT_ID)?
	}

	claim_rewards {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);
		let shares_amount = 10 * ONE;

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		//global id: 1, yield id: 2
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(),GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		//global id: 3, yield id: 4
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID_2, ASSET_PAIR, FixedU128::one())?;

		//global id: 5, yield id:6
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), 5, ASSET_PAIR, FixedU128::one())?;

		//global id: 7, yield id:8
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller.clone(), 7, ASSET_PAIR, FixedU128::one())?;

		//global id: 9, yield id:10
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller, 9, ASSET_PAIR, FixedU128::one())?;

		set_period::<T>(200_000);

		XYKLiquidityMining::<T>::deposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID, YIELD_FARM_ID, ASSET_PAIR, shares_amount)?;
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID_2, YIELD_FARM_ID_2, ASSET_PAIR, DEPOSIT_ID)?;
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), 5, 6, ASSET_PAIR, DEPOSIT_ID)?;
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), 7, 8, ASSET_PAIR, DEPOSIT_ID)?;
		XYKLiquidityMining::<T>::redeposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), 9, 10, ASSET_PAIR, DEPOSIT_ID)?;

		set_period::<T>(400_000);
		let liq_provider_bsx_balance = MultiCurrencyOf::<T>::free_balance(BSX, &liq_provider);
	}: {
		XYKLiquidityMining::<T>::claim_rewards(RawOrigin::Signed(liq_provider.clone()).into(), DEPOSIT_ID, 10)?
	} verify {
		assert!(MultiCurrencyOf::<T>::free_balance(BSX, &liq_provider).gt(&liq_provider_bsx_balance));
	}

	//This benchmark has higher weights than:
	//  * withdraw_shares with farms removal from storage
	//  * withdraw_shares without deposit removal from storage
	withdraw_shares {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);
		let shares_amount = 10 * ONE;

		initialize_pool::<T>(xyk_caller, ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		xyk_add_liquidity::<T>(liq_provider.clone(), ASSET_PAIR, 10 * ONE, 1_000 * ONE)?;

		//global id: 1, yield id: 2
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		lm_create_yield_farm::<T>(caller, GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;

		set_period::<T>(200_000);

		XYKLiquidityMining::<T>::deposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID, YIELD_FARM_ID, ASSET_PAIR, shares_amount)?;

		set_period::<T>(400_000);

		let liq_provider_bsx_balance = MultiCurrencyOf::<T>::free_balance(BSX, &liq_provider);
	}: {
		XYKLiquidityMining::<T>::withdraw_shares(RawOrigin::Signed(liq_provider.clone()).into(), DEPOSIT_ID, YIELD_FARM_ID, ASSET_PAIR)?
	} verify {
		assert!(MultiCurrencyOf::<T>::free_balance(BSX, &liq_provider).gt(&liq_provider_bsx_balance));
	}

	resume_yield_farm {
		let caller = create_funded_account::<T>("caller", 0);
		let xyk_caller = create_funded_account::<T>("xyk_caller", 1);
		let liq_provider = create_funded_account::<T>("liq_provider", 2);
		let shares_amount = 10 * ONE;
		let bsx_dot = AssetPair {
			asset_in:  BSX,
			asset_out: DOT
		};

		initialize_pool::<T>(xyk_caller.clone(), ASSET_PAIR.asset_in, ASSET_PAIR.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		initialize_pool::<T>(xyk_caller, bsx_dot.asset_in, bsx_dot.asset_out, 1_000_000 * ONE, 10_000_000 * ONE)?;
		xyk_add_liquidity::<T>(liq_provider.clone(), bsx_dot, 10 * ONE, 1_000 * ONE)?;

		//global id: 1
		lm_create_global_farm::<T>(100_000 * ONE, caller.clone(), Perquintill::from_percent(20))?;
		//yield id: 2
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, ASSET_PAIR, FixedU128::one())?;
		//yield id: 3
		lm_create_yield_farm::<T>(caller.clone(), GLOBAL_FARM_ID, bsx_dot, FixedU128::one())?;

		XYKLiquidityMining::<T>::stop_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, ASSET_PAIR)?;

		set_period::<T>(200_000);

		XYKLiquidityMining::<T>::deposit_shares(RawOrigin::Signed(liq_provider.clone()).into(), GLOBAL_FARM_ID, 3, bsx_dot, shares_amount)?;

		set_period::<T>(400_000);
		let liq_provider_bsx_balance = MultiCurrencyOf::<T>::free_balance(BSX, &liq_provider);
	}: {
		XYKLiquidityMining::<T>::resume_yield_farm(RawOrigin::Signed(caller.clone()).into(), GLOBAL_FARM_ID, YIELD_FARM_ID, ASSET_PAIR, FixedU128::from(12_452))?
	}
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
