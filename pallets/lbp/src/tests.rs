// This file is part of HydraDX-node.

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

#![allow(clippy::bool_assert_comparison)]
use super::*;
use crate::mock::{
	expect_events, generate_trades, run_to_sale_end, run_to_sale_start, AccountId, RuntimeCall as Call, DEFAULT_FEE,
	EXISTENTIAL_DEPOSIT, HDX_BSX_POOL_ID, INITIAL_BALANCE, INITIAL_ETH_BALANCE, KUSD_BSX_POOL_ID, SALE_END, SALE_START,
	SAMPLE_AMM_TRANSFER, SAMPLE_POOL_DATA,
};
pub use crate::mock::{
	set_block_number, Currency, ExtBuilder, LBPPallet, RuntimeEvent as TestEvent, RuntimeOrigin as Origin, Test, ALICE,
	BOB, BSX, CHARLIE, ETH, HDX, KUSD,
};
use frame_support::{assert_err, assert_noop, assert_ok};
use hydradx_traits::{AMMTransfer, LockedBalance};
use sp_runtime::traits::{BadOrigin, Dispatchable};
use sp_std::convert::TryInto;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| set_block_number(1));
	ext
}

pub fn predefined_test_ext() -> sp_io::TestExternalities {
	let mut ext = new_test_ext();
	ext.execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000,
			80_000_000,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			SALE_START,
			SALE_END,
			None,
			None,
			None,
			None,
			None,
		));

		let pool_data2 = Pool {
			owner: ALICE,
			start: SALE_START,
			end: SALE_END,
			assets: (KUSD, BSX),
			initial_weight: 20_000_000,
			final_weight: 80_000_000,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		};

		assert_eq!(<PoolData<Test>>::get(KUSD_BSX_POOL_ID).unwrap(), pool_data2);

		expect_events(vec![
			Event::LiquidityAdded {
				who: KUSD_BSX_POOL_ID,
				asset_a: KUSD,
				asset_b: BSX,
				amount_a: 1_000_000_000,
				amount_b: 2_000_000_000,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: pool_data2,
			}
			.into(),
		]);
	});
	ext
}

pub fn predefined_test_ext_with_repay_target() -> sp_io::TestExternalities {
	let mut ext = new_test_ext();
	ext.execute_with(|| {
		let initial_liquidity = 1_000_000_000;

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			80_000_000,
			20_000_000,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			initial_liquidity,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			SALE_START,
			Some(20),
			None,
			None,
			None,
			None,
			None,
		));

		assert_ok!(LBPPallet::add_liquidity(
			Origin::signed(ALICE),
			(KUSD, 10_000_000_000),
			(BSX, initial_liquidity),
		));
	});
	ext
}

pub fn start_50_50_lbp_without_fee_and_repay_target(
	asset_in: AssetId,
	reserve_in: Balance,
	asset_out: AssetId,
	reserve_out: Balance,
) -> AccountId {
	assert_ok!(LBPPallet::create_pool(
		Origin::root(),
		ALICE,
		asset_in,
		reserve_in,
		asset_out,
		reserve_out,
		50_000_000,
		50_000_000,
		WeightCurveType::Linear,
		(0, 1),
		CHARLIE,
		0,
	));

	let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

	assert_ok!(LBPPallet::update_pool_data(
		Origin::signed(ALICE),
		pool_id,
		None,
		SALE_START,
		SALE_END,
		None,
		None,
		None,
		None,
		None,
	));

	//start sale
	set_block_number(11);

	pool_id
}

#[test]
fn default_locked_balance_should_be_zero() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			<Test as pallet::Config>::LockedBalance::get_by_lock(COLLECTOR_LOCK_ID, BSX, BOB),
			0_u128
		);
	});
}

#[test]
fn validate_pool_data_should_work() {
	new_test_ext().execute_with(|| {
		let pool_data = Pool {
			owner: ALICE,
			start: SALE_START,
			end: SALE_END,
			assets: (KUSD, BSX),
			initial_weight: 20_000_000,
			final_weight: 90_000_000,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		};
		assert_ok!(LBPPallet::validate_pool_data(&pool_data));

		// null interval
		let pool_data = Pool {
			owner: ALICE,
			start: None,
			end: None,
			assets: (KUSD, BSX),
			initial_weight: 20_000_000,
			final_weight: 90_000_000,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		};
		assert_ok!(LBPPallet::validate_pool_data(&pool_data));

		let pool_data = Pool {
			owner: ALICE,
			start: SALE_START,
			end: Some(2u64),
			assets: (KUSD, BSX),
			initial_weight: 20_000_000,
			final_weight: 90_000_000,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		};
		assert_noop!(
			LBPPallet::validate_pool_data(&pool_data),
			Error::<Test>::InvalidBlockRange
		);

		let pool_data = Pool {
			owner: ALICE,
			start: SALE_START,
			end: Some(11u64 + u32::MAX as u64),
			assets: (KUSD, BSX),
			initial_weight: 20_000_000,
			final_weight: 90_000_000,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		};
		assert_noop!(
			LBPPallet::validate_pool_data(&pool_data),
			Error::<Test>::MaxSaleDurationExceeded
		);
	});
}

#[test]
fn max_sale_duration_ckeck() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::validate_pool_data(&Pool {
			owner: ALICE,
			start: SALE_START,
			end: Some(SALE_START.unwrap() + MAX_SALE_DURATION as u64 - 1),
			assets: (KUSD, BSX),
			initial_weight: 20_000_000,
			final_weight: 90_000_000,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		}));
		assert_noop!(
			LBPPallet::validate_pool_data(&Pool {
				owner: ALICE,
				start: SALE_START,
				end: Some(SALE_START.unwrap() + MAX_SALE_DURATION as u64),
				assets: (KUSD, BSX),
				initial_weight: 20_000_000,
				final_weight: 90_000_000,
				weight_curve: WeightCurveType::Linear,
				fee: DEFAULT_FEE,
				fee_collector: CHARLIE,
				repay_target: 0,
			}),
			Error::<Test>::MaxSaleDurationExceeded
		);
	});
}

#[test]
fn calculate_weights_should_work() {
	new_test_ext().execute_with(|| {
		let mut pool_data = Pool {
			owner: ALICE,
			start: Some(100),
			end: Some(200),
			assets: (KUSD, BSX),
			initial_weight: 50_000_000,
			final_weight: 33_333_333,
			weight_curve: WeightCurveType::Linear,
			fee: DEFAULT_FEE,
			fee_collector: CHARLIE,
			repay_target: 0,
		};
		assert_eq!(LBPPallet::calculate_weights(&pool_data, 170), Ok((38333333, 61666667)));

		pool_data.initial_weight = 33_333_333;
		pool_data.final_weight = 66_666_666;
		assert_eq!(LBPPallet::calculate_weights(&pool_data, 100), Ok((33333333, 66666667)));

		pool_data.initial_weight = 33_333_333;
		pool_data.final_weight = 33_333_333;
		assert_eq!(LBPPallet::calculate_weights(&pool_data, 100), Ok((33333333, 66666667)));

		pool_data.initial_weight = 50_000_000;
		pool_data.final_weight = 33_333_333;
		assert_eq!(LBPPallet::calculate_weights(&pool_data, 200), Ok((33333333, 66666667)));

		// invalid interval
		pool_data.start = Some(200);
		pool_data.end = Some(100);
		assert_eq!(
			LBPPallet::calculate_weights(&pool_data, 200),
			Err(Error::<Test>::WeightCalculationError.into())
		);

		// invalid interval
		pool_data.start = Some(100);
		pool_data.end = Some(200);
		assert_eq!(
			LBPPallet::calculate_weights(&pool_data, 201),
			Err(Error::<Test>::WeightCalculationError.into())
		);

		// out of bound
		pool_data.start = Some(100);
		pool_data.end = Some(200);
		assert_eq!(
			LBPPallet::calculate_weights(&pool_data, 10),
			Err(Error::<Test>::WeightCalculationError.into())
		);
		assert_eq!(
			LBPPallet::calculate_weights(&pool_data, 210),
			Err(Error::<Test>::WeightCalculationError.into())
		);
	});
}

#[test]
fn create_pool_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000u32,
			90_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		assert_eq!(Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID), 1_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &KUSD_BSX_POOL_ID), 2_000_000_000);
		assert_eq!(
			Currency::free_balance(KUSD, &ALICE),
			INITIAL_BALANCE.saturating_sub(1_000_000_000)
		);
		assert_eq!(
			Currency::free_balance(BSX, &ALICE),
			INITIAL_BALANCE.saturating_sub(2_000_000_000)
		);

		let pool_data = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(pool_data.owner, ALICE);
		assert_eq!(pool_data.start, None);
		assert_eq!(pool_data.end, None);
		assert_eq!(pool_data.assets, (KUSD, BSX));
		assert_eq!(pool_data.initial_weight, 20_000_000);
		assert_eq!(pool_data.final_weight, 90_000_000);
		assert_eq!(pool_data.weight_curve, WeightCurveType::Linear);
		assert_eq!(pool_data.fee, DEFAULT_FEE);
		assert_eq!(pool_data.fee_collector, CHARLIE);

		assert!(<FeeCollectorWithAsset<Test>>::contains_key(CHARLIE, KUSD));

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: 1_000_000_000,
			amount_b: 2_000_000_000,
		}
		.into()]);
	});
}

#[test]
fn create_pool_from_basic_origin_should_not_work() {
	new_test_ext().execute_with(|| {
		// only CreatePoolOrigin is allowed to create new pools
		assert_noop!(
			LBPPallet::create_pool(
				Origin::signed(ALICE),
				ALICE,
				HDX,
				1_000_000_000,
				BSX,
				2_000_000_000,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			BadOrigin
		);
	});
}

#[test]
fn create_same_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			80_000_000u32,
			10_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				KUSD,
				10_000_000_000,
				BSX,
				20_000_000_000,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::PoolAlreadyExists
		);

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: 1_000_000_000,
			amount_b: 2_000_000_000,
		}
		.into()]);
	});
}

#[test]
fn create_pool_with_same_assets_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				KUSD,
				1_000_000_000,
				KUSD,
				2_000_000_000,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::CannotCreatePoolWithSameAssets
		);
	});
}

