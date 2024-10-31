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

use crate::{assert_nft_owner, omnipool_init::hydra_run_to_block, polkadot_test_net::*};

use frame_support::assert_ok;
use hydradx_traits::{liquidity_mining::PriceAdjustment, AMM};
use pallet_asset_registry::AssetType;
use warehouse_liquidity_mining::{
	DefaultPriceAdjustment, DepositData, GlobalFarmData, GlobalFarmId, Instance2, LoyaltyCurve, YieldFarmData,
	YieldFarmEntry,
};

use orml_traits::MultiCurrency;
use sp_runtime::{
	traits::{One, Zero},
	FixedU128, Perquintill,
};
use xcm_emulator::TestExt;

use hydradx_runtime::{
	AssetRegistry, Balance, Bonds, Runtime, RuntimeOrigin, RuntimeOrigin as hydra_origin, XYKLiquidityMining,
	XYKWarehouseLM, XYK,
};
use pallet_xyk::types::AssetPair;
use polkadot_xcm::v3::{
	Junction::{GeneralIndex, Parachain},
	Junctions::X2,
	MultiLocation,
};
use pretty_assertions::assert_eq;
use primitives::constants::time::unix_time::MONTH;

#[test]
fn create_global_farm_should_work_when_origin_is_root() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let total_rewards: Balance = 1_000_000 * UNITS;
		let planned_yielding_periods: BlockNumber = 1_000_000;
		let blocks_per_period: BlockNumber = 10;
		let reward_currency = HDX;
		let incentivized_asset = PEPE;
		let owner = Treasury::account_id();
		let yield_per_period = Perquintill::from_parts(570_776_255_707);
		let min_deposit = 1_000;

		assert_ok!(hydradx_runtime::Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			owner.clone(),
			total_rewards,
		));

		set_relaychain_block_number(100);

		assert_ok!(XYKLiquidityMining::create_global_farm(
			hydradx_runtime::RuntimeOrigin::root(),
			total_rewards,
			planned_yielding_periods,
			blocks_per_period,
			incentivized_asset,
			reward_currency,
			owner.clone(),
			yield_per_period,
			min_deposit,
			FixedU128::from(2)
		));

		let farm_id = 1;
		let updated_at = 100 / blocks_per_period;
		assert_eq!(
			XYKWarehouseLM::global_farm(1).unwrap(),
			GlobalFarmData::new(
				farm_id,
				updated_at,
				reward_currency,
				yield_per_period,
				planned_yielding_periods,
				blocks_per_period,
				owner,
				incentivized_asset,
				total_rewards / planned_yielding_periods as u128,
				min_deposit,
				FixedU128::from(2),
			)
		);

		let g_farm_account = XYKWarehouseLM::farm_account_id(farm_id).unwrap();
		assert_eq!(hydradx_runtime::Balances::free_balance(g_farm_account), total_rewards);
	});
}

#[test]
fn create_yield_farm_should_work_when_xyk_exists() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_id = 1;
		let created_yield_farm_id = 2;
		let loyalty_curve = Some(LoyaltyCurve::default());
		let multiplier = FixedU128::one();
		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(asset_pair);

		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);

		create_xyk_pool(
			asset_pair.asset_in,
			1_000_000 * UNITS,
			asset_pair.asset_out,
			10_000_000 * UNITS,
		);

		set_relaychain_block_number(200);
		assert_ok!(XYKLiquidityMining::create_yield_farm(
			RuntimeOrigin::signed(Treasury::account_id()),
			global_farm_id,
			asset_pair,
			multiplier,
			loyalty_curve.clone()
		));

		let updated_at = 20;
		let y_farm = warehouse_liquidity_mining::YieldFarm::<Runtime, Instance2>::get((
			amm_pool_id,
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
		let deposit_amount = 5_000_000 * UNITS;
		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(asset_pair);

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);
		create_yield_farm(global_farm_id, asset_pair, None);
		set_relaychain_block_number(300);

		//Act
		set_relaychain_block_number(400);
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_id,
			yield_farm_id,
			asset_pair,
			deposit_amount
		));

		//Assert
		let deposit = XYKWarehouseLM::deposit(1).unwrap();
		let mut expected_deposit = DepositData::new(deposit_amount, amm_pool_id);
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_id,
				yield_farm_id,
				500_000 * UNITS,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);

		//assert LM deposit
		assert_nft_owner!(hydradx_runtime::XYKLmCollectionId::get(), 1, DAVE.into());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &DAVE.into()),
			dave_shares_balance - deposit_amount
		);
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			deposit_amount
		);
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

		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(asset_pair);

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);
		create_global_farm(None, ACA, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		//Act
		set_relaychain_block_number(500);
		assert_ok!(XYKLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			asset_pair,
			deposit_id,
		));

		let deposit = XYKWarehouseLM::deposit(deposit_id).unwrap();
		let mut expected_deposit = DepositData::new(dave_shares_balance, amm_pool_id);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				10_000_000 * UNITS,
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
				100_000_000 * UNITS,
				FixedU128::zero(),
				50,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);

		//assert LM deposit
		assert_nft_owner!(hydradx_runtime::XYKLmCollectionId::get(), 1, DAVE.into());
		assert_eq!(Currencies::free_balance(xyk_share_id, &DAVE.into()), Balance::zero());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			dave_shares_balance
		);
	});
}

