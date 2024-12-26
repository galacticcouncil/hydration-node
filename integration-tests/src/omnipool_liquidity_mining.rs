// This file is part of HydraDX-node.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::storage::with_transaction;
use hydradx_runtime::Omnipool;
use hydradx_runtime::{
	AssetRegistry, Balance, Bonds, Currencies, Runtime, RuntimeEvent, RuntimeOrigin, Stableswap, Treasury,
	TreasuryAccount,
};
use hydradx_traits::liquidity_mining::PriceAdjustment;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_asset_registry::AssetType;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use pretty_assertions::assert_eq;
use primitives::constants::time::unix_time::MONTH;
use primitives::{constants::currency::UNITS, AssetId};
use sp_runtime::DispatchResult;
use sp_runtime::TransactionOutcome;
use sp_runtime::{
	traits::{One, Zero},
	DispatchError, FixedPointNumber, FixedU128, Permill, Perquintill,
};
use warehouse_liquidity_mining::{
	DefaultPriceAdjustment, DepositData, GlobalFarmData, GlobalFarmId, Instance1, LoyaltyCurve, YieldFarmData,
	YieldFarmEntry,
};
use xcm_emulator::TestExt;

#[macro_export]
macro_rules! assert_nft_owner {
	( $coll:expr, $item: expr, $acc:expr ) => {{
		assert_eq!(hydradx_runtime::Uniques::owner($coll, $item).unwrap(), $acc);
	}};
}

#[test]
fn create_global_farm_should_work_when_origin_is_root() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let total_rewards: Balance = 1_000_000 * UNITS;
		let planned_yielding_periods: BlockNumber = 1_000_000;
		let blocks_per_period: BlockNumber = 10;
		let reward_currency = HDX;
		let owner = Treasury::account_id();
		let yield_per_period = Perquintill::from_parts(570_776_255_707);
		let min_deposit = 1_000;

		assert_ok!(hydradx_runtime::Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			owner.clone(),
			total_rewards,
		));

		set_relaychain_block_number(100);

		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::create_global_farm(
			hydradx_runtime::RuntimeOrigin::root(),
			total_rewards,
			planned_yielding_periods,
			blocks_per_period,
			reward_currency,
			owner.clone(),
			yield_per_period,
			min_deposit,
			FixedU128::from(2)
		));

		let farm_id = 1;
		let updated_at = 100 / blocks_per_period;
		assert_eq!(
			hydradx_runtime::OmnipoolWarehouseLM::global_farm(1).unwrap(),
			GlobalFarmData::new(
				farm_id,
				updated_at,
				reward_currency,
				yield_per_period,
				planned_yielding_periods,
				blocks_per_period,
				owner,
				LRNA,
				total_rewards / planned_yielding_periods as u128,
				min_deposit,
				FixedU128::from(2),
			)
		);

		let g_farm_account = hydradx_runtime::OmnipoolWarehouseLM::farm_account_id(farm_id).unwrap();
		assert_eq!(hydradx_runtime::Balances::free_balance(g_farm_account), total_rewards);
	});
}

#[test]
fn create_yield_farm_should_work_when_asset_is_in_omnipool() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_id = 1;
		let created_yield_farm_id = 2;
		let loyalty_curve = Some(LoyaltyCurve::default());
		let multiplier = FixedU128::one();

		init_omnipool();

		set_relaychain_block_number(100);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::create_yield_farm(
			RuntimeOrigin::signed(Treasury::account_id()),
			global_farm_id,
			BTC,
			multiplier,
			loyalty_curve.clone()
		));

		let updated_at = 20;
		let y_farm = warehouse_liquidity_mining::YieldFarm::<hydradx_runtime::Runtime, Instance1>::get((
			BTC,
			global_farm_id,
			created_yield_farm_id,
		))
		.unwrap();
		assert_eq!(
			y_farm,
			YieldFarmData::new(created_yield_farm_id, updated_at, loyalty_curve, multiplier)
		);
	});
}

#[test]
fn deposit_shares_should_work_when_yield_farm_exists() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_id = 1;
		let yield_farm_id = 2;

		//Arrange
		init_omnipool();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		//Act
		set_relaychain_block_number(400);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_id,
			yield_farm_id,
			position_id
		));

		//Assert
		let deposit = hydradx_runtime::OmnipoolWarehouseLM::deposit(1).unwrap();
		let mut expected_deposit = DepositData::new(1_000_000_000_000_000, ETH);
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_id,
				yield_farm_id,
				71_145_071_145_u128,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);

		//assert LM deposit
		assert_nft_owner!(hydradx_runtime::OmnipoolLMCollectionId::get(), 1, CHARLIE.into());
		//original position owner should be palelt account
		let lm_account = hydradx_runtime::OmnipoolLiquidityMining::account_id();
		assert_nft_owner!(hydradx_runtime::OmnipoolCollectionId::get(), position_id, lm_account);
	});
}

#[test]
fn redeposit_shares_multiple_times_should_work_when_shares_already_deposited() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		//Act
		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			deposit_id
		));

		let deposit = hydradx_runtime::OmnipoolWarehouseLM::deposit(deposit_id).unwrap();
		let mut expected_deposit = DepositData::new(1_000_000_000_000_000, ETH);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				71_145_071_145_u128,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//2-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_2_id,
				yield_farm_2_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				50,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);
	});
}