#[test]
fn create_pool_with_non_existing_fee_collector_with_asset_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000u32,
			90_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000u32,
			90_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		),);
	});
}

#[test]
fn create_pool_with_existing_fee_collector_with_asset_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000u32,
			90_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				KUSD,
				1_000_000_000,
				HDX,
				2_000_000_000,
				20_000_000u32,
				90_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::FeeCollectorWithAssetAlreadyUsed
		);
	});
}

#[test]
fn create_pool_with_insufficient_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				HDX,
				0,
				BSX,
				0,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::InsufficientLiquidity
		);

		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				HDX,
				0,
				BSX,
				2_000_000_000,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::InsufficientLiquidity
		);

		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				HDX,
				100,
				BSX,
				100,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::InsufficientLiquidity
		);
	});
}

#[test]
fn create_pool_with_insufficient_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				KUSD,
				2_000_000_000_000_000,
				BSX,
				2_000_000_000_000_000,
				80_000_000u32,
				10_000_000u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn update_pool_data_should_work() {
	predefined_test_ext().execute_with(|| {
		// update all parameters
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			Some(15),
			Some(18),
			Some(10_000_000),
			Some(80_000_000),
			Some((5, 100)),
			Some(BOB),
			None,
		));

		// verify changes
		let updated_pool_data_1 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data_1.start, Some(15));
		assert_eq!(updated_pool_data_1.end, Some(18));
		assert_eq!(updated_pool_data_1.initial_weight, 10_000_000);
		assert_eq!(updated_pool_data_1.final_weight, 80_000_000);
		assert_eq!(updated_pool_data_1.fee, (5, 100),);
		assert_eq!(updated_pool_data_1.fee_collector, BOB);

		// removes old fee collector from store and
		// sets updated fee collector
		assert!(!<FeeCollectorWithAsset<Test>>::contains_key(CHARLIE, KUSD));
		assert!(<FeeCollectorWithAsset<Test>>::contains_key(BOB, KUSD));

		// update only one parameter
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			None,
			Some(30),
			None,
			None,
			None,
			None,
			None,
		));

		// verify changes
		let updated_pool_data_2 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data_2.start, Some(15));
		assert_eq!(updated_pool_data_2.end, Some(30));
		assert_eq!(updated_pool_data_2.initial_weight, 10_000_000);
		assert_eq!(updated_pool_data_2.final_weight, 80_000_000);
		assert_eq!(updated_pool_data_2.fee, (5, 100),);
		assert_eq!(updated_pool_data_2.fee_collector, BOB);

		// update only one parameter
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			None,
			None,
			Some(12_500_000),
			None,
			None,
			None,
			None,
		));

		// verify changes
		let updated_pool_data_3 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data_3.start, Some(15));
		assert_eq!(updated_pool_data_3.end, Some(30));
		assert_eq!(updated_pool_data_3.initial_weight, 12_500_000);
		assert_eq!(updated_pool_data_3.final_weight, 80_000_000);
		assert_eq!(updated_pool_data_3.fee, (5, 100),);
		assert_eq!(updated_pool_data_3.fee_collector, BOB);

		// update only one parameter
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			None,
			None,
			None,
			None,
			None,
			Some(ALICE),
			None,
		));

		// verify changes
		let updated_pool_data_4 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data_4.start, Some(15));
		assert_eq!(updated_pool_data_4.end, Some(30));
		assert_eq!(updated_pool_data_4.initial_weight, 12_500_000);
		assert_eq!(updated_pool_data_4.final_weight, 80_000_000);
		assert_eq!(updated_pool_data_4.fee, (5, 100),);
		assert_eq!(updated_pool_data_4.fee_collector, ALICE);

		// mix
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			None,
			Some(18),
			Some(10_000_000),
			Some(80_000_000),
			Some((6, 1_000)),
			None,
			None,
		));

		// verify changes
		let updated_pool_data_5 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data_5.start, Some(15));
		assert_eq!(updated_pool_data_5.end, Some(18));
		assert_eq!(updated_pool_data_5.initial_weight, 10_000_000);
		assert_eq!(updated_pool_data_5.final_weight, 80_000_000);
		assert_eq!(updated_pool_data_5.fee, (6, 1_000),);
		assert_eq!(updated_pool_data_5.fee_collector, ALICE);

		// set repay target
		let repayment = 1_000_000;
		assert_eq!(updated_pool_data_5.repay_target, 0);
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			None,
			None,
			None,
			None,
			None,
			None,
			Some(repayment),
		));
		let updated_pool_data_6 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data_6.repay_target, repayment);

		expect_events(vec![
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data_1,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data_2,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data_3,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data_4,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data_5,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data_6,
			}
			.into(),
		]);
	});
}

#[test]
fn update_non_existing_pool_data_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(15),
				Some(18),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				None,
				None,
			),
			Error::<Test>::PoolNotFound
		);
	});
}

#[test]
fn update_pool_with_invalid_data_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				// reversed interval, the end precedes the beginning
				Some(20),
				Some(10),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				None,
				None,
			),
			Error::<Test>::InvalidBlockRange
		);

		set_block_number(6);

		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(5),
				Some(20),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				None,
				None,
			),
			Error::<Test>::InvalidBlockRange
		);

		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(0),
				Some(20),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				None,
				None,
			),
			Error::<Test>::InvalidBlockRange
		);

		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(5),
				Some(0),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				None,
				None,
			),
			Error::<Test>::InvalidBlockRange
		);
	});
}

#[test]
fn update_pool_data_without_changes_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
			),
			Error::<Test>::NothingToUpdate
		);
	});
}

#[test]
fn update_pool_data_by_non_owner_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(BOB),
				KUSD_BSX_POOL_ID,
				None,
				Some(15),
				Some(20),
				Some(10_000_000),
				Some(80_000_000),
				None,
				None,
				None,
			),
			Error::<Test>::NotOwner
		);
	});
}

#[test]
fn update_pool_owner_by_new_owner_should_work() {
	predefined_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			Some(BOB),
			Some(15),
			Some(20),
			Some(10_000_000),
			Some(80_000_000),
			None,
			None,
			None,
		));

		let pool_data1 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(BOB),
			KUSD_BSX_POOL_ID,
			Some(ALICE),
			Some(15),
			Some(20),
			Some(10_000_000),
			Some(80_000_000),
			None,
			None,
			None,
		));

		let pool_data2 = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();

		expect_events(vec![
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: pool_data1,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: pool_data2,
			}
			.into(),
		]);
	});
}

#[test]
fn update_pool_data_for_running_lbp_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			Some(15),
			Some(20),
			None,
			None,
			None,
			None,
			None,
		));

		set_block_number(16);

		// update starting block and final weights
		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(15),
				Some(30),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				Some(BOB),
				None,
			),
			Error::<Test>::SaleStarted
		);

		let pool_data = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();

		expect_events(vec![Event::PoolUpdated {
			pool: KUSD_BSX_POOL_ID,
			data: pool_data,
		}
		.into()]);
	});
}

#[test]
fn update_pool_with_existing_fee_collector_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			HDX,
			2_000_000_000,
			20_000_000u32,
			90_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			BOB,
			0,
		));

		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(15),
				Some(18),
				Some(10_000_000),
				Some(80_000_000),
				Some((5, 100)),
				Some(BOB),
				None,
			),
			Error::<Test>::FeeCollectorWithAssetAlreadyUsed
		);
	});
}

#[test]
fn update_pool_interval_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			10_000_000,
			10_000_000,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		set_block_number(15);

		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(16),
				Some(0),
				None,
				None,
				None,
				None,
				None,
			),
			Error::<Test>::InvalidBlockRange
		);

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			Some(16),
			Some(20),
			None,
			None,
			None,
			None,
			None,
		));

		// verify changes
		let updated_pool_data = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(updated_pool_data.start, Some(16));
		assert_eq!(updated_pool_data.end, Some(20));

		expect_events(vec![
			Event::LiquidityAdded {
				who: KUSD_BSX_POOL_ID,
				asset_a: KUSD,
				asset_b: BSX,
				amount_a: 1_000_000_000,
				amount_b: 2_000_000_000,
			}
			.into(),
			Event::PoolUpdated {
				pool: KUSD_BSX_POOL_ID,
				data: updated_pool_data,
			}
			.into(),
		]);
	});
}

#[test]
fn add_liquidity_should_work() {
	predefined_test_ext().execute_with(|| {
		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);
		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		let added_a = 10_000_000_000;
		let added_b = 20_000_000_000;

		assert_ok!(LBPPallet::add_liquidity(
			Origin::signed(ALICE),
			(KUSD, added_a),
			(BSX, added_b),
		));

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: added_a,
			amount_b: added_b,
		}
		.into()]);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);
		assert_eq!(pool_balance_a_after, pool_balance_a_before.saturating_add(added_a));
		assert_eq!(pool_balance_b_after, pool_balance_b_before.saturating_add(added_b));

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(user_balance_a_after, user_balance_a_before.saturating_sub(added_a));
		assert_eq!(user_balance_b_after, user_balance_b_before.saturating_sub(added_b));

		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);
		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::add_liquidity(
			Origin::signed(ALICE),
			(KUSD, added_a),
			(BSX, 0),
		));

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: added_a,
			amount_b: 0,
		}
		.into()]);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);
		assert_eq!(pool_balance_a_after, pool_balance_a_before.saturating_add(added_a));
		assert_eq!(pool_balance_b_after, pool_balance_b_before);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(user_balance_a_after, user_balance_a_before.saturating_sub(added_a));
		assert_eq!(user_balance_b_after, user_balance_b_before);

		// change asset order
		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);
		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::add_liquidity(
			Origin::signed(ALICE),
			(BSX, added_b),
			(KUSD, added_a),
		));

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);
		assert_eq!(pool_balance_a_after, pool_balance_a_before.saturating_add(added_a));
		assert_eq!(pool_balance_b_after, pool_balance_b_before.saturating_add(added_b));

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(user_balance_a_after, user_balance_a_before.saturating_sub(added_a));
		assert_eq!(user_balance_b_after, user_balance_b_before.saturating_sub(added_b));

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: BSX,
			asset_b: KUSD,
			amount_a: added_b,
			amount_b: added_a,
		}
		.into()]);
	});
}

