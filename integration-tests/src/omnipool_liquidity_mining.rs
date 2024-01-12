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

use frame_support::{assert_noop, assert_ok};
use hydradx_traits::liquidity_mining::PriceAdjustment;
use warehouse_liquidity_mining::{
	DefaultPriceAdjustment, DepositData, GlobalFarmData, GlobalFarmId, Instance1, LoyaltyCurve, YieldFarmData,
	YieldFarmEntry,
};

use orml_traits::MultiCurrency;
use primitives::AssetId;
use sp_runtime::{
	traits::{One, Zero},
	FixedPointNumber, FixedU128, Permill, Perquintill,
};
use xcm_emulator::TestExt;

use hydradx_runtime::{Balance, RuntimeOrigin};
use pretty_assertions::assert_eq;

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
		create_global_farm();

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
		create_global_farm();

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
		create_global_farm();
		create_global_farm();

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
fn claim_rewards_should_work_when_rewards_are_accumulated_for_deposit() {
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
		create_global_farm();
		create_global_farm();

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

		let bob_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		//Act 1 - claim rewards for 2-nd yield-farm-entry
		set_relaychain_block_number(600);
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_2_id
		));

		//Assert
		//NOTE: can't assert state in the deposit because fields are private
		let expected_claimed_amount = 184_024_112_u128;
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into()),
			bob_hdx_balance_0 + expected_claimed_amount
		);

		//Act & assert 2 - claim rewards in the same period for same yield-farm-entry should not work.
		assert_noop!(
			hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
				RuntimeOrigin::signed(CHARLIE.into()),
				deposit_id,
				yield_farm_2_id
			),
			warehouse_liquidity_mining::Error::<hydradx_runtime::Runtime, Instance1>::DoubleClaimInPeriod
		);

		let bob_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
		//Act 3 - claim rewards for differnt yield-farm-entry in the same period should work.
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
			RuntimeOrigin::signed(CHARLIE.into()),
			deposit_id,
			yield_farm_1_id
		));

		//Assert
		//NOTE: can't assert state in the deposit because fields are private
		let expected_claimed_amount = 393_607_131_u128;
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into()),
			bob_hdx_balance_0 + expected_claimed_amount
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

		//Arrange
		init_omnipool();

		seed_lm_pot();
		//necessary for oracle to have a price.
		do_lrna_hdx_trade();

		//NOTE: necessary to get oracle price.
		hydradx_run_to_block(100);
		set_relaychain_block_number(100);
		create_global_farm();
		create_global_farm();

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

		let bob_hdx_balance_0 = hydradx_runtime::Currencies::free_balance(HDX, &CHARLIE.into());
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
			bob_hdx_balance_0
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

fn create_global_farm() {
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
		HDX,
		Treasury::account_id(),
		Perquintill::from_parts(570_776_255_707),
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
	use hydradx_runtime::Omnipool;

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
		create_global_farm();

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
		create_global_farm();

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
		assert_ok!(hydradx_runtime::OmnipoolLiquidityMining::claim_rewards(
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