#[test]
fn join_farms_should_work_for_multiple_farms() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);
		create_yield_farm(global_farm_3_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		let farms = vec![
			(global_farm_1_id, yield_farm_1_id),
			(global_farm_2_id, yield_farm_2_id),
			(global_farm_3_id, yield_farm_3_id),
		];
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::join_farms(
			RuntimeOrigin::signed(CHARLIE.into()),
			farms.try_into().unwrap(),
			position_id
		));

		//Assert that the ownership of the nft should be transferred to omnipool account
		let lm_account = hydradx_runtime::OmnipoolLiquidityMining::account_id();
		assert_nft_owner!(hydradx_runtime::OmnipoolCollectionId::get(), position_id, lm_account);

		set_relaychain_block_number(500);

		let deposit = hydradx_runtime::OmnipoolWarehouseLM::deposit(deposit_id).unwrap();
		let mut expected_deposit = DepositData::new(1_000_000_000_000_000, ETH);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				71_145_071_145_u128,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//2-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_2_id,
				yield_farm_2_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//3-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_3_id,
				yield_farm_3_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);
	});
}

#[test]
fn add_liquidity_and_join_farms_should_work_for_multiple_farms() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);
		create_yield_farm(global_farm_3_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		//Add some liquidiity to make sure that it does not interfere with the new liquidty add
		assert_ok!(hydradx_runtime::Omnipool::add_liquidity(
			RuntimeOrigin::signed(CHARLIE.into()),
			ETH,
			100 * UNITS,
		));

		let position_id = hydradx_runtime::Omnipool::next_position_id();

		set_relaychain_block_number(400);
		let deposit_id = 1;
		let farms = vec![
			(global_farm_1_id, yield_farm_1_id),
			(global_farm_2_id, yield_farm_2_id),
			(global_farm_3_id, yield_farm_3_id),
		];
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::add_liquidity_and_join_farms(
			RuntimeOrigin::signed(CHARLIE.into()),
			farms.try_into().unwrap(),
			ETH,
			1_000 * UNITS,
		));

		//Assert that the ownership of the nft should be transferred to omnipool account
		let lm_account = hydradx_runtime::OmnipoolLiquidityMining::account_id();
		assert_nft_owner!(hydradx_runtime::OmnipoolCollectionId::get(), position_id, lm_account);

		set_relaychain_block_number(500);

		let deposit = hydradx_runtime::OmnipoolWarehouseLM::deposit(deposit_id).unwrap();
		let mut expected_deposit = DepositData::new(1_000_000_000_000_000, ETH);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				71_145_071_145_u128,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//2-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_2_id,
				yield_farm_2_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//3-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_3_id,
				yield_farm_3_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);
	});
}

#[test]
fn add_liquidity_with_limit_and_join_farms_should_work_for_multiple_farms() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);
		create_yield_farm(global_farm_3_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		//Add some liquidiity to make sure that it does not interfere with the new liquidty add
		assert_ok!(hydradx_runtime::Omnipool::add_liquidity(
			RuntimeOrigin::signed(CHARLIE.into()),
			ETH,
			100 * UNITS,
		));

		let position_id = hydradx_runtime::Omnipool::next_position_id();

		set_relaychain_block_number(400);
		let deposit_id = 1;
		let farms = vec![
			(global_farm_1_id, yield_farm_1_id),
			(global_farm_2_id, yield_farm_2_id),
			(global_farm_3_id, yield_farm_3_id),
		];
		assert_ok!(
			hydradx_runtime::OmnipoolLiquidityMining::add_liquidity_with_limit_and_join_farms(
				RuntimeOrigin::signed(CHARLIE.into()),
				farms.try_into().unwrap(),
				ETH,
				1_000 * UNITS,
				1_000 * UNITS
			)
		);

		//Assert that the ownership of the nft should be transferred to omnipool account
		let lm_account = hydradx_runtime::OmnipoolLiquidityMining::account_id();
		assert_nft_owner!(hydradx_runtime::OmnipoolCollectionId::get(), position_id, lm_account);

		set_relaychain_block_number(500);

		let deposit = hydradx_runtime::OmnipoolWarehouseLM::deposit(deposit_id).unwrap();
		let mut expected_deposit = DepositData::new(1_000_000_000_000_000, ETH);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				71_145_071_145_u128,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//2-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_2_id,
				yield_farm_2_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		//3-nd redeposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_3_id,
				yield_farm_3_id,
				71_145_071_145_u128, //NOTE: nothing changed in omnipool so shares are
				//valued same as before
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);

		expect_omnipool_liquidity_added_events(vec![pallet_omnipool::Event::LiquidityAdded {
			who: CHARLIE.into(),
			asset_id: ETH,
			amount: 1_000 * UNITS,
			position_id: 7,
		}
		.into()]);

		expect_lm_events(vec![
			pallet_omnipool_liquidity_mining::Event::SharesDeposited {
				global_farm_id: global_farm_1_id,
				yield_farm_id: yield_farm_1_id,
				deposit_id: 1,
				asset_id: ETH,
				who: CHARLIE.into(),
				shares_amount: 1_000 * UNITS,
				position_id: 7,
			}
			.into(),
			pallet_omnipool_liquidity_mining::Event::SharesRedeposited {
				global_farm_id: global_farm_2_id,
				yield_farm_id: yield_farm_2_id,
				deposit_id: 1,
				asset_id: ETH,
				who: CHARLIE.into(),
				shares_amount: 1_000 * UNITS,
				position_id: 7,
			}
			.into(),
			pallet_omnipool_liquidity_mining::Event::SharesRedeposited {
				global_farm_id: global_farm_3_id,
				yield_farm_id: yield_farm_3_id,
				deposit_id: 1,
				asset_id: ETH,
				who: CHARLIE.into(),
				shares_amount: 1_000 * UNITS,
				position_id: 7,
			}
			.into(),
		]);
	});
}