#[test]
fn add_liquidity_by_non_owner_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_eq!(Currency::free_balance(KUSD, &BOB), 1000000000000000);
		assert_eq!(Currency::free_balance(BSX, &BOB), 1000000000000000);

		assert_eq!(Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID), 1_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &KUSD_BSX_POOL_ID), 2_000_000_000);

		assert_noop!(
			LBPPallet::add_liquidity(Origin::signed(BOB), (KUSD, 10_000_000_000), (BSX, 20_000_000_000),),
			Error::<Test>::NotOwner
		);
	});
}

#[test]
fn add_zero_liquidity_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_noop!(
			LBPPallet::add_liquidity(Origin::signed(ALICE), (KUSD, 0), (BSX, 0),),
			Error::<Test>::CannotAddZeroLiquidity
		);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, pool_balance_a_before);
		assert_eq!(pool_balance_b_after, pool_balance_b_before);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(user_balance_a_after, user_balance_a_before);
		assert_eq!(user_balance_b_after, user_balance_b_before);

		// No new events expected
		expect_events(vec![]);
	});
}

#[test]
fn add_liquidity_with_insufficient_balance_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_noop!(
			LBPPallet::add_liquidity(Origin::signed(ALICE), (KUSD, u128::MAX), (BSX, 0),),
			Error::<Test>::InsufficientAssetBalance
		);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, pool_balance_a_before);
		assert_eq!(pool_balance_b_after, pool_balance_b_before);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		assert_eq!(user_balance_a_after, user_balance_a_before);
	});
}

#[test]
fn add_liquidity_after_sale_started_should_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(15);

		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::add_liquidity(
			Origin::signed(ALICE),
			(KUSD, 1_000),
			(BSX, 1_000),
		));

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: 1_000,
			amount_b: 1_000,
		}
		.into()]);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, pool_balance_a_before.saturating_add(1_000));
		assert_eq!(pool_balance_b_after, pool_balance_b_before.saturating_add(1_000));

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);

		assert_eq!(user_balance_a_after, user_balance_a_before.saturating_sub(1_000));
		assert_eq!(user_balance_b_after, user_balance_b_before.saturating_sub(1_000));

		// sale ended at the block number 20
		set_block_number(30);

		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::add_liquidity(
			Origin::signed(ALICE),
			(KUSD, 1_000),
			(BSX, 1_000),
		));

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, pool_balance_a_before.saturating_add(1_000));
		assert_eq!(pool_balance_b_after, pool_balance_b_before.saturating_add(1_000));

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);

		assert_eq!(user_balance_a_after, user_balance_a_before.saturating_sub(1_000));
		assert_eq!(user_balance_b_after, user_balance_b_before.saturating_sub(1_000));

		expect_events(vec![Event::LiquidityAdded {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: 1_000,
			amount_b: 1_000,
		}
		.into()]);
	});
}

#[test]
fn add_liquidity_to_non_existing_pool_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::add_liquidity(Origin::signed(ALICE), (KUSD, 1_000), (HDX, 1_000),),
			Error::<Test>::PoolNotFound
		);
	});
}

#[test]
fn remove_liquidity_should_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(41);

		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID,));

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, 0);
		assert_eq!(pool_balance_b_after, 0);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		assert_eq!(
			user_balance_a_after,
			user_balance_a_before.saturating_add(pool_balance_a_before)
		);

		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(
			user_balance_b_after,
			user_balance_b_before.saturating_add(pool_balance_b_before)
		);

		assert!(!<FeeCollectorWithAsset<Test>>::contains_key(CHARLIE, KUSD));
		assert!(!<PoolData<Test>>::contains_key(KUSD_BSX_POOL_ID));

		expect_events(vec![
			frame_system::Event::KilledAccount {
				account: KUSD_BSX_POOL_ID,
			}
			.into(),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Transfer {
				currency_id: BSX,
				from: KUSD_BSX_POOL_ID,
				to: ALICE,
				amount: 2000000000,
			}),
			Event::LiquidityRemoved {
				who: KUSD_BSX_POOL_ID,
				asset_a: KUSD,
				asset_b: BSX,
				amount_a: pool_balance_a_before,
				amount_b: pool_balance_b_before,
			}
			.into(),
		]);
	});
}

#[test]
fn remove_liquidity_from_not_started_pool_should_work() {
	predefined_test_ext().execute_with(|| {
		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID,));
		expect_events(vec![Event::LiquidityRemoved {
			who: KUSD_BSX_POOL_ID,
			asset_a: KUSD,
			asset_b: BSX,
			amount_a: pool_balance_a_before,
			amount_b: pool_balance_b_before,
		}
		.into()]);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, 0);
		assert_eq!(pool_balance_b_after, 0);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		assert_eq!(
			user_balance_a_after,
			user_balance_a_before.saturating_add(pool_balance_a_before)
		);

		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(
			user_balance_b_after,
			user_balance_b_before.saturating_add(pool_balance_b_before)
		);

		assert!(!<PoolData<Test>>::contains_key(KUSD_BSX_POOL_ID));

		// sale duration is not specified
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			BSX,
			2_000_000_000,
			10_000_000,
			90_000_000,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		let user_balance_a_before = Currency::free_balance(HDX, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(HDX, &HDX_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &HDX_BSX_POOL_ID);

		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(ALICE), HDX_BSX_POOL_ID,));

		expect_events(vec![Event::LiquidityRemoved {
			who: HDX_BSX_POOL_ID,
			asset_a: HDX,
			asset_b: BSX,
			amount_a: pool_balance_a_before,
			amount_b: pool_balance_b_before,
		}
		.into()]);

		let pool_balance_a_after = Currency::free_balance(HDX, &HDX_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &HDX_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, 0);
		assert_eq!(pool_balance_b_after, 0);

		let user_balance_a_after = Currency::free_balance(HDX, &ALICE);
		assert_eq!(
			user_balance_a_after,
			user_balance_a_before.saturating_add(pool_balance_a_before)
		);

		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(
			user_balance_b_after,
			user_balance_b_before.saturating_add(pool_balance_b_before)
		);

		assert!(!<PoolData<Test>>::contains_key(HDX_BSX_POOL_ID));
	});
}

#[test]
fn remove_liquidity_from_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID),
			Error::<Test>::PoolNotFound
		);
	});
}

#[test]
fn remove_liquidity_from_not_finalized_pool_should_not_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(15);

		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_noop!(
			LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID,),
			Error::<Test>::SaleNotEnded
		);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_before, pool_balance_a_after);
		assert_eq!(pool_balance_b_before, pool_balance_b_after);
		assert_eq!(user_balance_a_before, user_balance_a_after);
		assert_eq!(user_balance_b_before, user_balance_b_after);
	});
}

#[test]
fn remove_liquidity_from_finalized_pool_should_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(41);

		let user_balance_a_before = Currency::free_balance(KUSD, &ALICE);
		let user_balance_b_before = Currency::free_balance(BSX, &ALICE);

		let pool_balance_a_before = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_before = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID,));

		let pool_balance_a_after = Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID);
		let pool_balance_b_after = Currency::free_balance(BSX, &KUSD_BSX_POOL_ID);

		assert_eq!(pool_balance_a_after, 0);
		assert_eq!(pool_balance_b_after, 0);

		let user_balance_a_after = Currency::free_balance(KUSD, &ALICE);
		assert_eq!(
			user_balance_a_after,
			user_balance_a_before.saturating_add(pool_balance_a_before)
		);

		let user_balance_b_after = Currency::free_balance(BSX, &ALICE);
		assert_eq!(
			user_balance_b_after,
			user_balance_b_before.saturating_add(pool_balance_b_before)
		);

		assert!(!<PoolData<Test>>::contains_key(KUSD_BSX_POOL_ID));

		expect_events(vec![
			frame_system::Event::KilledAccount {
				account: KUSD_BSX_POOL_ID,
			}
			.into(),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Transfer {
				currency_id: BSX,
				from: KUSD_BSX_POOL_ID,
				to: ALICE,
				amount: 2000000000,
			}),
			Event::LiquidityRemoved {
				who: KUSD_BSX_POOL_ID,
				asset_a: KUSD,
				asset_b: BSX,
				amount_a: pool_balance_a_before,
				amount_b: pool_balance_b_before,
			}
			.into(),
		]);
	});
}

#[test]
fn remove_liquidity_by_non_owner_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::remove_liquidity(Origin::signed(BOB), KUSD_BSX_POOL_ID),
			Error::<Test>::NotOwner
		);
	});
}

#[test]
fn execute_trade_should_work() {
	predefined_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = KUSD_BSX_POOL_ID;

		let amount_in = 5_000_000_u128;
		let amount_b = 10_000_000_u128;
		let t_sell = AMMTransfer {
			origin: ALICE,
			assets: AssetPair { asset_in, asset_out },
			amount: amount_in,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_in, 1_000),
		};

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 0);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);

		assert_ok!(LBPPallet::execute_trade(&t_sell));

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_998_994_999_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_010_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 1_000);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_005_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_990_000_000);

		let t_buy = AMMTransfer {
			origin: ALICE,
			assets: AssetPair { asset_in, asset_out },
			amount: amount_in,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_in, 1_000),
		};

		assert_ok!(LBPPallet::execute_trade(&t_buy));

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_998_989_998_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_020_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 2_000);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_010_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_980_000_000);
	});
}

#[test]
fn trade_fails_when_first_fee_lesser_than_existential_deposit() {
	predefined_test_ext().execute_with(|| {
		let trade = AMMTransfer {
			origin: ALICE,
			assets: AssetPair {
				asset_in: KUSD,
				asset_out: BSX,
			},
			amount: 1000,
			amount_b: 1000,
			discount: false,
			discount_amount: 0_u128,
			fee: (KUSD, EXISTENTIAL_DEPOSIT - 1),
		};

		assert_noop!(
			LBPPallet::execute_trade(&trade),
			orml_tokens::Error::<Test>::ExistentialDeposit
		);
	});
}

