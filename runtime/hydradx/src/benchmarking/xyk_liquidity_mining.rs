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
	AccountId, AssetId, Balance, BlockNumber, Currencies, EmaOracle, Runtime, System, XYKLiquidityMining,
	XYKWarehouseLM, XYK,
};

use sp_core::Get;

use super::*;

use frame_benchmarking::{account, BenchmarkError};
use frame_support::{
	assert_ok,
	sp_runtime::{DispatchResult, FixedU128, Perquintill},
	traits::{OnFinalize, OnInitialize},
};
use frame_system::RawOrigin;
use hydradx_traits::AMM;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_xyk::types::AssetPair;
use sp_std::vec;
use warehouse_liquidity_mining::{GlobalFarmId, LoyaltyCurve};

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
		0,
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
	}: _(RawOrigin::Root,  total_rewards, planned_yielding_periods, blocks_per_period, HDX, reward_currency, owner, yield_per_period, min_deposit, FixedU128::one())
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
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(farm_owner.clone(), gfarm_id, pair, FixedU128::one())?;

		run_to_block(200);
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(liq_provider).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;
		run_to_block(300);
	}: _(RawOrigin::Signed(farm_owner), gfarm_id, FixedU128::from_inner(234_456_677_000_000_000_u128))
	//NOTE: not verified because update prop is not public

	terminate_global_farm {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let farm_owner = funded_account("caller", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let liq_provider = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(liq_provider, pair, 1_000 * ONE, 100_000 * ONE)?;


		let gfarm_id = 1;
		let yfarm_id = 2;
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(farm_owner.clone(), gfarm_id, pair, FixedU128::one())?;

		run_to_block(300);
		XYKLiquidityMining::stop_yield_farm(RawOrigin::Signed(farm_owner.clone()).into(), gfarm_id, pair)?;
		XYKLiquidityMining::terminate_yield_farm(RawOrigin::Signed(farm_owner.clone()).into(), gfarm_id, yfarm_id, pair)?;
		run_to_block(400);
	}: _(RawOrigin::Signed(farm_owner), gfarm_id)
	//NOTE: farm is removed from storage lazylly and prop to check is private

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

	update_yield_farm {
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
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(farm_owner.clone(), gfarm_id, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(liq_provider).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;
		run_to_block(300);
	}: _(RawOrigin::Signed(farm_owner), gfarm_id, pair, FixedU128::one())
	//NOTE: updated field is not public

	stop_yield_farm {
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
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(farm_owner.clone(), gfarm_id, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(liq_provider).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;
		run_to_block(300);
	}: _(RawOrigin::Signed(farm_owner), gfarm_id, pair)

	terminate_yield_farm {
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
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(farm_owner.clone(), gfarm_id, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(liq_provider).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;
		run_to_block(300);

		XYKLiquidityMining::stop_yield_farm(RawOrigin::Signed(farm_owner.clone()).into(), gfarm_id, pair)?;
	}: _(RawOrigin::Signed(farm_owner), gfarm_id, yfarm_id, pair)

	deposit_shares {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let farm_owner = funded_account("caller", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let liq_provider = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let liq_provider2 = funded_account("lp2", 3, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(liq_provider.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(liq_provider2.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;


		let gfarm_id = 1;
		let yfarm_id = 2;
		create_gfarm(farm_owner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(farm_owner, gfarm_id, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(liq_provider).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;
		run_to_block(300);

		assert!(XYKWarehouseLM::deposit(2).is_none());
	}: _(RawOrigin::Signed(liq_provider2), gfarm_id, yfarm_id, pair, 10 * ONE)
	verify {
		assert!(XYKWarehouseLM::deposit(2).is_some());
	}

	redeposit_shares {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let fowner1 = funded_account("fowner1", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner2 = funded_account("fowner2", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner3 = funded_account("fowner3", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner4 = funded_account("fowner4", 3, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner5 = funded_account("fowner5", 4, &[HDX, pair.asset_in, pair.asset_out]);


		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let lp1 = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let lp2 = funded_account("lp2", 3, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(lp1.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(lp2.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let lp1_deposit_id = 1;
		let gfarm_id1 = 1;
		let yfarm_id1 = 2;

		//gId: 1, yId: 2
		create_gfarm(fowner1.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner1, 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 3, yId: 4
		create_gfarm(fowner2.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner2, 3, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 5, yId: 6
		create_gfarm(fowner3.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner3, 5, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 7, yId: 8
		create_gfarm(fowner4.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner4, 7, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 9, yId: 10
		create_gfarm(fowner5.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner5, 9, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp1.clone()).into(), gfarm_id1, yfarm_id1, pair, 10 * ONE)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, pair, lp1_deposit_id)?;

		//Deposit into the global-farm so it will be updated
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp2).into(), 9, 10, pair, 10 * ONE)?;

		run_to_block(400);
	}: _(RawOrigin::Signed(lp1), 9, 10, pair, lp1_deposit_id)

	claim_rewards {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let fowner1 = funded_account("fowner1", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner2 = funded_account("fowner2", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner3 = funded_account("fowner3", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner4 = funded_account("fowner4", 3, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner5 = funded_account("fowner5", 4, &[HDX, pair.asset_in, pair.asset_out]);


		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let lp1 = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let lp2 = funded_account("lp2", 3, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(lp1.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(lp2.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let lp1_deposit_id = 1;
		let gfarm_id1 = 1;
		let yfarm_id1 = 2;

		//gId: 1, yId: 2
		create_gfarm(fowner1.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner1, 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 3, yId: 4
		create_gfarm(fowner2.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner2, 3, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 5, yId: 6
		create_gfarm(fowner3.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner3, 5, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 7, yId: 8
		create_gfarm(fowner4.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner4, 7, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 9, yId: 10
		create_gfarm(fowner5.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner5, 9, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp1.clone()).into(), gfarm_id1, yfarm_id1, pair, 10 * ONE)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, pair, lp1_deposit_id)?;

		//Deposit into the global-farm so it will be updated
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp2).into(), 9, 10, pair, 10 * ONE)?;

		run_to_block(400);
		let lp1_rew_curr_balance = Currencies::free_balance(pair.asset_out, &lp1);
	}: _(RawOrigin::Signed(lp1.clone()), lp1_deposit_id, yfarm_id1)
	verify {
		assert!(Currencies::free_balance(pair.asset_out, &lp1).gt(&lp1_rew_curr_balance));
	}

	withdraw_shares {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let fowner1 = funded_account("fowner", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let lp = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(lp.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let lp_deposit_id = 1;
		let gfarm_id = 1;
		let yfarm_id = 2;

		//gId: 1, yId: 2
		create_gfarm(fowner1.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner1, 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp.clone()).into(), gfarm_id, yfarm_id, pair, 10 * ONE)?;

		run_to_block(400);

		let lp_rew_curr_balance = Currencies::free_balance(pair.asset_out, &lp);
	}: _(RawOrigin::Signed(lp.clone()), lp_deposit_id, yfarm_id, pair)
	verify {
		assert!(Currencies::free_balance(pair.asset_out, &lp).gt(&lp_rew_curr_balance));
	}

	resume_yield_farm {
		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		//NOTE: pair.asset_in is incentivized asset
		let pair2 = AssetPair {
			asset_in: pair.asset_in,
			asset_out: register_external_asset(b"TKN3".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
		};

		let fowner = funded_account("fowner", 0, &[HDX, pair.asset_in, pair.asset_out, pair2.asset_out]);
		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out, pair2.asset_out]);
		let lp = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out, pair2.asset_out]);

		create_xyk_pool(xyk_caller.clone(), pair.asset_in, pair.asset_out);
		let xyk_id_1 = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);

		create_xyk_pool(xyk_caller, pair2.asset_in, pair2.asset_out);
		let xyk_id_2 = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);

		xyk_add_liquidity(lp.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(lp.clone(), pair2, 1_000 * ONE, 100_000 * ONE)?;

		let lp_deposit_id = 1;
		let gfarm_id = 1;
		let yfarm_id1 = 2;
		let yfarm_id2 = 3;

		//gId: 1, yId: 2
		create_gfarm(fowner.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner.clone(), 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;
		create_yfarm(fowner.clone(), 1, pair2, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::stop_yield_farm(RawOrigin::Signed(fowner.clone()).into(), gfarm_id, pair)?;

		run_to_block(300);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp).into(), gfarm_id, yfarm_id2, pair2, 10 * ONE)?;

		run_to_block(400);
	}: _(RawOrigin::Signed(fowner), gfarm_id, yfarm_id1, pair, FixedU128::from(12_452))

	join_farms {
		let c in 1..get_max_entries::<Runtime>();

		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let fowner1 = funded_account("fowner1", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner2 = funded_account("fowner2", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner3 = funded_account("fowner3", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner4 = funded_account("fowner4", 3, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner5 = funded_account("fowner5", 4, &[HDX, pair.asset_in, pair.asset_out]);


		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let lp1 = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let lp2 = funded_account("lp2", 3, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(lp1.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(lp2.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let lp1_deposit_id = 1;
		let gfarm_id1 = 1;
		let yfarm_id1 = 2;

		//gId: 1, yId: 2
		create_gfarm(fowner1.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner1, 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 3, yId: 4
		create_gfarm(fowner2.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner2, 3, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 5, yId: 6
		create_gfarm(fowner3.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner3, 5, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 7, yId: 8
		create_gfarm(fowner4.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner4, 7, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 9, yId: 10
		create_gfarm(fowner5.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner5, 9, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp1.clone()).into(), gfarm_id1, yfarm_id1, pair, 10 * ONE)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, pair, lp1_deposit_id)?;

		//Deposit into the global-farm so it will be updated
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp2).into(), 9, 10, pair, 10 * ONE)?;

		let farms_entries = [(1,2), (3,4), (5,6), (7,8), (9, 10)];
		let farms = farms_entries[0..c as usize].to_vec();

		run_to_block(400);
	}: _(RawOrigin::Signed(lp1), farms.try_into().unwrap(),pair,  10 * ONE)

	add_liquidity_and_join_farms {
		let c in 1..get_max_entries::<Runtime>();

		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let fowner1 = funded_account("fowner1", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner2 = funded_account("fowner2", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner3 = funded_account("fowner3", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner4 = funded_account("fowner4", 3, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner5 = funded_account("fowner5", 4, &[HDX, pair.asset_in, pair.asset_out]);

		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let lp1 = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let lp2 = funded_account("lp2", 3, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(lp1.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(lp2.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let lp1_deposit_id = 1;
		let gfarm_id1 = 1;
		let yfarm_id1 = 2;

		//gId: 1, yId: 2
		create_gfarm(fowner1.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner1, 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 3, yId: 4
		create_gfarm(fowner2.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner2, 3, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 5, yId: 6
		create_gfarm(fowner3.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner3, 5, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 7, yId: 8
		create_gfarm(fowner4.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner4, 7, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 9, yId: 10
		create_gfarm(fowner5.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner5, 9, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp1.clone()).into(), gfarm_id1, yfarm_id1, pair, 10 * ONE)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, pair, lp1_deposit_id)?;

		//Deposit into the yield-farm so it will be updated
		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp2).into(), 9, 10, pair, 10 * ONE)?;

		let farms_entries = [(1,2), (3,4), (5,6), (7,8), (9, 10)];
		let farms = farms_entries[0..c as usize].to_vec();

		run_to_block(400);
	}: _(RawOrigin::Signed(lp1),pair.asset_in, pair.asset_out, ONE, 10 * ONE, farms.try_into().unwrap())

	exit_farms {
		let c in 1..get_max_entries::<Runtime>();

		let pair = AssetPair {
			asset_in: register_external_asset(b"TKN1".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?,
			asset_out: register_external_asset(b"TKN2".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?
		};

		let fowner1 = funded_account("fowner1", 0, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner2 = funded_account("fowner2", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner3 = funded_account("fowner3", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner4 = funded_account("fowner4", 3, &[HDX, pair.asset_in, pair.asset_out]);
		let fowner5 = funded_account("fowner5", 4, &[HDX, pair.asset_in, pair.asset_out]);


		let xyk_caller = funded_account("xyk_caller", 1, &[HDX, pair.asset_in, pair.asset_out]);
		let lp1 = funded_account("liq_provider", 2, &[HDX, pair.asset_in, pair.asset_out]);
		let lp2 = funded_account("lp2", 3, &[HDX, pair.asset_in, pair.asset_out]);

		create_xyk_pool(xyk_caller, pair.asset_in, pair.asset_out);
		let xyk_id = XYK::pair_account_from_assets(pair.asset_in, pair.asset_out);
		xyk_add_liquidity(lp1.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;
		xyk_add_liquidity(lp2.clone(), pair, 1_000 * ONE, 100_000 * ONE)?;

		let lp1_deposit_id = 1;
		let gfarm_id1 = 1;
		let yfarm_id1 = 2;

		//gId: 1, yId: 2
		create_gfarm(fowner1.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner1, 1, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 3, yId: 4
		create_gfarm(fowner2.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner2, 3, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 5, yId: 6
		create_gfarm(fowner3.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner3, 5, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 7, yId: 8
		create_gfarm(fowner4.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner4, 7, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		//gId: 9, yId: 10
		create_gfarm(fowner5.clone(), pair.asset_in, pair.asset_out, 9_000_000 * ONE)?;
		create_yfarm(fowner5, 9, pair, FixedU128::from_inner(500_000_000_000_000_000_u128))?;

		run_to_block(200);

		XYKLiquidityMining::deposit_shares(RawOrigin::Signed(lp1.clone()).into(), gfarm_id1, yfarm_id1, pair, 10 * ONE)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, pair, lp1_deposit_id)?;
		XYKLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 9, 10, pair, lp1_deposit_id)?;

		let farm_entries = [yfarm_id1, 4, 6, 8, 10];
		let farms = farm_entries[0..c as usize].to_vec();

		run_to_block(400);
	}: _(RawOrigin::Signed(lp1),lp1_deposit_id, pair, farms.try_into().unwrap())
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

fn get_max_entries<T: pallet_xyk_liquidity_mining::Config>() -> u32 {
	T::MaxFarmEntriesPerDeposit::get()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
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

		<pallet_xyk_liquidity_mining::GenesisConfig<crate::Runtime> as BuildStorage>::assimilate_storage(
			&pallet_xyk_liquidity_mining::GenesisConfig::<crate::Runtime>::default(),
			&mut t,
		)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