#[test]
fn add_liquidity_with_limit_and_join_farms_should_fail_when_reaches_limit() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);
		create_yield_farm(global_farm_3_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		//Add some liquidiity to make sure that it does not interfere with the new liquidty add
		assert_ok!(hydradx_runtime::Omnipool::add_liquidity(
			RuntimeOrigin::signed(CHARLIE.into()),
			ETH,
			100 * UNITS,
		));

		set_relaychain_block_number(400);
		let farms = vec![
			(global_farm_1_id, yield_farm_1_id),
			(global_farm_2_id, yield_farm_2_id),
			(global_farm_3_id, yield_farm_3_id),
		];
		assert_noop!(
			hydradx_runtime::OmnipoolLiquidityMining::add_liquidity_with_limit_and_join_farms(
				RuntimeOrigin::signed(CHARLIE.into()),
				farms.try_into().unwrap(),
				ETH,
				1_000 * UNITS,
				1_000 * UNITS + 1
			),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::SlippageLimit
		);
	});
}

#[test]
fn add_liquidity_stableswap_omnipool_and_join_farms_should_work_for_multiple_farms() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			let global_farm_1_id = 1;
			let global_farm_2_id = 2;
			let global_farm_3_id = 3;
			let yield_farm_1_id = 4;
			let yield_farm_2_id = 5;
			let yield_farm_3_id = 6;

			//Arrange
			let (stable_pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

			init_omnipool();
			seed_lm_pot();

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				stable_pool_id,
				30_000_000 * UNITS as i128,
			));

			assert_ok!(Omnipool::add_token(
				RuntimeOrigin::root(),
				stable_pool_id,
				FixedU128::from_rational(50, 100),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			//NOTE: necessary to get oracle price.
			hydradx_run_to_block(100);
			set_relaychain_block_number(100);
			create_global_farm(None, None);
			create_global_farm(None, None);
			create_global_farm(None, None);

			set_relaychain_block_number(200);
			create_yield_farm(global_farm_1_id, stable_pool_id);
			create_yield_farm(global_farm_2_id, stable_pool_id);
			create_yield_farm(global_farm_3_id, stable_pool_id);

			set_relaychain_block_number(300);

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				CHARLIE.into(),
				ETH,
				10_000 * UNITS as i128,
			));

			//Add some liquidiity to make sure that it does not interfere with the new liquidty add
			assert_ok!(hydradx_runtime::Omnipool::add_liquidity(
				RuntimeOrigin::signed(CHARLIE.into()),
				ETH,
				100 * UNITS,
			));

			let position_id = hydradx_runtime::Omnipool::next_position_id();

			set_relaychain_block_number(400);
			let deposit_id = 1;
			let farms = vec![
				(global_farm_1_id, yield_farm_1_id),
				(global_farm_2_id, yield_farm_2_id),
				(global_farm_3_id, yield_farm_3_id),
			];

			//Act
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				CHARLIE.into(),
				stable_asset_1,
				100 * UNITS as i128,
			));
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				CHARLIE.into(),
				stable_asset_2,
				100 * UNITS as i128,
			));
			assert_ok!(
				hydradx_runtime::OmnipoolLiquidityMining::add_liquidity_stableswap_omnipool_and_join_farms(
					RuntimeOrigin::signed(CHARLIE.into()),
					stable_pool_id,
					vec![
						AssetAmount::new(stable_asset_1, 10 * UNITS),
						AssetAmount::new(stable_asset_2, 10 * UNITS)
					]
					.try_into()
					.unwrap(),
					farms.try_into().unwrap()
				)
			);

			//Assert that the ownership of the nft should be transferred to omnipool account
			let lm_account = hydradx_runtime::OmnipoolLiquidityMining::account_id();
			assert_nft_owner!(hydradx_runtime::OmnipoolCollectionId::get(), position_id, lm_account);

			set_relaychain_block_number(500);

			let deposit = hydradx_runtime::OmnipoolWarehouseLM::deposit(deposit_id).unwrap();
			let mut expected_deposit = DepositData::new(20044549999405, stable_pool_id);
			//1-th deposit entry
			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					global_farm_1_id,
					yield_farm_1_id,
					10022274999702,
					FixedU128::zero(),
					40,
					0,
				))
				.unwrap();

			//2-nd redeposit entry
			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					global_farm_2_id,
					yield_farm_2_id,
					10022274999702, //NOTE: nothing changed in omnipool so shares are
					//valued same as before
					FixedU128::zero(),
					40,
					0,
				))
				.unwrap();

			//3-nd redeposit entry
			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					global_farm_3_id,
					yield_farm_3_id,
					10022274999702, //NOTE: nothing changed in omnipool so shares are
					//valued same as before
					FixedU128::zero(),
					40,
					0,
				))
				.unwrap();

			assert_eq!(deposit, expected_deposit);

			expect_stableswap_liquidity_added_events(vec![pallet_stableswap::Event::LiquidityAdded {
				pool_id: stable_pool_id,
				who: CHARLIE.into(),
				shares: 20044549999405,
				assets: vec![
					AssetAmount::new(stable_asset_1, 10 * UNITS),
					AssetAmount::new(stable_asset_2, 10 * UNITS),
				],
			}
			.into()]);

			let stableswap_shares_amount = 20044549999405;
			expect_omnipool_liquidity_added_events(vec![pallet_omnipool::Event::LiquidityAdded {
				who: CHARLIE.into(),
				asset_id: stable_pool_id,
				amount: stableswap_shares_amount,
				position_id: 8,
			}
			.into()]);

			expect_lm_events(vec![
				pallet_omnipool_liquidity_mining::Event::SharesDeposited {
					global_farm_id: global_farm_1_id,
					yield_farm_id: yield_farm_1_id,
					deposit_id: 1,
					asset_id: stable_pool_id,
					who: CHARLIE.into(),
					shares_amount: stableswap_shares_amount,
					position_id: 8,
				}
				.into(),
				pallet_omnipool_liquidity_mining::Event::SharesRedeposited {
					global_farm_id: global_farm_2_id,
					yield_farm_id: yield_farm_2_id,
					deposit_id: 1,
					asset_id: stable_pool_id,
					who: CHARLIE.into(),
					shares_amount: stableswap_shares_amount,
					position_id: 8,
				}
				.into(),
				pallet_omnipool_liquidity_mining::Event::SharesRedeposited {
					global_farm_id: global_farm_3_id,
					yield_farm_id: yield_farm_3_id,
					deposit_id: 1,
					asset_id: stable_pool_id,
					who: CHARLIE.into(),
					shares_amount: stableswap_shares_amount,
					position_id: 8,
				}
				.into(),
			]);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn withdraw_shares_should_work_when_deposit_exists() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;

		//Arrange
		init_omnipool();

		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			deposit_id
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance1>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		let bob_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		//Act 1 - withdraw shares from 2-nd yield-farm
		set_relaychain_block_number(600);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_2_id
		));

		//Assert
		//NOTE: withdraw is claiming rewards automatically
		let expected_claimed_amount = 184_024_112_u128;
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into()),
			bob_hdx_balance_0 + expected_claimed_amount
		);

		//NOTE:	omnipool position should not be unlocked because deposit wasn't destroyed(it has 1
		//yield-farm-entry left)
		//assert LM deposit
		assert_nft_owner!(hydradx_runtime::OmnipoolLMCollectionId::get(), 1, CHARLIE.into());
		//original position owner should be palelt account
		let lm_account = hydradx_runtime::OmnipoolLiquidityMining::account_id();
		assert_nft_owner!(hydradx_runtime::OmnipoolCollectionId::get(), position_id, lm_account);

		//Check if yield-farm-entry was removed from the deposit.
		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance1>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_none()
		);

		set_relaychain_block_number(700);
		//Arrange - claim before withdraw
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_1_id
		),);

		let charlie_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		//Act 2 - claim and withdraw should in the same period should work.
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_1_id
		));

		//Assert
		//NOTE: claim happened before withdraw in this period so no rewards should be claimed.

		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into()),
			charlie_hdx_balance_0
		);

		//NOTE: last shares were unlockend and deposit's nft should be destroyed and omnipool's
		//position should be unlocked.
		assert!(warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance1>::get(deposit_id).is_none());
		//LM nft should be destroyed
		assert!(hydradx_runtime::Uniques::owner(hydradx_runtime::OmnipoolLMCollectionId::get(), deposit_id).is_none());
		//omnpool's position should be unlocekd
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);
	});
}