// // This test ensure storage was not modified on error
#[test]
fn execute_trade_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		let amount_in = 5_000_000_u128;
		let amount_b = 10_000_000_000_000_000u128;
		let t = AMMTransfer {
			origin: ALICE,
			assets: AssetPair { asset_in, asset_out },
			amount: amount_in,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_in, 1_000),
		};

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);

		assert_noop!(LBPPallet::execute_trade(&t), orml_tokens::Error::<Test>::BalanceTooLow);

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);
	});
}

#[test]
fn execute_sell_should_work() {
	predefined_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		let amount_in = 8_000_000_u128;
		let amount_b = 20_000_000_u128;
		let t = AMMTransfer {
			origin: ALICE,
			assets: AssetPair { asset_in, asset_out },
			amount: amount_in,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_in, 1_000),
		};

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);

		assert_ok!(LBPPallet::execute_sell(&t));

		expect_events(vec![Event::SellExecuted {
			who: ALICE,
			asset_in,
			asset_out,
			amount: amount_in,
			sale_price: amount_b,
			fee_asset: asset_in,
			fee_amount: 1_000,
		}
		.into()]);

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_998_991_999_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_020_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 1_000);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_008_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_980_000_000);

		expect_events(vec![Event::SellExecuted {
			who: ALICE,
			asset_in,
			asset_out,
			amount: 8_000_000,
			sale_price: 20_000_000,
			fee_asset: asset_in,
			fee_amount: 1_000,
		}
		.into()]);
	});
}

// This test ensure storage was not modified on error
#[test]
fn execute_sell_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let t = AMMTransfer {
			origin: ALICE,
			assets: AssetPair {
				asset_in: KUSD,
				asset_out: BSX,
			},
			amount: 8_000_000_000_u128,
			amount_b: 200_000_000_000_000_u128,
			discount: false,
			discount_amount: 0_u128,
			fee: (KUSD, 1_000),
		};

		assert_eq!(Currency::free_balance(KUSD, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID), 1_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &KUSD_BSX_POOL_ID), 2_000_000_000);

		assert_noop!(LBPPallet::execute_sell(&t), orml_tokens::Error::<Test>::BalanceTooLow);

		assert_eq!(Currency::free_balance(KUSD, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID), 1_000_000_000);
		assert_eq!(Currency::free_balance(BSX, &KUSD_BSX_POOL_ID), 2_000_000_000);
	});
}

#[test]
fn zero_weight_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				ETH,
				1_000_000_000,
				KUSD,
				2_000_000_000,
				0u32,
				20u32,
				WeightCurveType::Linear,
				DEFAULT_FEE,
				CHARLIE,
				0,
			),
			Error::<Test>::InvalidWeight
		);

		let call = Call::LBPPallet(crate::Call::<Test>::update_pool_data {
			pool_id: KUSD_BSX_POOL_ID,
			pool_owner: None,
			start: Some(15),
			end: Some(18),
			initial_weight: Some(0),
			final_weight: Some(80),
			fee: Some((5, 100)),
			fee_collector: Some(BOB),
			repay_target: Some(0),
		});

		assert_noop!(call.dispatch(Origin::signed(ALICE)), Error::<Test>::InvalidWeight);
	});
}

#[test]
fn execute_buy_should_work() {
	predefined_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		let amount_in = 8_000_000_u128;
		let amount_b = 20_000_000_u128;
		let t = AMMTransfer {
			origin: ALICE,
			assets: AssetPair { asset_in, asset_out },
			amount: amount_in,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_in, 1_000),
		};

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 0);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);

		assert_ok!(LBPPallet::execute_buy(&t));

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_998_991_999_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_020_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 1_000);
		assert_eq!(Currency::free_balance(asset_out, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_008_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_980_000_000);

		expect_events(vec![Event::BuyExecuted {
			who: ALICE,
			asset_out,
			asset_in,
			amount: 8_000_000,
			buy_price: 20_000_000,
			fee_asset: asset_in,
			fee_amount: 1_000,
		}
		.into()]);
	});
}

// This test ensures storage was not modified on error
#[test]
fn execute_buy_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		let amount_in = 8_000_000_000_u128;
		let amount_b = 200_000_000_000_000_u128;
		let t = AMMTransfer {
			origin: ALICE,
			assets: AssetPair { asset_in, asset_out },
			amount: amount_in,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_in, 1_000),
		};

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);

		assert_noop!(LBPPallet::execute_buy(&t), orml_tokens::Error::<Test>::BalanceTooLow);

		assert_eq!(Currency::free_balance(asset_in, &ALICE), 999_999_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &ALICE), 999_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 0);

		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 2_000_000_000);
	});
}

#[test]
fn sell_zero_amount_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::sell(Origin::signed(BOB), KUSD, BSX, 0_u128, 200_000_u128),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn buy_zero_amount_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::buy(Origin::signed(BOB), KUSD, BSX, 0_u128, 200_000_u128),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn sell_to_non_existing_pool_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::sell(Origin::signed(BOB), KUSD, ETH, 800_000_u128, 200_000_u128),
			Error::<Test>::PoolNotFound
		);
	});
}

#[test]
fn buy_from_non_existing_pool_should_not_work() {
	predefined_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::buy(Origin::signed(BOB), KUSD, ETH, 800_000_u128, 200_000_u128),
			Error::<Test>::PoolNotFound
		);
	});
}

#[test]
fn exceed_max_in_ratio_should_not_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(11); //start sale
		assert_noop!(
			LBPPallet::sell(
				Origin::signed(BOB),
				KUSD,
				BSX,
				1_000_000_000 / LBPPallet::get_max_in_ratio() + 1,
				200_000_u128
			),
			Error::<Test>::MaxInRatioExceeded
		);

		// 1/2 should not work
		assert_noop!(
			LBPPallet::sell(Origin::signed(BOB), KUSD, BSX, 1_000_000_000 / 2, 200_000_u128),
			Error::<Test>::MaxInRatioExceeded
		);

		// max ratio should work
		assert_ok!(LBPPallet::sell(
			Origin::signed(BOB),
			KUSD,
			BSX,
			1_000_000_000 / LBPPallet::get_max_in_ratio(),
			2_000_u128
		));
	});
}

#[test]
fn exceed_max_out_ratio_should_not_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(11); //start sale

		// max_ratio_out + 1 should not work
		assert_noop!(
			LBPPallet::buy(
				Origin::signed(BOB),
				BSX,
				KUSD,
				2_000_000_000 / LBPPallet::get_max_out_ratio() + 1,
				200_000_u128
			),
			Error::<Test>::MaxOutRatioExceeded
		);

		// 1/2 should not work
		assert_noop!(
			LBPPallet::buy(Origin::signed(BOB), BSX, KUSD, 2_000_000_000 / 2, 200_000_u128),
			Error::<Test>::MaxOutRatioExceeded
		);
	});
}

#[test]
fn trade_in_non_running_pool_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let who = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let amount = 800_000_u128;
		let limit = 200_000_u128;

		//sale not started
		set_block_number(9);
		assert_noop!(
			LBPPallet::sell(Origin::signed(who), asset_in, asset_out, amount, limit),
			Error::<Test>::SaleIsNotRunning
		);
		assert_noop!(
			LBPPallet::buy(Origin::signed(who), asset_out, asset_in, amount, limit),
			Error::<Test>::SaleIsNotRunning
		);

		//sale ended
		set_block_number(41);
		assert_noop!(
			LBPPallet::sell(Origin::signed(who), asset_in, asset_out, amount, limit),
			Error::<Test>::SaleIsNotRunning
		);
		assert_noop!(
			LBPPallet::buy(Origin::signed(who), asset_out, asset_in, amount, limit),
			Error::<Test>::SaleIsNotRunning
		);
	});
}

#[test]
fn exceed_trader_limit_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let who = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let amount = 800_000_u128;
		let sell_limit = 800_000_u128;
		let buy_limit = 1_000_u128;

		//start sale
		set_block_number(11);
		assert_noop!(
			LBPPallet::sell(Origin::signed(who), asset_in, asset_out, amount, sell_limit),
			Error::<Test>::TradingLimitReached
		);

		assert_noop!(
			LBPPallet::buy(Origin::signed(who), asset_out, asset_in, amount, buy_limit),
			Error::<Test>::TradingLimitReached
		);
	});
}