#[test]
fn join_farms_should_work_with_multiple_farm_entries() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(asset_pair);

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);
		create_global_farm(None, ACA, None);
		create_global_farm(None, PEPE, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);
		create_yield_farm(global_farm_3_id, asset_pair, None);

		set_relaychain_block_number(400);
		let farms = vec![
			(global_farm_1_id, yield_farm_1_id),
			(global_farm_2_id, yield_farm_2_id),
			(global_farm_3_id, yield_farm_3_id),
		];
		let deposit_id = 1;

		//Act
		assert_ok!(XYKLiquidityMining::join_farms(
			RuntimeOrigin::signed(DAVE.into()),
			farms.try_into().unwrap(),
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(500);

		let deposit = XYKWarehouseLM::deposit(deposit_id).unwrap();
		let mut expected_deposit = DepositData::new(dave_shares_balance, amm_pool_id);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				10_000_000 * UNITS,
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
				100_000_000 * UNITS,
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
				10_000_000 * UNITS,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);

		//assert LM deposit
		assert_nft_owner!(hydradx_runtime::XYKLmCollectionId::get(), 1, DAVE.into());
		assert_eq!(Currencies::free_balance(xyk_share_id, &DAVE.into()), Balance::zero());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			dave_shares_balance
		);
	});
}

#[test]
fn add_liquidity_and_join_farms_should_work_with_multiple_farm_entries() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let global_farm_3_id = 3;
		let yield_farm_1_id = 4;
		let yield_farm_2_id = 5;
		let yield_farm_3_id = 6;

		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(asset_pair);

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);
		create_global_farm(None, ACA, None);
		create_global_farm(None, PEPE, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);
		create_yield_farm(global_farm_3_id, asset_pair, None);

		set_relaychain_block_number(400);
		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			BOB.into(),
			PEPE,
			10_000_0000 * UNITS as i128,
		));
		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			BOB.into(),
			ACA,
			10_000_0000 * UNITS as i128,
		));

		let farms = vec![
			(global_farm_1_id, yield_farm_1_id),
			(global_farm_2_id, yield_farm_2_id),
			(global_farm_3_id, yield_farm_3_id),
		];
		let deposit_id = 1;
		let liquidity_amount = 5_000_000 * UNITS;
		//Act
		assert_ok!(XYKLiquidityMining::add_liquidity_and_join_farms(
			RuntimeOrigin::signed(BOB.into()),
			PEPE,
			ACA,
			liquidity_amount,
			6_000_0000 * UNITS,
			farms.try_into().unwrap(),
		));

		set_relaychain_block_number(500);

		let deposit = XYKWarehouseLM::deposit(deposit_id).unwrap();

		let shares_amount = Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id());
		let mut expected_deposit = DepositData::new(shares_amount, amm_pool_id);
		//1-th deposit entry
		expected_deposit
			.add_yield_farm_entry(YieldFarmEntry::new(
				global_farm_1_id,
				yield_farm_1_id,
				liquidity_amount,
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
				shares_amount,
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
				liquidity_amount,
				FixedU128::zero(),
				40,
				0,
			))
			.unwrap();

		assert_eq!(deposit, expected_deposit);

		//assert LM deposit
		assert_nft_owner!(hydradx_runtime::XYKLmCollectionId::get(), 1, BOB.into());
		assert_eq!(Currencies::free_balance(xyk_share_id, &BOB.into()), Balance::zero());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			50000000000000000000
		);
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
		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);
		create_global_farm(None, ACA, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(500);
		assert_ok!(XYKLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			asset_pair,
			deposit_id,
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		set_relaychain_block_number(600);
		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_2_id,
			asset_pair,
		));

		//Assert
		//NOTE: withdraw is claiming rewards automatically
		let dave_hdx_0 = hydradx_runtime::Currencies::free_balance(HDX, &DAVE.into());
		assert_eq!(dave_hdx_0, 1_004_254_545_454_545_u128);

		//NOTE:	shares should not be unlocked because deposit wasn't destroyed(it has 1
		//yield-farm-entry left)
		//assert LM deposit
		assert_eq!(Currencies::free_balance(xyk_share_id, &DAVE.into()), Balance::zero());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			dave_shares_balance
		);

		//Check if yield-farm-entry was removed from the deposit.
		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_none()
		);

		set_relaychain_block_number(700);

		//Act 2
		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_1_id,
			asset_pair,
		));

		//Assert
		//withdraw_shares claims rewards under the hood
		let dave_hdx_1 = hydradx_runtime::Currencies::free_balance(HDX, &DAVE.into());

		assert!(dave_hdx_1 > dave_hdx_0);

		//NOTE: last shares were unlockend and deposit's nft should be destroyed and omnipool's
		//position should be unlocked.
		assert!(warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id).is_none());
		//LM nft should be destroyed
		assert!(hydradx_runtime::Uniques::owner(hydradx_runtime::XYKLmCollectionId::get(), deposit_id).is_none());
		//omnpool's position should be unlocekd
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &DAVE.into()),
			dave_shares_balance
		);
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			Balance::zero()
		);
	});
}