#[test]
fn withdraw_shares_should_send_reward_to_user_when_bigger_than_ed_but_user_has_no_reward_balance() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;

		//Arrange
		init_omnipool();

		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();
		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);

		create_global_farm(None, Some(Perquintill::from_percent(40)));
		create_global_farm(None, Some(Perquintill::from_percent(40)));

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			deposit_id
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance1>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		//We make sure that charlie has 0 HDX so reward (which is less than ED) can not be sent to him
		//We also make sure that treasury has some balance so we don't trigger BelowMinimum error because treasury balance is below ED
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CHARLIE.into()),
			Treasury::account_id(),
			HDX,
			1000 * UNITS,
		));

		//Act
		set_relaychain_block_number(1000);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_2_id
		));

		//Assert
		let expected_claimed_amount = 33_333_333_333_331;
		std::assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into()),
			expected_claimed_amount
		);

		expect_reward_claimed_events(vec![pallet_omnipool_liquidity_mining::Event::RewardClaimed {
			global_farm_id: global_farm_2_id,
			yield_farm_id: yield_farm_2_id,
			who: AccountId::from(CHARLIE),
			claimed: expected_claimed_amount,
			reward_currency: HDX,
			deposit_id: 1,
		}
		.into()]);
	});
}

#[test]
fn withdraw_shares_should_send_reward_to_user_when_reward_is_less_than_ed_but_user_has_balance() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;

		//Arrange
		init_omnipool();

		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();
		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);

		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			deposit_id
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance1>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		//Act
		set_relaychain_block_number(600);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_2_id
		));

		let expected_claimed_amount = 184_024_112_u128;
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into()),
			1000 * UNITS + expected_claimed_amount
		);

		expect_reward_claimed_events(vec![pallet_omnipool_liquidity_mining::Event::RewardClaimed {
			global_farm_id: global_farm_2_id,
			yield_farm_id: yield_farm_2_id,
			who: AccountId::from(CHARLIE),
			claimed: expected_claimed_amount,
			reward_currency: HDX,
			deposit_id: 1,
		}
		.into()]);
	});
}