#[test]
fn sell_with_insufficient_balance_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let who = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let amount = 1_000_000_u128;

		Currency::set_balance(Origin::root(), who, asset_in, 100_000, 0).unwrap();
		Currency::set_balance(Origin::root(), who, asset_out, 100_000, 0).unwrap();

		//start sale
		set_block_number(11);

		assert_noop!(
			LBPPallet::sell(Origin::signed(who), asset_in, asset_out, amount, 800_000_u128),
			Error::<Test>::InsufficientAssetBalance
		);

		// swap assets
		assert_noop!(
			LBPPallet::sell(Origin::signed(who), asset_out, asset_in, amount, 800_000_u128),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn buy_with_insufficient_balance_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let who = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let amount = 1_000_000_u128;

		Currency::set_balance(Origin::root(), who, asset_in, 100_000, 0).unwrap();
		Currency::set_balance(Origin::root(), who, asset_out, 100_000, 0).unwrap();

		//start sale
		set_block_number(11);

		assert_noop!(
			LBPPallet::buy(Origin::signed(who), asset_out, asset_in, amount, 2_000_000_u128),
			Error::<Test>::InsufficientAssetBalance
		);

		// swap assets
		assert_noop!(
			LBPPallet::buy(Origin::signed(who), asset_in, asset_out, amount, 2_000_000_u128),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn inverted_operations_should_be_equal() {
	let buy = predefined_test_ext().execute_with(|| {
		run_to_sale_start();
		assert_ok!(LBPPallet::buy(
			Origin::signed(BOB),
			BSX,
			KUSD,
			10_000_000_u128,
			21_000_000_u128
		));
		(
			Currency::free_balance(KUSD, &BOB),
			Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID),
			Currency::free_balance(KUSD, &CHARLIE),
		)
	});
	let sell = predefined_test_ext().execute_with(|| {
		run_to_sale_start();
		assert_ok!(LBPPallet::sell(
			Origin::signed(BOB),
			KUSD,
			BSX,
			20_252_524_u128,
			9_000_000_u128
		));
		(
			Currency::free_balance(KUSD, &BOB),
			Currency::free_balance(KUSD, &KUSD_BSX_POOL_ID),
			Currency::free_balance(KUSD, &CHARLIE),
		)
	});
	assert_eq!(buy, sell);
}

#[test]
fn buy_should_work() {
	predefined_test_ext().execute_with(|| {
		let buyer = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		//start sale
		set_block_number(11);
		assert_ok!(LBPPallet::buy(
			Origin::signed(buyer),
			asset_out,
			asset_in,
			10_000_000_u128,
			2_000_000_000_u128
		));

		assert_eq!(Currency::free_balance(asset_in, &buyer), 999_999_982_069_402);
		assert_eq!(Currency::free_balance(asset_out, &buyer), 1_000_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_017_894_738);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_990_000_000);

		// test buy where the amount_in is less than the amount_out
		let asset_in = HDX;
		let asset_out = BSX;
		let pool_id2 = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			BSX,
			2_000_000_000,
			80_000_000u32,
			10_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		let pool_data1 = LBPPallet::pool_data(pool_id2).unwrap();
		expect_events(vec![
			Event::PoolCreated {
				pool: pool_id2,
				data: pool_data1,
			}
			.into(),
			frame_system::Event::NewAccount { account: pool_id2 }.into(),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Endowed {
				currency_id: HDX,
				who: HDX_BSX_POOL_ID,
				amount: 1000000000,
			}),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Transfer {
				currency_id: HDX,
				from: ALICE,
				to: HDX_BSX_POOL_ID,
				amount: 1000000000,
			}),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Endowed {
				currency_id: BSX,
				who: HDX_BSX_POOL_ID,
				amount: 2000000000,
			}),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Transfer {
				currency_id: BSX,
				from: ALICE,
				to: HDX_BSX_POOL_ID,
				amount: 2000000000,
			}),
			mock::RuntimeEvent::LBPPallet(Event::LiquidityAdded {
				who: pool_id2,
				asset_a: HDX,
				asset_b: BSX,
				amount_a: 1_000_000_000,
				amount_b: 2_000_000_000,
			}),
		]);

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			HDX_BSX_POOL_ID,
			None,
			Some(20),
			Some(30),
			None,
			None,
			None,
			None,
			None
		));

		let pool_data2 = LBPPallet::pool_data(pool_id2).unwrap();

		expect_events(vec![Event::PoolUpdated {
			pool: pool_id2,
			data: pool_data2,
		}
		.into()]);

		//start sale
		set_block_number(21);
		assert_ok!(LBPPallet::buy(
			Origin::signed(buyer),
			asset_out,
			asset_in,
			10_000_000_u128,
			2_000_000_000_u128
		));

		assert_eq!(Currency::free_balance(asset_in, &buyer), 999_999_998_144_325);
		assert_eq!(Currency::free_balance(asset_out, &buyer), 1_000_000_020_000_000);
		assert_eq!(Currency::free_balance(asset_in, &pool_id2), 1_001_851_965);
		assert_eq!(Currency::free_balance(asset_out, &pool_id2), 1_990_000_000);
	});
}

#[test]
fn buy_should_work_when_limit_is_set_above_account_balance() {
	predefined_test_ext().execute_with(|| {
		let buyer = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;

		//start sale
		set_block_number(11);

		assert_ok!(LBPPallet::buy(
			Origin::signed(buyer),
			asset_out,
			asset_in,
			10_000_000_u128,
			u128::MAX,
		));

		expect_events(vec![Event::BuyExecuted {
			who: buyer,
			asset_out: BSX,
			asset_in: KUSD,
			amount: 17_894_738,
			buy_price: 10_000_000,
			fee_asset: KUSD,
			fee_amount: 35_860,
		}
		.into()]);

		// swap assets
		set_block_number(11);
		assert_ok!(LBPPallet::buy(
			Origin::signed(buyer),
			asset_in,
			asset_out,
			10_000_000_u128,
			u128::MAX,
		));

		expect_events(vec![Event::BuyExecuted {
			who: buyer,
			asset_out: KUSD,
			asset_in: BSX,
			amount: 5_560_304,
			buy_price: 10_000_000,
			fee_asset: KUSD,
			fee_amount: 20_000,
		}
		.into()]);
	});
}

#[test]
fn update_pool_data_after_sale_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let buyer = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		//start sale
		set_block_number(11);
		assert_ok!(LBPPallet::buy(
			Origin::signed(buyer),
			asset_out,
			asset_in,
			10_000_000_u128,
			2_000_000_000_u128
		));

		assert_eq!(Currency::free_balance(asset_in, &buyer), 999_999_982_069_402);
		assert_eq!(Currency::free_balance(asset_out, &buyer), 1_000_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_017_894_738);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_990_000_000);
		assert_eq!(Currency::free_balance(asset_in, &CHARLIE), 35_860);

		set_block_number(41);

		expect_events(vec![Event::BuyExecuted {
			who: buyer,
			asset_out: BSX,
			asset_in: KUSD,
			amount: 17_894_738,
			buy_price: 10_000_000,
			fee_asset: KUSD,
			fee_amount: 35_860,
		}
		.into()]);

		assert_noop!(
			LBPPallet::update_pool_data(
				Origin::signed(ALICE),
				KUSD_BSX_POOL_ID,
				None,
				Some(50),
				Some(60),
				None,
				None,
				None,
				None,
				None,
			),
			Error::<Test>::SaleStarted
		);
	});
}

#[test]
fn sell_should_work() {
	predefined_test_ext().execute_with(|| {
		let buyer = BOB;
		let asset_in = KUSD;
		let asset_out = BSX;
		let pool_id = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });

		//start sale
		set_block_number(11);

		assert_ok!(LBPPallet::sell(
			Origin::signed(buyer),
			asset_in,
			asset_out,
			10_000_000_u128,
			2_000_u128
		));

		assert_eq!(Currency::free_balance(asset_in, &buyer), 999_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_out, &buyer), 1_000_000_005_605_138);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_009_980_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_994_394_862);

		// test buy where the amount_in is less than the amount_out
		let asset_in = HDX;
		let asset_out = BSX;
		let pool_id2 = LBPPallet::get_pair_id(AssetPair { asset_in, asset_out });
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			BSX,
			2_000_000_000,
			80_000_000u32,
			10_000_000u32,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));
		let pool_data1 = LBPPallet::pool_data(pool_id2).unwrap();

		expect_events(vec![
			Event::PoolCreated {
				pool: pool_id2,
				data: pool_data1,
			}
			.into(),
			frame_system::Event::NewAccount { account: pool_id2 }.into(),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Endowed {
				currency_id: HDX,
				who: HDX_BSX_POOL_ID,
				amount: 1000000000,
			}),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Transfer {
				currency_id: HDX,
				from: ALICE,
				to: HDX_BSX_POOL_ID,
				amount: 1000000000,
			}),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Endowed {
				currency_id: BSX,
				who: HDX_BSX_POOL_ID,
				amount: 2000000000,
			}),
			mock::RuntimeEvent::Currency(orml_tokens::Event::Transfer {
				currency_id: BSX,
				from: ALICE,
				to: HDX_BSX_POOL_ID,
				amount: 2000000000,
			}),
			mock::RuntimeEvent::LBPPallet(Event::LiquidityAdded {
				who: pool_id2,
				asset_a: HDX,
				asset_b: BSX,
				amount_a: 1_000_000_000,
				amount_b: 2_000_000_000,
			}),
		]);

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			HDX_BSX_POOL_ID,
			None,
			Some(20),
			Some(30),
			None,
			None,
			None,
			None,
			None
		));

		let pool_data2 = LBPPallet::pool_data(pool_id2).unwrap();

		expect_events(vec![Event::PoolUpdated {
			pool: pool_id2,
			data: pool_data2,
		}
		.into()]);

		//start sale
		set_block_number(21);
		assert_ok!(LBPPallet::sell(
			Origin::signed(buyer),
			asset_out,
			asset_in,
			10_000_000_u128,
			2_000_u128
		));

		assert_eq!(Currency::free_balance(asset_in, &buyer), 1_000_000_001_839_320);
		assert_eq!(Currency::free_balance(asset_out, &buyer), 999_999_995_605_138);
		assert_eq!(Currency::free_balance(asset_in, &pool_id2), 998_156_994);
		assert_eq!(Currency::free_balance(asset_out, &pool_id2), 2_010_000_000);
	});
}

#[test]
fn sell_should_work_with_different_token_precisions() {
	new_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let reserve_in = 1_000 * 1_000_000;
		let reserve_out = 1_000 * 1_000_000_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let sell_amount = 1_000_000;

		assert_ok!(LBPPallet::sell(
			Origin::signed(BOB),
			asset_in,
			asset_out,
			sell_amount,
			0
		));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - sell_amount);
		assert_eq!(
			Currency::free_balance(asset_out, &BOB),
			INITIAL_BALANCE + 999_000_999_000
		);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_001_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_000_999_001_000);
	});

	new_test_ext().execute_with(|| {
		let asset_in = BSX;
		let asset_out = KUSD;
		let reserve_in = 1_000 * 1_000_000_000_000;
		let reserve_out = 1_000 * 1_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let sell_amount = 1_000_000_000_000;

		assert_ok!(LBPPallet::sell(
			Origin::signed(BOB),
			asset_in,
			asset_out,
			sell_amount,
			0
		));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - sell_amount);
		assert_eq!(Currency::free_balance(asset_out, &BOB), INITIAL_BALANCE + 999_000);
		assert_eq!(
			Currency::free_balance(asset_in, &pool_id),
			INITIAL_BALANCE + 1_000_000_000_000
		);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_001_000);
	});

	new_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = ETH;
		let reserve_in = 1_000 * 1_000_000;
		let reserve_out = 1_000 * 1_000_000_000_000_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let sell_amount = 1_000_000;

		assert_ok!(LBPPallet::sell(
			Origin::signed(BOB),
			asset_in,
			asset_out,
			sell_amount,
			0
		));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - sell_amount);
		assert_eq!(
			Currency::free_balance(asset_out, &BOB),
			INITIAL_ETH_BALANCE + 999_000_999_000_999_000
		);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_001_000_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_000_999_000_999_001_000);
	});

	// selling a small amount can result in receiving 0
	new_test_ext().execute_with(|| {
		let asset_in = BSX;
		let asset_out = KUSD;
		let reserve_in = 1_000 * 1_000_000_000_000;
		let reserve_out = 1_000 * 1_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let sell_amount = 100_000;

		assert_ok!(LBPPallet::sell(
			Origin::signed(BOB),
			asset_in,
			asset_out,
			sell_amount,
			0
		));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - sell_amount);
		assert_eq!(Currency::free_balance(asset_out, &BOB), INITIAL_BALANCE);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_000_100_000);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 1_000_000_000);
	});
}