#[test]
fn liquidity_mining_should_work_when_distributes_insufficient_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let ext1 = register_external_asset(0_u128);
		let ext2 = register_external_asset(1_u128);

		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;
		let asset_pair = AssetPair {
			asset_in: ext1,
			asset_out: ext2,
		};

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		set_relaychain_block_number(100);
		let farm_owner = BOB;
		create_global_farm(Some(ext1), ext1, Some(farm_owner.into()));
		create_global_farm(Some(ext1), ext2, Some(farm_owner.into()));

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, Some(farm_owner.into()));
		create_yield_farm(global_farm_2_id, asset_pair, Some(farm_owner.into()));

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(500);
		assert_ok!(XYKLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			asset_pair,
			deposit_id,
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		set_relaychain_block_number(600);
		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_2_id,
			asset_pair,
		));

		//Assert
		//NOTE: withdraw is claiming rewards automatically
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &DAVE.into()),
			995_300_000_000_000_u128
		);

		//NOTE:	shares should not be unlocked because deposit wasn't destroyed(it has 1
		//yield-farm-entry left)
		//assert LM deposit
		assert_eq!(Currencies::free_balance(xyk_share_id, &DAVE.into()), Balance::zero());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			dave_shares_balance
		);

		//Check if yield-farm-entry was removed from the deposit.
		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_none()
		);

		set_relaychain_block_number(700);

		//Act
		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_1_id,
			asset_pair,
		));

		//Assert
		//NOTE: claim happened before withdraw in this period so no rewards should be claimed.
		//This is lower than before because dave received insufficient asset and he had to paid ED.
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &DAVE.into()),
			994_200_000_000_000_u128
		);

		//NOTE: last shares were unlockend and deposit's nft should be destroyed and omnipool's
		//position should be unlocked.
		assert!(warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id).is_none());
		//LM nft should be destroyed
		assert!(hydradx_runtime::Uniques::owner(hydradx_runtime::XYKLmCollectionId::get(), deposit_id).is_none());
		//omnpool's position should be unlocekd
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &DAVE.into()),
			dave_shares_balance
		);
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			Balance::zero()
		);
	});
}