#[test]
fn withdraw_shares_should_send_reward_to_treasury_when_reward_is_less_than_ed_and_user_has_no_balance() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;

		//Arrange
		init_omnipool();

		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();
		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);

		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			deposit_id
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance1>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		//We make sure that charlie has 0 HDX so reward (which is less than ED) can not be sent to him
		//We also make sure that treasury has some balance so we don't trigger BelowMinimum error because treasury balance is below ED
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CHARLIE.into()),
			Treasury::account_id(),
			HDX,
			1000 * UNITS,
		));

		let charlie_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		assert_eq!(charlie_hdx_balance_0, 0);

		//Act
		set_relaychain_block_number(600);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_2_id
		));

		//Assert that reward it sent to treasury instead of the claimer since reward is less than ed
		assert_eq!(charlie_hdx_balance_0, 0);

		let expected_claimed_amount = 184_024_112_u128;
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &TreasuryAccount::get()),
			1000 * UNITS + expected_claimed_amount
		);

		expect_reward_claimed_events(vec![]);
	});
}

#[test]
fn exit_farms_should_work_for_multiple_farms() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);
		create_global_farm(None, None);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_2_id, ETH);
		create_yield_farm(global_farm_3_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			deposit_id
		));

		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_3_id,
			yield_farm_3_id,
			deposit_id
		));

		//Act
		let farm_entries = vec![yield_farm_1_id, yield_farm_2_id, yield_farm_3_id];
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::exit_farms(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			farm_entries.try_into().unwrap()
		));

		assert!(hydradx_runtime::OmnipoolWarehouseLM::deposit(deposit_id).is_none());
	});
}

fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		hydradx_runtime::Omnipool::protocol_account(),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));
	let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		token_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let token_price = FixedU128::from_inner(71_145_071_145_071);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		ETH,
		token_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let btc_price = FixedU128::from_inner(9_647_109_647_109_650_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		BTC,
		btc_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let lrna_price = FixedU128::from_inner(71_145_071_145_071);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		LRNA,
		lrna_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));
}

fn create_global_farm(rewards_currency: Option<AssetId>, yield_percentage: Option<Perquintill>) {
	let total_rewards = 1_000_000 * UNITS;

	assert_ok!(hydradx_runtime::Balances::force_set_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		Treasury::account_id(),
		total_rewards,
	));

	assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::create_global_farm(
		hydradx_runtime::RuntimeOrigin::root(),
		total_rewards,
		1_000_000,
		10,
		rewards_currency.unwrap_or(HDX),
		Treasury::account_id(),
		yield_percentage.unwrap_or(Perquintill::from_parts(570_776_255_707)),
		1_000,
		FixedU128::one()
	));
}

fn create_yield_farm(id: GlobalFarmId, asset: AssetId) {
	assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::create_yield_farm(
		RuntimeOrigin::signed(Treasury::account_id()),
		id,
		asset,
		FixedU128::one(),
		Some(LoyaltyCurve::default())
	));
}

fn omnipool_add_liquidity(lp: AccountId, asset: AssetId, amount: Balance) -> primitives::ItemId {
	let current_position_id = Omnipool::next_position_id();

	assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(lp), asset, amount));

	current_position_id
}

//This function add initial amount in native currency to pot to prevent dusting.
fn seed_lm_pot() {
	//prevent pot account from dusting
	let pot = warehouse_liquidity_mining::Pallet::<hydradx_runtime::Runtime, Instance1>::pot_account_id().unwrap();
	assert_ok!(hydradx_runtime::Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		pot,
		HDX,
		100 * UNITS as i128,
	));
}

fn do_lrna_hdx_trade() {
	assert_ok!(hydradx_runtime::Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		HDX,
		100 * UNITS as i128,
	));

	assert_ok!(hydradx_runtime::Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		LRNA,
		100 * UNITS as i128,
	));

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		LRNA,
		HDX,
		UNITS,
		0,
	));
}