#[test]
fn buy_should_work_with_different_token_precisions() {
	new_test_ext().execute_with(|| {
		let asset_in = BSX;
		let asset_out = KUSD;
		let reserve_in = 1_000 * 1_000_000_000_000;
		let reserve_out = 1_000 * 1_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let buy_amount = 1_000_000;

		assert_ok!(LBPPallet::buy(
			Origin::signed(BOB),
			asset_out,
			asset_in,
			buy_amount,
			2_000_000_000_000,
		));

		assert_eq!(Currency::free_balance(asset_in, &BOB), 998_998_998_998_997);
		assert_eq!(Currency::free_balance(asset_out, &BOB), INITIAL_BALANCE + buy_amount);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_001_001_001_001_003);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_000_000);
	});

	new_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = BSX;
		let reserve_in = 1_000 * 1_000_000;
		let reserve_out = 1_000 * 1_000_000_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let buy_amount = 1_000_000;

		assert_ok!(LBPPallet::buy(Origin::signed(BOB), asset_out, asset_in, buy_amount, 10,));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - 3);
		assert_eq!(Currency::free_balance(asset_out, &BOB), INITIAL_BALANCE + buy_amount);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_003);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_999_999_000_000);
	});

	new_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = ETH;
		let reserve_in = 1_000 * 1_000_000;
		let reserve_out = 1_000 * 1_000_000_000_000_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		let buy_amount = 1_000_000;

		assert_ok!(LBPPallet::buy(Origin::signed(BOB), asset_out, asset_in, buy_amount, 10,));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - 2);
		assert_eq!(
			Currency::free_balance(asset_out, &BOB),
			INITIAL_ETH_BALANCE + buy_amount
		);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_002);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_999_999_999_999_000_000);
	});
}

#[test]
fn buy_small_amount_should_return_non_zero_amount() {
	new_test_ext().execute_with(|| {
		let asset_in = KUSD;
		let asset_out = ETH;
		let reserve_in = 1_000 * 1_000_000;
		let reserve_out = 1_000 * 1_000_000_000_000_000_000;

		let pool_id = start_50_50_lbp_without_fee_and_repay_target(asset_in, reserve_in, asset_out, reserve_out);

		//start sale
		set_block_number(11);

		let buy_amount = 1_000;

		assert_ok!(LBPPallet::buy(Origin::signed(BOB), asset_out, asset_in, buy_amount, 10,));

		assert_eq!(Currency::free_balance(asset_in, &BOB), INITIAL_BALANCE - 2);
		assert_eq!(
			Currency::free_balance(asset_out, &BOB),
			INITIAL_ETH_BALANCE + buy_amount
		);
		assert_eq!(Currency::free_balance(asset_in, &pool_id), 1_000_000_002);
		assert_eq!(Currency::free_balance(asset_out, &pool_id), 999_999_999_999_999_999_000);
	});
}

#[test]
fn zero_fee_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000,
			80_000_000,
			WeightCurveType::Linear,
			(0, 100),
			CHARLIE,
			0,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			Some(10),
			Some(20),
			None,
			None,
			None,
			None,
			None
		));

		//start sale
		set_block_number(11);

		assert_ok!(LBPPallet::sell(Origin::signed(ALICE), KUSD, BSX, 1_000, 1,));
	});
}

#[test]
fn invalid_fee_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LBPPallet::create_pool(
				Origin::root(),
				ALICE,
				KUSD,
				1_000_000_000,
				BSX,
				2_000_000_000,
				20_000_000,
				80_000_000,
				WeightCurveType::Linear,
				(10, 0),
				CHARLIE,
				0,
			),
			Error::<Test>::FeeAmountInvalid
		);
	});
}

#[test]
fn amm_trait_should_work() {
	predefined_test_ext().execute_with(|| {
		let asset_pair = AssetPair {
			asset_in: KUSD,
			asset_out: BSX,
		};
		let reversed_asset_pair = AssetPair {
			asset_in: BSX,
			asset_out: KUSD,
		};
		let non_existing_asset_pair = AssetPair {
			asset_in: BSX,
			asset_out: HDX,
		};

		set_block_number(11);

		assert!(LBPPallet::exists(asset_pair));
		assert!(LBPPallet::exists(reversed_asset_pair));
		assert!(!LBPPallet::exists(non_existing_asset_pair));

		assert_eq!(LBPPallet::get_pair_id(asset_pair), KUSD_BSX_POOL_ID);
		assert_eq!(LBPPallet::get_pair_id(reversed_asset_pair), KUSD_BSX_POOL_ID);

		assert_eq!(LBPPallet::get_pool_assets(&KUSD_BSX_POOL_ID), Some(vec![KUSD, BSX]));
		assert_eq!(LBPPallet::get_pool_assets(&HDX_BSX_POOL_ID), None);

		// calculate_spot_price is tested in get_spot_price_should_work
		// execute_sell and execute_buy is tested in execute_sell_should_work and execute_buy_should_work

		let who = BOB;
		let amount_in = 1_000_000;
		let sell_limit = 100_000;
		let pool_id = LBPPallet::get_pair_id(asset_pair);
		let pool_data = LBPPallet::pool_data(pool_id).unwrap();

		let fee = LBPPallet::calculate_fees(&pool_data, amount_in).unwrap();

		let t_sell = AMMTransfer {
			origin: who,
			assets: asset_pair,
			amount: amount_in - fee,
			amount_b: 563_741,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_pair.asset_in, fee),
		};

		assert_eq!(
			LBPPallet::validate_sell(&who, asset_pair, amount_in, sell_limit, false).unwrap(),
			t_sell
		);

		let amount_b = 1_000_000;
		let buy_limit = 10_000_000;
		let t_buy = AMMTransfer {
			origin: who,
			assets: asset_pair,
			amount: 1_771_197,
			amount_b,
			discount: false,
			discount_amount: 0_u128,
			fee: (asset_pair.asset_in, 3_548),
		};
		assert_eq!(
			LBPPallet::validate_buy(&who, asset_pair, amount_in, buy_limit, false).unwrap(),
			t_buy
		);

		assert_eq!(
			LBPPallet::get_min_trading_limit(),
			<Test as Config>::MinTradingLimit::get()
		);
		assert_eq!(
			LBPPallet::get_min_pool_liquidity(),
			<Test as Config>::MinPoolLiquidity::get()
		);
		assert_eq!(LBPPallet::get_max_in_ratio(), <Test as Config>::MaxInRatio::get());
		assert_eq!(LBPPallet::get_max_out_ratio(), <Test as Config>::MaxOutRatio::get());

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000,
			80_000_000,
			WeightCurveType::Linear,
			(400, 1_000),
			CHARLIE,
			0,
		));

		let pool_id = LBPPallet::get_pair_id(AssetPair {
			asset_in: HDX,
			asset_out: BSX,
		});
		// existing pool
		assert_eq!(LBPPallet::get_fee(&pool_id), (400, 1_000));
		// not existing pool
		assert_eq!(LBPPallet::get_fee(&1_234), (0, 0));
	});
}

#[test]
fn get_spot_price_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000,
			90_000_000,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			Some(10),
			Some(20),
			None,
			None,
			None,
			None,
			None
		));

		set_block_number(10);

		let price = hydra_dx_math::lbp::calculate_spot_price(
			1_000_000_000_u128,
			2_000_000_000_u128,
			20_u32,
			80_u32,
			1_000_000_u128,
		)
		.unwrap_or_else(|_| BalanceOf::<Test>::zero());

		assert_eq!(LBPPallet::get_spot_price_unchecked(KUSD, BSX, 1_000_000_u128), price);

		// swap assets
		let price = hydra_dx_math::lbp::calculate_spot_price(
			2_000_000_000_u128,
			1_000_000_000_u128,
			80_u32,
			20_u32,
			1_000_000_u128,
		)
		.unwrap_or_else(|_| BalanceOf::<Test>::zero());

		assert_eq!(LBPPallet::get_spot_price_unchecked(BSX, KUSD, 1_000_000_u128), price);

		// change weights
		set_block_number(20);

		let price = hydra_dx_math::lbp::calculate_spot_price(
			1_000_000_000_u128,
			2_000_000_000_u128,
			90_u32,
			10_u32,
			1_000_000_u128,
		)
		.unwrap_or_else(|_| BalanceOf::<Test>::zero());

		assert_eq!(LBPPallet::get_spot_price_unchecked(KUSD, BSX, 1_000_000), price);

		// pool does not exist
		assert_eq!(LBPPallet::get_spot_price_unchecked(KUSD, HDX, 1_000_000), 0);

		// overflow
		assert_eq!(LBPPallet::get_spot_price_unchecked(KUSD, BSX, u128::MAX), 0);

		// sale ended
		set_block_number(21);
		assert_eq!(LBPPallet::get_spot_price_unchecked(KUSD, BSX, 1_000_000), 0);
	});
}