#[test]
fn liquidity_mining_should_work_when_xyk_assets_are_insufficient() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let ext1 = register_external_asset(0_u128);
		let ext2 = register_external_asset(1_u128);

		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;
		let asset_pair = AssetPair {
			asset_in: ext1,
			asset_out: ext2,
		};

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		set_relaychain_block_number(100);
		create_global_farm(None, ext1, None);
		create_global_farm(None, ext2, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(500);
		assert_ok!(XYKLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			asset_pair,
			deposit_id,
		));

		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_some()
		);

		set_relaychain_block_number(600);
		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_2_id,
			asset_pair,
		));

		//Assert
		//NOTE: withdraw is claiming rewards automatically
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &DAVE.into()),
			1_001_854_545_454_545_u128
		);

		//NOTE:	shares should not be unlocked because deposit wasn't destroyed(it has 1
		//yield-farm-entry left)
		//assert LM deposit
		assert_eq!(Currencies::free_balance(xyk_share_id, &DAVE.into()), Balance::zero());
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			dave_shares_balance
		);

		//Check if yield-farm-entry was removed from the deposit.
		assert!(
			warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id)
				.unwrap()
				.get_yield_farm_entry(yield_farm_2_id)
				.is_none()
		);

		set_relaychain_block_number(700);

		//Act
		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_1_id,
			asset_pair,
		));

		//Assert
		//NOTE: claim happened before withdraw in this period so no rewards should be claimed.
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &DAVE.into()),
			1_019_216_083_916_083_u128
		);

		//NOTE: last shares were unlockend and deposit's nft should be destroyed and omnipool's
		//position should be unlocked.
		assert!(warehouse_liquidity_mining::Deposit::<hydradx_runtime::Runtime, Instance2>::get(deposit_id).is_none());
		//LM nft should be destroyed
		assert!(hydradx_runtime::Uniques::owner(hydradx_runtime::XYKLmCollectionId::get(), deposit_id).is_none());
		//omnpool's position should be unlocekd
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &DAVE.into()),
			dave_shares_balance
		);
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			Balance::zero()
		);
	});
}

#[test]
fn price_adjustment_from_oracle_should_be_saved_in_global_farm_when_oracle_is_available() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		set_relaychain_block_number(100);
		create_global_farm(Some(ACA), PEPE, None);
		create_global_farm(Some(PEPE), ACA, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			PEPE,
			1000 * UNITS as i128,
		));

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			ACA,
			1000 * UNITS as i128,
		));

		assert_ok!(hydradx_runtime::XYK::buy(
			RuntimeOrigin::signed(ALICE.into()),
			PEPE,
			ACA,
			2 * UNITS,
			200 * UNITS,
			false,
		));

		hydra_run_to_block(500);
		set_relaychain_block_number(500);

		//Act
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(600);

		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_1_id,
			asset_pair
		));

		//Assert
		let global_farm = XYKWarehouseLM::global_farm(global_farm_1_id).unwrap();
		let price_adjustment = DefaultPriceAdjustment::get(&global_farm).unwrap();
		assert_eq!(price_adjustment, FixedU128::from_inner(10_000_004_006_001_202_400_u128));
	});
}

#[test]
fn liquidity_mining_should_work_when_farm_distribute_bonds() {
	TestNet::reset();

	Hydra::execute_with(|| {
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

		let global_farm_1_id = 1;
		let yield_farm_1_id = 2;
		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);

		create_xyk_pool(HDX, 10_000_000 * UNITS, PEPE, 100_000_000 * UNITS);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());

		set_relaychain_block_number(100);
		create_global_farm(Some(bond_id), PEPE, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			PEPE,
			1000 * UNITS as i128,
		));

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			bond_id,
			1000 * UNITS as i128,
		));

		assert_ok!(hydradx_runtime::XYK::buy(
			RuntimeOrigin::signed(ALICE.into()),
			PEPE,
			HDX,
			2 * UNITS,
			200 * UNITS,
			false,
		));

		hydra_run_to_block(500);
		set_relaychain_block_number(500);

		//Act
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(600);

		let dave_bonds_balance = Currencies::free_balance(bond_id, &DAVE.into());

		assert_ok!(XYKLiquidityMining::withdraw_shares(
			RuntimeOrigin::signed(DAVE.into()),
			deposit_id,
			yield_farm_1_id,
			asset_pair
		));

		//Assert
		assert!(Currencies::free_balance(bond_id, &DAVE.into()) > dave_bonds_balance);
		let global_farm = XYKWarehouseLM::global_farm(global_farm_1_id).unwrap();
		let price_adjustment = DefaultPriceAdjustment::get(&global_farm).unwrap();
		assert_eq!(price_adjustment, FixedU128::from_inner(100_000_004_006_000_120_u128));
	});
}