#[test]
fn position_should_be_valued_correctly_when_oracle_is_used() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_id = 1;
		let yield_farm_id = 2;

		//Arrange
		init_omnipool();
		seed_lm_pot();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		hydradx_run_to_block(400);
		set_relaychain_block_number(400);

		//NOTE: we don't have any trades in mocked env so position should be valued same using
		//oracle and omnipool's spot price.
		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		let omnipool_position = hydradx_runtime::Omnipool::load_position(position_id, CHARLIE.into()).unwrap();
		let omnipool_asset_state = hydradx_runtime::Omnipool::load_asset_state(omnipool_position.asset_id).unwrap();

		let expected_position_value = omnipool_asset_state
			.price()
			.unwrap()
			.checked_mul_int(omnipool_position.amount)
			.unwrap();

		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_id,
			yield_farm_id,
			position_id
		));

		let deposit = warehouse_liquidity_mining::Deposit::<
			hydradx_runtime::Runtime,
			warehouse_liquidity_mining::Instance1,
		>::get(deposit_id)
		.unwrap();

		use warehouse_liquidity_mining::DepositData;
		let mut expected_deposit: DepositData<hydradx_runtime::Runtime, warehouse_liquidity_mining::Instance1> =
			DepositData::new(1_000_000_000_000_000_u128, ETH);

		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_id,
				yield_farm_id,
				expected_position_value,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(expected_deposit, deposit);
	});
}

#[test]
fn price_adjustment_from_oracle_should_be_saved_in_global_farm_when_oracle_is_available() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let yield_farm_1_id = 2;

		//Arrange
		init_omnipool();
		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		//Act
		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_1_id
		));

		//Assert
		let global_farm = hydradx_runtime::OmnipoolWarehouseLM::global_farm(global_farm_1_id).unwrap();
		let price_adjustment = DefaultPriceAdjustment::get(&global_farm).unwrap();
		assert_eq!(
			price_adjustment,
			FixedU128::from_inner(830_817_151_946_084_689_817_u128)
		);
	});
}

#[test]
fn liquidity_mining_should_work_when_farm_distribute_bonds() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let yield_farm_1_id = 2;

		//Arrange
		init_omnipool();
		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();

		//Create bodns
		assert_ok!(hydradx_runtime::Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			Treasury::account_id(),
			2_000_000 * UNITS,
		));

		let maturity = NOW + MONTH;
		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(
			RuntimeOrigin::signed(Treasury::account_id()),
			HDX,
			2_000_000 * UNITS,
			maturity
		));
		assert_eq!(AssetRegistry::assets(bond_id).unwrap().asset_type, AssetType::Bond);
		//NOTE: make bond sufficient because treasury account is whitelisted. In this case farm
		//would have to pay ED for receiving insufficicient bods and farm's account has no balance.
		assert_ok!(AssetRegistry::update(
			hydradx_runtime::RuntimeOrigin::root(),
			bond_id,
			None,
			None,
			None,
			None,
			Some(true),
			None,
			None,
			None,
		));

		// farm's rewards in test are less than ED.
		assert_ok!(hydradx_runtime::Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(Treasury::account_id()),
			CHARLIE.into(),
			bond_id,
			2 * UNITS,
		));

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(Some(bond_id), None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(600);

		//Assert
		set_relaychain_block_number(700);
		let charlie_bonds_balance_0 = hydradx_runtime::Currencies::free_balance(bond_id, &CHARLIE.into());
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_1_id
		));

		let expected_claimed_amount = 622850845;
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(bond_id, &CHARLIE.into()),
			charlie_bonds_balance_0 + expected_claimed_amount
		);

		// NOTE: make sure oracle's price adjustment was used.
		let global_farm = hydradx_runtime::OmnipoolWarehouseLM::global_farm(global_farm_1_id).unwrap();
		let price_adjustment = DefaultPriceAdjustment::get(&global_farm).unwrap();
		assert_eq!(
			price_adjustment,
			FixedU128::from_inner(830_817_151_946_084_689_817_u128)
		);
	});
}

#[test]
fn claim_rewards_should_work_when_farm_is_updated() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let yield_farm_1_id = 2;
		let yield_farm_2_id = 3;

		//Arrange
		init_omnipool();
		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, ETH);
		create_yield_farm(global_farm_1_id, DOT);

		set_relaychain_block_number(300);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			ETH,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), ETH, 1_000 * UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			position_id
		));

		set_relaychain_block_number(500);
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			CHARLIE.into(),
			DOT,
			10_000 * UNITS as i128,
		));

		let position_id = omnipool_add_liquidity(CHARLIE.into(), DOT, UNITS);
		assert_nft_owner!(
			hydradx_runtime::OmnipoolCollectionId::get(),
			position_id,
			CHARLIE.into()
		);

		set_relaychain_block_number(550);
		let deposit_id_2 = 2;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(CHARLIE.into()),
			global_farm_1_id,
			yield_farm_2_id,
			position_id
		));

		//Act - update farm
		let planned_yielding_periods: BlockNumber = 2_000_000_u32;
		let yield_per_period = Perquintill::from_parts(550_776_255_707);
		let min_deposit = 5_000;
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::update_global_farm(
			RuntimeOrigin::root(),
			global_farm_1_id,
			planned_yielding_periods,
			yield_per_period,
			min_deposit
		));
		//Assert
		let g_farm =
			warehouse_liquidity_mining::GlobalFarm::<hydradx_runtime::Runtime, Instance1>::get(global_farm_1_id)
				.unwrap();
		assert_eq!(g_farm.planned_yielding_periods, planned_yielding_periods);
		assert_eq!(g_farm.yield_per_period, yield_per_period);
		assert_eq!(g_farm.min_deposit, min_deposit);

		let charlie_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		//Act 1 - claim rewards for 2-nd yield-farm-entry
		set_relaychain_block_number(600);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id_2,
			yield_farm_2_id
		));

		//Assert
		//NOTE: can't assert state in the deposit because fields are private
		let charlie_new_hdx_balance_after_first_claim = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		assert!(
			charlie_new_hdx_balance_after_first_claim > charlie_hdx_balance_0,
			"Charlie's balance should be increased"
		);
		assert_eq!(charlie_new_hdx_balance_after_first_claim, 1000030740535405);

		//Act 2 - claim rewards for differnt yield-farm-entry in the same period should work.
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_1_id
		));

		//Assert
		//NOTE: can't assert state in the deposit because fields are private
		let charlie_new_hdx_balance_after_2nd_claim = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		assert!(
			charlie_new_hdx_balance_after_2nd_claim > charlie_new_hdx_balance_after_first_claim,
			"Charlie's balance should be increased"
		);
		assert_eq!(charlie_new_hdx_balance_after_2nd_claim, 1000031130694537);
	});
}