#[test]
fn simulate_lbp_event_should_work() {
	new_test_ext().execute_with(|| {
		// setup
		let pool_owner = BOB;
		let lbp_participant = CHARLIE;

		let asset_in = BSX;
		let asset_in_pool_reserve: u128 = 1_000_000;
		let owner_initial_asset_in_balance: u128 = 1_000_000_000_000;
		let lbp_participant_initial_asset_in_balance: u128 = 1_000_000_000_000;

		let asset_in_initial_weight = 10_000_000; // 10%
		let asset_in_final_weight = 75_000_000; // 75%

		let asset_out = HDX;
		let asset_out_pool_reserve: u128 = 500_000_000;
		let owner_initial_asset_out_balance: u128 = 1_000_000_000_000;
		let lbp_participant_initial_asset_out_balance: u128 = 1_000_000_000_000;

		let sale_start: u64 = 1_000;
		let sale_end: u64 = 22_600; // in blocks; 3 days

		let trades = generate_trades(sale_start, sale_end, 200_000_000, 2);

		let fee = (9, 1_000);

		let fee_collector = ALICE;

		let trade_limit_factor: u128 = 1_000;

		// preparations
		let asset_pair = AssetPair { asset_in, asset_out };
		let pool_account = LBPPallet::get_pair_id(asset_pair);

		Currency::set_balance(Origin::root(), fee_collector, asset_in, 0, 0).unwrap();
		Currency::set_balance(Origin::root(), fee_collector, asset_out, 0, 0).unwrap();

		Currency::set_balance(Origin::root(), pool_owner, asset_in, 0, 0).unwrap();
		Currency::set_balance(Origin::root(), pool_owner, asset_out, 0, 0).unwrap();

		Currency::set_balance(
			Origin::root(),
			pool_owner,
			asset_in,
			owner_initial_asset_in_balance
				.checked_add(asset_in_pool_reserve)
				.unwrap(),
			0,
		)
		.unwrap();
		Currency::set_balance(
			Origin::root(),
			pool_owner,
			asset_out,
			owner_initial_asset_out_balance
				.checked_add(asset_out_pool_reserve)
				.unwrap(),
			0,
		)
		.unwrap();

		<Test as Config>::MultiCurrency::update_balance(
			asset_in,
			&lbp_participant,
			lbp_participant_initial_asset_in_balance.try_into().unwrap(),
		)
		.unwrap();
		<Test as Config>::MultiCurrency::update_balance(
			asset_out,
			&lbp_participant,
			lbp_participant_initial_asset_out_balance.try_into().unwrap(),
		)
		.unwrap();

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			pool_owner,
			asset_in,
			asset_in_pool_reserve,
			asset_out,
			asset_out_pool_reserve,
			asset_in_initial_weight,
			asset_in_final_weight,
			WeightCurveType::Linear,
			fee,
			fee_collector,
			0,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(pool_owner),
			pool_account,
			None,
			Some(sale_start),
			Some(sale_end),
			None,
			None,
			None,
			None,
			None
		));

		set_block_number(sale_start.checked_sub(1).unwrap());
		//frame_system::Pallet::<Test>::set_block_number(sale_start + 1);

		// start LBP
		for block_num in sale_start..=sale_end {
			set_block_number(block_num);

			if let Some((is_buy, amount)) = trades.get(&block_num) {
				if *is_buy {
					assert_ok!(LBPPallet::buy(
						Origin::signed(lbp_participant),
						asset_out,
						asset_in,
						*amount,
						amount.saturating_mul(trade_limit_factor)
					));
				} else {
					assert_ok!(LBPPallet::sell(
						Origin::signed(lbp_participant),
						asset_out,
						asset_in,
						*amount,
						amount.checked_div(trade_limit_factor).unwrap()
					));
				}
			}
		}

		// end LBP and consolidate results
		set_block_number(sale_end.checked_add(1).unwrap());

		let pool_account_result_asset_in = Currency::free_balance(asset_in, &pool_account);
		let pool_account_result_asset_out = Currency::free_balance(asset_out, &pool_account);

		assert_eq!(
			Currency::free_balance(asset_in, &pool_owner),
			owner_initial_asset_in_balance
		);
		assert_eq!(
			Currency::free_balance(asset_out, &pool_owner),
			owner_initial_asset_out_balance
		);

		// TODO: figure out why this changed so much: 4_893_544 -> 4_892_751
		assert_eq!(Currency::free_balance(asset_in, &pool_account), 4_892_751);
		assert_eq!(Currency::free_balance(asset_out, &pool_account), 125_000_009);

		// TODO: figure out why this changed so much: 999_996_061_843 -> 999_996_062_654
		assert_eq!(Currency::free_balance(asset_in, &lbp_participant), 999_996_062_654);
		assert_eq!(Currency::free_balance(asset_out, &lbp_participant), 1_000_374_999_991);

		// remove liquidity from the pool
		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(pool_owner), pool_account));

		assert_eq!(Currency::free_balance(asset_in, &pool_account), 0);
		assert_eq!(Currency::free_balance(asset_out, &pool_account), 0);

		assert_eq!(
			Currency::free_balance(asset_in, &pool_owner),
			owner_initial_asset_in_balance
				.checked_add(pool_account_result_asset_in)
				.unwrap()
		);
		assert_eq!(
			Currency::free_balance(asset_out, &pool_owner),
			owner_initial_asset_out_balance
				.checked_add(pool_account_result_asset_out)
				.unwrap()
		);

		assert_eq!(Currency::free_balance(asset_in, &fee_collector), 44_595);
		assert_eq!(Currency::free_balance(asset_out, &fee_collector), 0);
	});
}

#[test]
fn validate_trade_should_work() {
	predefined_test_ext().execute_with(|| {
		run_to_sale_start();

		assert_eq!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				1_000_000_u128,
				2_157_153_u128,
				false
			)
			.unwrap(),
			AMMTransfer {
				origin: ALICE,
				assets: AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				amount: 1_998_500_u128,
				amount_b: 1_000_000_u128,
				discount: false,
				discount_amount: 0_u128,
				fee: (KUSD, 4_004),
			}
		);

		assert_eq!(
			LBPPallet::validate_sell(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				1_000_000_u128,
				2_000_u128,
				false
			)
			.unwrap(),
			AMMTransfer {
				origin: ALICE,
				assets: AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				amount: 998_000_u128,
				amount_b: 499_687_u128,
				discount: false,
				discount_amount: 0_u128,
				fee: (KUSD, 2000),
			}
		);
	});
}

#[test]
fn validate_trade_should_not_work() {
	predefined_test_ext().execute_with(|| {
		set_block_number(9);

		assert_noop!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				1_000_000_u128,
				2_157_153_u128,
				false,
			),
			Error::<Test>::SaleIsNotRunning
		);

		set_block_number(10);

		assert_noop!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				0,
				2_157_153_u128,
				false,
			),
			Error::<Test>::ZeroAmount
		);

		Currency::set_balance(Origin::root(), ALICE, KUSD, 10_000_000, 0).unwrap();
		assert_err!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				100_000_000u128,
				3_000_000_000_u128,
				false,
			),
			Error::<Test>::InsufficientAssetBalance
		);
		// set the balance back
		Currency::set_balance(Origin::root(), ALICE, KUSD, INITIAL_BALANCE, 0).unwrap();

		assert_noop!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: HDX
				},
				1_000_000_u128,
				2_157_153_u128,
				false,
			),
			Error::<Test>::PoolNotFound
		);

		assert_err!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				1_000_000_000_u128,
				2_157_153_u128,
				false,
			),
			Error::<Test>::MaxOutRatioExceeded
		);

		assert_err!(
			LBPPallet::validate_sell(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				400_000_000_u128,
				2_157_153_u128,
				false,
			),
			Error::<Test>::MaxInRatioExceeded
		);

		assert_err!(
			LBPPallet::validate_sell(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				1_000_u128,
				500_u128,
				false,
			),
			Error::<Test>::TradingLimitReached
		);

		assert_err!(
			LBPPallet::validate_buy(
				&ALICE,
				AssetPair {
					asset_in: KUSD,
					asset_out: BSX
				},
				1_000_u128,
				1_994_u128,
				false,
			),
			Error::<Test>::TradingLimitReached
		);

		Currency::set_balance(Origin::root(), ALICE, KUSD, INITIAL_BALANCE, 0).unwrap();
		Currency::set_balance(Origin::root(), ALICE, HDX, INITIAL_BALANCE, 0).unwrap();

		// transfer fee > token amount in
		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			KUSD,
			2_000_000_000,
			20_000_000,
			80_000_000,
			WeightCurveType::Linear,
			(10, 1),
			CHARLIE,
			0,
		));
		let pool_id2 = LBPPallet::get_pair_id(AssetPair {
			asset_in: KUSD,
			asset_out: HDX,
		});
		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			pool_id2,
			None,
			Some(12),
			Some(20),
			None,
			None,
			None,
			None,
			None
		));
	});
}

#[test]
fn get_sorted_weight_should_work() {
	predefined_test_ext().execute_with(|| {
		let pool = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();

		assert_eq!(
			LBPPallet::get_sorted_weight(KUSD, BlockNumberFor::<Test>::from(10u32), &pool).unwrap(),
			(20_000_000, 80_000_000),
		);

		assert_eq!(
			LBPPallet::get_sorted_weight(BSX, BlockNumberFor::<Test>::from(10u32), &pool).unwrap(),
			(80_000_000, 20_000_000),
		);

		assert_err!(
			LBPPallet::get_sorted_weight(KUSD, BlockNumberFor::<Test>::from(41u32), &pool)
				.map_err(Into::<DispatchError>::into),
			Error::<Test>::InvalidWeight
		);
	});
}

#[test]
fn calculate_fees_should_work() {
	predefined_test_ext().execute_with(|| {
		let pool = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();

		assert_eq!(LBPPallet::calculate_fees(&pool, 1_234_567_890_u128).unwrap(), 2_469_134,);

		assert_eq!(LBPPallet::calculate_fees(&pool, 1000_u128).unwrap(), 2,);

		assert_eq!(LBPPallet::calculate_fees(&pool, 1999_u128).unwrap(), 2,);

		assert_eq!(LBPPallet::calculate_fees(&pool, 999_u128).unwrap(), 0,);

		assert_eq!(
			LBPPallet::calculate_fees(&pool, u128::MAX).unwrap(),
			680564733841876926926749214863536422
		);

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			HDX,
			1_000_000_000,
			BSX,
			2_000_000_000,
			80_000_000,
			20_000_000,
			WeightCurveType::Linear,
			(10, 1),
			CHARLIE,
			0,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			SALE_START,
			Some(20),
			None,
			None,
			None,
			None,
			None,
		));

		let pool = LBPPallet::pool_data(HDX_BSX_POOL_ID).unwrap();

		assert_err!(
			LBPPallet::calculate_fees(&pool, u128::MAX),
			Error::<Test>::FeeAmountInvalid,
		);
	});
}