#[test]
fn exit_farm_should_work_on_multiple_different_farms() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let global_farm_1_id = 1;
		let global_farm_2_id = 2;
		let yield_farm_1_id = 3;
		let yield_farm_2_id = 4;
		let yield_farm_3_id = 5;

		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};

		let asset_pair2 = AssetPair {
			asset_in: ACA,
			asset_out: HDX,
		};

		//Arrange
		let xyk_share_id = create_xyk_pool(
			asset_pair.asset_in,
			10_000_000 * UNITS,
			asset_pair.asset_out,
			100_000_000 * UNITS,
		);
		let xyk_share_id2 = create_xyk_pool(
			asset_pair2.asset_in,
			10_000_000 * UNITS,
			asset_pair2.asset_out,
			100_000_000 * UNITS,
		);
		let dave_shares_balance = Currencies::free_balance(xyk_share_id, &DAVE.into());
		let dave_shares_balance2 = Currencies::free_balance(xyk_share_id2, &DAVE.into());

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm(None, PEPE, None);
		create_global_farm(None, ACA, None);

		set_relaychain_block_number(200);
		create_yield_farm(global_farm_1_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair, None);
		create_yield_farm(global_farm_2_id, asset_pair2, None);

		set_relaychain_block_number(400);
		let deposit_id = 1;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_1_id,
			yield_farm_1_id,
			asset_pair,
			dave_shares_balance,
		));

		set_relaychain_block_number(500);
		assert_ok!(XYKLiquidityMining::redeposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_2_id,
			yield_farm_2_id,
			asset_pair,
			deposit_id,
		));

		let deposit_id2 = 2;
		assert_ok!(XYKLiquidityMining::deposit_shares(
			RuntimeOrigin::signed(DAVE.into()),
			global_farm_2_id,
			yield_farm_3_id,
			asset_pair2,
			dave_shares_balance2,
		));

		let exit_entries = vec![
			(deposit_id, yield_farm_1_id, asset_pair),
			(deposit_id, yield_farm_2_id, asset_pair),
			(deposit_id2, yield_farm_3_id, asset_pair2),
		];
		//Act
		assert_ok!(XYKLiquidityMining::exit_farms(
			RuntimeOrigin::signed(DAVE.into()),
			exit_entries.try_into().unwrap()
		));

		//Assert
		assert!(XYKWarehouseLM::deposit(deposit_id).is_none());
		assert!(XYKWarehouseLM::deposit(deposit_id2).is_none());

		assert_eq!(
			Currencies::free_balance(xyk_share_id, &DAVE.into()),
			dave_shares_balance
		);
		assert_eq!(
			Currencies::free_balance(xyk_share_id, &XYKLiquidityMining::account_id()),
			Balance::zero()
		);

		assert_eq!(
			Currencies::free_balance(xyk_share_id2, &DAVE.into()),
			dave_shares_balance2
		);
		assert_eq!(
			Currencies::free_balance(xyk_share_id2, &XYKLiquidityMining::account_id()),
			Balance::zero()
		);
	});
}

fn create_xyk_pool(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) -> AssetId {
	let share_id = AssetRegistry::next_asset_id().unwrap();

	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));

	share_id
}

fn create_global_farm(rewards_currency: Option<AssetId>, incentivized_asset: AssetId, owner: Option<AccountId>) {
	let total_rewards = 1_000_000 * UNITS;

	let owner = owner.unwrap_or(Treasury::account_id());
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		owner.clone(),
		rewards_currency.unwrap_or(HDX),
		total_rewards as i128 + 1_000, //for seeding pot's account
	));

	assert_ok!(XYKLiquidityMining::create_global_farm(
		hydradx_runtime::RuntimeOrigin::root(),
		total_rewards,
		1_000_000,
		10,
		incentivized_asset,
		rewards_currency.unwrap_or(HDX),
		owner,
		Perquintill::from_parts(570_776_255_707),
		1_000,
		FixedU128::one()
	));
}

fn create_yield_farm(id: GlobalFarmId, pair: AssetPair, owner: Option<AccountId>) {
	assert_ok!(XYKLiquidityMining::create_yield_farm(
		RuntimeOrigin::signed(owner.unwrap_or(Treasury::account_id())),
		id,
		pair,
		FixedU128::one(),
		Some(LoyaltyCurve::default())
	));
}

fn register_external_asset(general_index: u128) -> AssetId {
	let location = hydradx_runtime::AssetLocation(MultiLocation::new(
		1,
		X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(general_index)),
	));

	let next_asset_id = AssetRegistry::next_asset_id().unwrap();
	AssetRegistry::register_external(hydra_origin::signed(BOB.into()), location).unwrap();

	next_asset_id
}