pub fn expect_reward_claimed_events(e: Vec<RuntimeEvent>) {
	let last_events = test_utils::last_events::<hydradx_runtime::RuntimeEvent, hydradx_runtime::Runtime>(10);

	let mut reward_claimed_events = vec![];

	for event in &last_events {
		let e = event.clone();
		if matches!(
			e,
			RuntimeEvent::OmnipoolLiquidityMining(
				pallet_omnipool_liquidity_mining::Event::<hydradx_runtime::Runtime>::RewardClaimed { .. }
			)
		) {
			reward_claimed_events.push(e);
		}
	}

	pretty_assertions::assert_eq!(reward_claimed_events, e);
}

pub fn expect_lm_events(e: Vec<RuntimeEvent>) {
	let last_events = test_utils::last_events::<hydradx_runtime::RuntimeEvent, hydradx_runtime::Runtime>(10);

	let mut reward_claimed_events = vec![];

	for event in &last_events {
		let e = event.clone();
		if matches!(
			e,
			RuntimeEvent::OmnipoolLiquidityMining(
				pallet_omnipool_liquidity_mining::Event::<hydradx_runtime::Runtime>::RewardClaimed { .. }
			) | RuntimeEvent::OmnipoolLiquidityMining(pallet_omnipool_liquidity_mining::Event::<
				hydradx_runtime::Runtime,
			>::SharesDeposited { .. })
				| RuntimeEvent::OmnipoolLiquidityMining(pallet_omnipool_liquidity_mining::Event::<
					hydradx_runtime::Runtime,
				>::SharesRedeposited { .. })
		) {
			reward_claimed_events.push(e);
		}
	}

	pretty_assertions::assert_eq!(reward_claimed_events, e);
}

pub fn expect_stableswap_liquidity_added_events(e: Vec<RuntimeEvent>) {
	let last_events = test_utils::last_events::<hydradx_runtime::RuntimeEvent, hydradx_runtime::Runtime>(40);

	let mut events = vec![];

	for event in &last_events {
		let e = event.clone();
		if matches!(
			e,
			RuntimeEvent::Stableswap(pallet_stableswap::Event::<hydradx_runtime::Runtime>::LiquidityAdded { .. })
		) {
			events.push(e);
		}
	}

	pretty_assertions::assert_eq!(events, e);
}

pub fn expect_omnipool_liquidity_added_events(e: Vec<RuntimeEvent>) {
	let last_events = test_utils::last_events::<hydradx_runtime::RuntimeEvent, hydradx_runtime::Runtime>(10);

	let mut events = vec![];

	for event in &last_events {
		let e = event.clone();
		if matches!(
			e,
			RuntimeEvent::Omnipool(pallet_omnipool::Event::<hydradx_runtime::Runtime>::LiquidityAdded { .. })
		) {
			events.push(e);
		}
	}

	pretty_assertions::assert_eq!(events, e);
}

//TODO: duplicated, remove duplication
pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut asset_ids: Vec<<Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let asset_id = AssetRegistry::register_sufficient_asset(
			None,
			Some(name.try_into().unwrap()),
			AssetKind::Token,
			1u128,
			Some(b"xDUM".to_vec().try_into().unwrap()),
			Some(18u8),
			None,
			None,
		)?;

		asset_ids.push(asset_id);
		Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			initial_liquidity as i128,
		)?;
		initial.push(AssetAmount::new(asset_id, initial_liquidity));
	}
	let pool_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	Stableswap::create_pool(RuntimeOrigin::root(), pool_id, asset_ids, amplification, fee)?;

	Stableswap::add_liquidity(RuntimeOrigin::signed(BOB.into()), pool_id, initial.try_into().unwrap())?;

	Ok((pool_id, asset_in, asset_out))
}

