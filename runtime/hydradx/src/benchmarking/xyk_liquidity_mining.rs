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
use crate::{
	AccountId, AssetId, AssetRegistry as Registry, Balance, BlockNumber, Bonds, Currencies, EmaOracle,
	MaxSchedulesPerBlock, MultiTransactionPayment, NativeExistentialDeposit, Runtime, RuntimeOrigin, System,
	XYKLiquidityMining, XYKWarehouseLM, XYK,
};

use super::*;

use crate::benchmarking::register_asset;
use frame_benchmarking::{account, BenchmarkError};
use frame_support::{
	assert_ok,
	sp_runtime::{DispatchError, DispatchResult, FixedU128, Permill, Perquintill},
	traits::{OnFinalize, OnInitialize},
};
use frame_system::RawOrigin;
use hydradx_traits::AMM;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended, NamedMultiReservableCurrency};
use pallet_xyk::types::AssetPair;
use scale_info::prelude::vec::Vec;
use sp_runtime::traits::ConstU32;
use sp_std::vec;
use warehouse_liquidity_mining::{GlobalFarmId, LoyaltyCurve, YieldFarmId};

pub const HDX: AssetId = 0;

pub const ONE: Balance = 1_000_000_000_000;

pub const INITIAL_BALANCE: Balance = 10_000_000 * ONE;

fn create_gfarm(
	owner: AccountId,
	incentivized_asset: AssetId,
	reward_currency: AssetId,
	total_rewards: Balance,
) -> DispatchResult {
	let planned_yielding_periods = BlockNumber::from(1_000_000_u32);
	let yield_per_period = Perquintill::from_percent(20);
	let blocks_per_period = BlockNumber::from(1_u32);
	let min_deposit = 1_000;

	XYKLiquidityMining::create_global_farm(
		RawOrigin::Root.into(),
		total_rewards,
		planned_yielding_periods,
		blocks_per_period,
		incentivized_asset,
		reward_currency,
		owner,
		yield_per_period,
		min_deposit,
		FixedU128::one(),
	)
}

fn create_yfarm(caller: AccountId, farm_id: GlobalFarmId, assets: AssetPair, multiplier: FixedU128) -> DispatchResult {
	XYKLiquidityMining::create_yield_farm(
		RawOrigin::Signed(caller).into(),
		farm_id,
		assets,
		multiplier,
		Some(LoyaltyCurve::default()),
	)
}

fn xyk_add_liquidity(caller: AccountId, assets: AssetPair, amount_a: Balance, amount_b_max: Balance) -> DispatchResult {
	XYK::add_liquidity(
		RawOrigin::Signed(caller).into(),
		assets.asset_in,
		assets.asset_out,
		amount_a,
		amount_b_max,
	)
}

fn run_to_block(to: u32) {
	while System::block_number() < to {
		let b = System::block_number();

		System::on_finalize(b);
		EmaOracle::on_finalize(b);

		System::on_initialize(b + 1_u32);
		EmaOracle::on_initialize(b + 1_u32);

		System::set_block_number(b + 1_u32);
	}
}

runtime_benchmarks! {
	{Runtime, pallet_xyk_liquidity_mining }

	create_global_farm {
		let total_rewards = 1_000_000 * ONE;
		let planned_yielding_periods = BlockNumber::from(1_000_000_u32);
		let yield_per_period = Perquintill::from_percent(20);
		let blocks_per_period = BlockNumber::from(1_u32);
		let min_deposit = 1_000;
		let reward_currency = register_external_asset(b"FCK".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let owner = funded_account("caller", 0, &[HDX, reward_currency]);
	}: _(RawOrigin::Root,  total_rewards, planned_yielding_periods, blocks_per_period, HDX.into(), reward_currency, owner, yield_per_period, min_deposit, FixedU128::one())
	verify {
		assert!(XYKWarehouseLM::global_farm(1).is_some());
	}

	update_global_farm {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let farm_owner = funded_account("caller", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let liq_provider = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(liq_provider.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let gfarm_id = 1;
		let yfarm_id = 2;
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 10_000_000 * ONE)?;
		create_yfarm(farm_owner.clone(), gfarm_id, pair, FixedU128::one())?;

		run_to_block(200);
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(liq_provider).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;
		run_to_block(300);
	}: _(RawOrigin::Signed(farm_owner), gfarm_id, FixedU128::from_inner(234_456_677_000_000_000_u128))
	//NOTE: not verified because update prop is not public


	create_yield_farm {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let xyk_owner = funded_account("xyk", 0, &[HDX, pair.asset_in, pair.asset_out]);
		create_xyk_pool(xyk_owner, pair.asset_in, pair.asset_out);

		let farm_owner = funded_account("caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let global_farm_id = 1;
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 1_000_000 * ONE)?;
	}:  _(RawOrigin::Signed(farm_owner), global_farm_id, pair, FixedU128::one(), Some(LoyaltyCurve::default()))
	verify {
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(pair);

		assert!(XYKWarehouseLM::active_yield_farm(amm_pool_id, global_farm_id).is_some());
	}


}

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

fn create_xyk_pool(caller: AccountId, asset_a: u32, asset_b: u32) {
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		caller.clone(),
		0,
		10 * ONE as i128,
	));

	let amount = 100_000 * ONE;
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
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