#[test]
fn can_create_should_work() {
	new_test_ext().execute_with(|| {
		let asset_pair = AssetPair {
			asset_in: KUSD,
			asset_out: BSX,
		};
		// pool doesn't exist
		assert!(DisallowWhenLBPPoolRunning::<Test>::can_create(
			asset_pair.asset_in,
			asset_pair.asset_out
		));

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			ALICE,
			KUSD,
			1_000_000_000,
			BSX,
			2_000_000_000,
			20_000_000,
			80_000_000,
			WeightCurveType::Linear,
			DEFAULT_FEE,
			CHARLIE,
			0,
		));
		// pool is not initialized
		assert!(!DisallowWhenLBPPoolRunning::<Test>::can_create(
			asset_pair.asset_in,
			asset_pair.asset_out
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(ALICE),
			KUSD_BSX_POOL_ID,
			None,
			Some(10),
			Some(20),
			None,
			None,
			None,
			None,
			None,
		));
		// pool is initialized but is not running
		assert!(!DisallowWhenLBPPoolRunning::<Test>::can_create(
			asset_pair.asset_in,
			asset_pair.asset_out
		));

		set_block_number(15);
		// pool is running
		assert!(!DisallowWhenLBPPoolRunning::<Test>::can_create(
			asset_pair.asset_in,
			asset_pair.asset_out
		));

		set_block_number(30);
		// sale ended
		assert!(DisallowWhenLBPPoolRunning::<Test>::can_create(
			asset_pair.asset_in,
			asset_pair.asset_out
		));

		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID,));
		// pool was destroyed
		assert!(DisallowWhenLBPPoolRunning::<Test>::can_create(
			asset_pair.asset_in,
			asset_pair.asset_out
		));
	});
}

#[test]
fn repay_fee_not_applied_when_set_to_zero() {
	new_test_ext().execute_with(|| {
		let pool = Pool {
			repay_target: 0,
			..SAMPLE_POOL_DATA
		};
		assert_eq!(LBPPallet::is_repay_fee_applied(&pool), false);
	});
}

#[test]
fn repay_fee_applied_when_set() {
	new_test_ext().execute_with(|| {
		let pool = Pool {
			repay_target: 10_000_000,
			..SAMPLE_POOL_DATA
		};
		assert_eq!(LBPPallet::is_repay_fee_applied(&pool), true);
	});
}

#[test]
fn repay_fee_not_applied_when_target_reached() {
	new_test_ext().execute_with(|| {
		let pool = Pool {
			fee_collector: ALICE,
			repay_target: INITIAL_BALANCE,
			..SAMPLE_POOL_DATA
		};
		assert_ok!(Currency::set_lock(
			COLLECTOR_LOCK_ID,
			pool.assets.0,
			&ALICE,
			INITIAL_BALANCE
		));
		assert_eq!(LBPPallet::is_repay_fee_applied(&pool), false);
	});
}

#[test]
fn repay_fee_not_applied_in_predefined_env() {
	predefined_test_ext().execute_with(|| {
		let pool = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(LBPPallet::is_repay_fee_applied(&pool), false);
	});
}

#[test]
fn repay_fee_applied_in_env_with_repay_target() {
	predefined_test_ext_with_repay_target().execute_with(|| {
		let pool = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		assert_eq!(LBPPallet::is_repay_fee_applied(&pool), true);
	});
}

#[test]
fn calculate_repay_fee() {
	predefined_test_ext_with_repay_target().execute_with(|| {
		let pool = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();

		assert_eq!(LBPPallet::calculate_fees(&pool, 1000).unwrap(), 200,);
	});
}

#[test]
fn repay_fee_should_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(LBPPallet::repay_fee(), (2, 10));
	});
}

#[test]
fn collected_fees_should_be_locked_and_unlocked_after_liquidity_is_removed() {
	predefined_test_ext().execute_with(|| {
		run_to_sale_start();
		let Pool { fee_collector, .. } = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		let (fee_asset, fee_amount) = SAMPLE_AMM_TRANSFER.fee;
		assert_ok!(LBPPallet::execute_buy(&SAMPLE_AMM_TRANSFER));

		// collector receives locked fee
		assert_eq!(Currency::free_balance(fee_asset, &fee_collector), fee_amount);
		assert_eq!(
			<Test as pallet::Config>::LockedBalance::get_by_lock(COLLECTOR_LOCK_ID, fee_asset, fee_collector),
			fee_amount
		);

		// still locked after sale ends
		run_to_sale_end();
		assert_eq!(
			<Test as pallet::Config>::LockedBalance::get_by_lock(COLLECTOR_LOCK_ID, fee_asset, fee_collector),
			fee_amount
		);

		// unlocked after liquidity is removed from pool
		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(ALICE), KUSD_BSX_POOL_ID));
		assert_eq!(
			<Test as pallet::Config>::LockedBalance::get_by_lock(COLLECTOR_LOCK_ID, fee_asset, fee_collector),
			0
		);
	});
}

#[test]
fn collected_fees_are_continually_locked() {
	predefined_test_ext().execute_with(|| {
		run_to_sale_start();
		let Pool { fee_collector, .. } = LBPPallet::pool_data(KUSD_BSX_POOL_ID).unwrap();
		let (fee_asset, fee_amount) = SAMPLE_AMM_TRANSFER.fee;
		assert_ok!(LBPPallet::execute_buy(&SAMPLE_AMM_TRANSFER));
		assert_ok!(LBPPallet::execute_buy(&SAMPLE_AMM_TRANSFER));
		let total = 2 * fee_amount;
		assert_eq!(Currency::free_balance(fee_asset, &fee_collector), total);
		assert_eq!(
			<Test as pallet::Config>::LockedBalance::get_by_lock(COLLECTOR_LOCK_ID, fee_asset, fee_collector),
			total
		);
	});
}

#[ignore]
#[test]
fn simulate_lbp_event_with_repayment() {
	new_test_ext().execute_with(|| {
		// setup
		let pool_owner = ALICE;
		let lbp_participant = BOB;
		let initial_balance: u128 = 1_000_000_000_000_000_000_000_000;

		let accumulated_asset = BSX;
		let asset_in_pool_reserve: u128 = 1_000_000_000_000;

		let sold_asset = HDX;
		let asset_out_pool_reserve: u128 = 500_000_000_000_000;

		let initial_weight = 90_000_000;
		let final_weight = 30_000_000;

		let sale_start: u64 = 1_000;
		let sale_end: u64 = 22_600; // in blocks; 3 days

		let trades = generate_trades(sale_start, sale_end, 500_000_000_000, 4);

		let fee = (9, 1_000);

		let fee_collector = CHARLIE;

		let trade_limit_factor: u128 = 1_000_000_000;

		// preparations
		let asset_pair = AssetPair {
			asset_in: accumulated_asset,
			asset_out: sold_asset,
		};
		let pool_account = LBPPallet::get_pair_id(asset_pair);

		assert_ok!(LBPPallet::create_pool(
			Origin::root(),
			pool_owner,
			accumulated_asset,
			asset_in_pool_reserve,
			sold_asset,
			asset_out_pool_reserve,
			initial_weight,
			final_weight,
			WeightCurveType::Linear,
			fee,
			fee_collector,
			0,
		));

		assert_ok!(LBPPallet::update_pool_data(
			Origin::signed(pool_owner),
			pool_account,
			None,
			Some(sale_start),
			Some(sale_end),
			None,
			None,
			None,
			None,
			None
		));

		set_block_number(sale_start.checked_sub(1).unwrap());
		//frame_system::Pallet::<Test>::set_block_number(sale_start + 1);

		// start LBP
		for block_num in sale_start..=sale_end {
			set_block_number(block_num);
			println!("{}", LBPPallet::get_spot_price_unchecked(HDX, BSX, 100_000_000_000));
			if let Some((is_buy, amount)) = trades.get(&block_num) {
				if *is_buy {
					assert_ok!(LBPPallet::buy(
						Origin::signed(lbp_participant),
						accumulated_asset,
						sold_asset,
						*amount,
						amount.saturating_mul(trade_limit_factor)
					));
				} else {
					assert_ok!(LBPPallet::sell(
						Origin::signed(lbp_participant),
						accumulated_asset,
						sold_asset,
						*amount,
						amount.checked_div(trade_limit_factor).unwrap()
					));
				}
			}
		}

		// end LBP and consolidate results
		set_block_number(sale_end.checked_add(1).unwrap());

		let pool_account_result_asset_in = Currency::free_balance(accumulated_asset, &pool_account);
		let pool_account_result_asset_out = Currency::free_balance(sold_asset, &pool_account);

		assert_eq!(Currency::free_balance(accumulated_asset, &pool_owner), initial_balance);
		assert_eq!(Currency::free_balance(sold_asset, &pool_owner), initial_balance);

		assert_eq!(Currency::free_balance(accumulated_asset, &pool_account), 4_970_435);
		assert_eq!(Currency::free_balance(sold_asset, &pool_account), 125_000_009);

		assert_eq!(
			Currency::free_balance(accumulated_asset, &lbp_participant),
			999_995_984_267
		);
		assert_eq!(Currency::free_balance(sold_asset, &lbp_participant), 1_000_374_999_991);

		// remove liquidity from the pool
		assert_ok!(LBPPallet::remove_liquidity(Origin::signed(pool_owner), pool_account));

		assert_eq!(Currency::free_balance(accumulated_asset, &pool_account), 0);
		assert_eq!(Currency::free_balance(sold_asset, &pool_account), 0);

		assert_eq!(
			Currency::free_balance(accumulated_asset, &pool_owner),
			initial_balance.checked_add(pool_account_result_asset_in).unwrap()
		);
		assert_eq!(
			Currency::free_balance(sold_asset, &pool_owner),
			initial_balance.checked_add(pool_account_result_asset_out).unwrap()
		);

		assert_eq!(Currency::free_balance(accumulated_asset, &fee_collector), 45_298);
		assert_eq!(Currency::free_balance(sold_asset, &fee_collector), 0);
	});
}