#[test]
pub fn create_yield_farm2() {
	TestNet::reset();

	Hydra::execute_with(|| {
		fund_treasury().unwrap(); //To prevent BelowMinimum error

		let owner = ALICE;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(0, &owner.into(), INITIAL_BALANCE as i128).unwrap();
		assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
			HDX,
			&owner.into(),
			G_FARM_TOTAL_REWARDS.try_into().unwrap(),
		));
		let global_farm_id = 1;

		initialize_omnipool().unwrap();

		initialize_global_farm(owner.clone().into()).unwrap();
		initialize_yield_farm(owner.clone().into(), global_farm_id, BTC.into()).unwrap();

		let lp = BOB;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(0, &lp.into(), INITIAL_BALANCE as i128).unwrap();
		assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
			BTC,
			&lp.into(),
			(10 * BTC_ONE).try_into().unwrap(),
		));

		let position_id = omnipool_add_liquidity(lp.clone().into(), BTC.into(), 10 * BTC_ONE);

		set_period(100);
		lm_deposit_shares(lp.into(), global_farm_id, 2, position_id).unwrap();

		set_period(200);

		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::create_yield_farm(
			RuntimeOrigin::signed(owner.clone().into()),
			global_farm_id,
			ETH.into(),
			FixedU128::one(),
			Some(LoyaltyCurve::default())
		));
	});
}
pub const INITIAL_BALANCE: Balance = 10_000_000 * UNITS;
const BTC_ONE: Balance = 100_000_000;

fn fund_treasury() -> DispatchResult {
	let account = Treasury::account_id();

	<Currencies as MultiCurrencyExtended<_>>::update_balance(0, &account, INITIAL_BALANCE as i128).unwrap();
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		HDX,
		&account,
		(10000 * UNITS).try_into().unwrap(),
	));

	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		ETH,
		&account,
		(10000 * UNITS).try_into().unwrap(),
	));

	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		HDX,
		&account,
		(10000 * UNITS).try_into().unwrap(),
	));

	Ok(())
}
const G_FARM_TOTAL_REWARDS: Balance = 10_000_000 * UNITS;

fn initialize_global_farm(owner: AccountId) -> DispatchResult {
	OmnipoolLiquidityMining::create_global_farm(
		RawOrigin::Root.into(),
		G_FARM_TOTAL_REWARDS,
		BlockNumberFor::<hydradx_runtime::Runtime>::from(100_000_u32),
		BlockNumberFor::<hydradx_runtime::Runtime>::from(1_u32),
		HDX.into(),
		owner,
		Perquintill::from_percent(20),
		1_000,
		FixedU128::one(),
	)?;

	seed_lm_pot();

	Ok(())
}

use orml_traits::MultiCurrencyExtended;
fn initialize_yield_farm(owner: AccountId, id: GlobalFarmId, asset: AssetId) -> DispatchResult {
	OmnipoolLiquidityMining::create_yield_farm(RawOrigin::Signed(owner).into(), id, asset, FixedU128::one(), None)
}

fn initialize_omnipool() -> DispatchResult {
	let stable_amount: Balance = 1_000_000_000_000_000u128;
	let native_amount: Balance = 1_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);

	let acc = Omnipool::protocol_account();

	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), DAI.into(), stable_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), HDX.into(), native_amount as Amount)?;

	fund(ALICE.clone().into(), HDX.into(), 10_000 * UNITS)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), DAI.into(), stable_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), HDX.into(), native_amount as Amount)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		HDX.into(),
		native_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;
	Omnipool::add_token(
		RawOrigin::Root.into(),
		DAI.into(),
		stable_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;

	let name = b"BSX".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	// Register new asset in asset registry
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			Balance::one(),
			None,
			None,
			None,
			None,
		))
	})?;

	// Create account for token provider and set balance
	let owner: AccountId = ALICE.into();

	let token_price = FixedU128::from((1, 5));
	let token_amount = 200_000_000_000_000u128;

	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), ETH.into(), token_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), BTC.into(), token_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), DOT.into(), token_amount as Amount)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		ETH.into(),
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		BTC.into(),
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		DOT.into(),
		token_price,
		Permill::from_percent(100),
		owner,
	)?;

	//NOTE: This is necessary for oracle to provide price.
	set_period(10);

	do_lrna_hdx_trade();

	Ok(())
}

fn fund(to: AccountId, currency: AssetId, amount: Balance) -> DispatchResult {
	Currencies::deposit(currency, &to, amount)
}

fn lm_deposit_shares(who: AccountId, g_id: GlobalFarmId, y_id: YieldFarmId, position_id: ItemId) -> DispatchResult {
	OmnipoolLiquidityMining::deposit_shares(RawOrigin::Signed(who).into(), g_id, y_id, position_id)
}

use frame_support::traits::OnFinalize;
use frame_support::traits::OnInitialize;
use frame_system::pallet_prelude::BlockNumberFor;
use hydradx_runtime::OmnipoolLiquidityMining;
use hydradx_traits::liquidity_mining::YieldFarmId;
use primitives::Amount;
use primitives::ItemId;

fn set_period(to: u32) {
	//NOTE: predefined global farm has period size = 1 block.
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		<frame_system::Pallet<Runtime> as OnFinalize<BlockNumberFor<hydradx_runtime::Runtime>>>::on_finalize(b);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<
			BlockNumberFor<hydradx_runtime::Runtime>,
		>>::on_finalize(b);

		<frame_system::Pallet<Runtime> as OnInitialize<BlockNumberFor<hydradx_runtime::Runtime>>>::on_initialize(
			b + 1_u32,
		);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<
			BlockNumberFor<hydradx_runtime::Runtime>,
		>>::on_initialize(b + 1_u32);

		hydradx_runtime::System::set_block_number(b + 1_u32);
	}
}
