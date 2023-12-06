// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use pretty_assertions::assert_eq;
use test_ext::*;
use test_utils::assert_transact_ok;

#[test]
fn validate_create_farm_data_should_work() {
	assert_ok!(LiquidityMining::validate_create_global_farm_data(
		1_000_000,
		100,
		1,
		Perquintill::from_percent(50),
		5_000,
		One::one(),
	));

	assert_ok!(LiquidityMining::validate_create_global_farm_data(
		9_999_000_000_000,
		2_000_000,
		500,
		Perquintill::from_percent(100),
		crate::MIN_DEPOSIT,
		One::one(),
	));

	assert_ok!(LiquidityMining::validate_create_global_farm_data(
		10_000_000,
		101,
		16_986_741,
		Perquintill::from_perthousand(1),
		1_000_000_000_000_000,
		One::one(),
	));
}

#[test]
fn validate_create_farm_data_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				999_999,
				100,
				1,
				Perquintill::from_percent(50),
				10_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidTotalRewards
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				9,
				100,
				1,
				Perquintill::from_percent(50),
				1_500,
				One::one()
			),
			Error::<Test, Instance1>::InvalidTotalRewards
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				0,
				100,
				1,
				Perquintill::from_percent(50),
				1_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidTotalRewards
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				1_000_000,
				99,
				1,
				Perquintill::from_percent(50),
				2_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidPlannedYieldingPeriods
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				1_000_000,
				0,
				1,
				Perquintill::from_percent(50),
				3_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidPlannedYieldingPeriods
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				1_000_000,
				87,
				1,
				Perquintill::from_percent(50),
				4_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidPlannedYieldingPeriods
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				1_000_000,
				100,
				0,
				Perquintill::from_percent(50),
				4_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidBlocksPerPeriod
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				1_000_000,
				100,
				10,
				Perquintill::from_percent(0),
				10_000,
				One::one()
			),
			Error::<Test, Instance1>::InvalidYieldPerPeriod
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				10_000_000,
				101,
				16_986_741,
				Perquintill::from_perthousand(1),
				crate::MIN_DEPOSIT - 1,
				One::one()
			),
			Error::<Test, Instance1>::InvalidMinDeposit
		);

		assert_noop!(
			LiquidityMining::validate_create_global_farm_data(
				10_000_000,
				101,
				16_986_741,
				Perquintill::from_perthousand(1),
				10_000,
				Zero::zero()
			),
			Error::<Test, Instance1>::InvalidPriceAdjustment
		);
	});
}
#[test]
fn get_period_number_should_work() {
	let block_num: BlockNumber = 1_u64;
	let blocks_per_period = 1;
	assert_eq!(
		LiquidityMining::get_period_number(block_num, blocks_per_period).unwrap(),
		1
	);

	let block_num: BlockNumber = 1_000_u64;
	let blocks_per_period = 1;
	assert_eq!(
		LiquidityMining::get_period_number(block_num, blocks_per_period).unwrap(),
		1_000
	);

	let block_num: BlockNumber = 23_u64;
	let blocks_per_period = 15;
	assert_eq!(
		LiquidityMining::get_period_number(block_num, blocks_per_period).unwrap(),
		1
	);

	let block_num: BlockNumber = 843_712_398_u64;
	let blocks_per_period = 13_412_341;
	assert_eq!(
		LiquidityMining::get_period_number(block_num, blocks_per_period).unwrap(),
		62
	);

	let block_num: BlockNumber = 843_u64;
	let blocks_per_period = 2_000;
	assert_eq!(
		LiquidityMining::get_period_number(block_num, blocks_per_period).unwrap(),
		0
	);

	let block_num: BlockNumber = 10_u64;
	let blocks_per_period = 10;
	assert_eq!(
		LiquidityMining::get_period_number(block_num, blocks_per_period).unwrap(),
		1
	);
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "Defensive failure has been triggered!"))]
fn get_period_number_should_not_work_when_block_per_period_is_zero() {
	new_test_ext().execute_with(|| {
		let block_num: BlockNumber = 10_u64;
		assert_noop!(
			LiquidityMining::get_period_number(block_num, 0),
			Error::InconsistentState(InconsistentStateError::InvalidPeriod)
		);
	});
}

#[test]
fn get_loyalty_multiplier_should_work() {
	let loyalty_curve_1 = LoyaltyCurve::default();
	let loyalty_curve_2 = LoyaltyCurve {
		initial_reward_percentage: FixedU128::from(1),
		scale_coef: 50,
	};
	let loyalty_curve_3 = LoyaltyCurve {
		initial_reward_percentage: FixedU128::from_inner(123_580_000_000_000_000), // 0.12358
		scale_coef: 23,
	};
	let loyalty_curve_4 = LoyaltyCurve {
		initial_reward_percentage: FixedU128::from_inner(0), // 0.12358
		scale_coef: 15,
	};

	let testing_values = vec![
		(
			0,
			FixedU128::from_float(0.5_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.12358_f64),
			FixedU128::from_float(0_f64),
		),
		(
			1,
			FixedU128::from_float(0.504950495_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.1600975_f64),
			FixedU128::from_float(0.0625_f64),
		),
		(
			4,
			FixedU128::from_float(0.5192307692_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.25342_f64),
			FixedU128::from_float(0.2105263158_f64),
		),
		(
			130,
			FixedU128::from_float(0.7826086957_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.8682505882_f64),
			FixedU128::from_float(0.8965517241_f64),
		),
		(
			150,
			FixedU128::from_float(0.8_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.8834817341_f64),
			FixedU128::from_float(0.9090909091_f64),
		),
		(
			180,
			FixedU128::from_float(0.8214285714_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9007011823_f64),
			FixedU128::from_float(0.9230769231_f64),
		),
		(
			240,
			FixedU128::from_float(0.8529411765_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9233549049_f64),
			FixedU128::from_float(0.9411764706_f64),
		),
		(
			270,
			FixedU128::from_float(0.8648648649_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9312025256_f64),
			FixedU128::from_float(0.9473684211_f64),
		),
		(
			280,
			FixedU128::from_float(0.8684210526_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9334730693_f64),
			FixedU128::from_float(0.9491525424_f64),
		),
		(
			320,
			FixedU128::from_float(0.880952381_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.941231312_f64),
			FixedU128::from_float(0.9552238806_f64),
		),
		(
			380,
			FixedU128::from_float(0.8958333333_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9499809926_f64),
			FixedU128::from_float(0.9620253165_f64),
		),
		(
			390,
			FixedU128::from_float(0.8979591837_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9511921065_f64),
			FixedU128::from_float(0.962962963_f64),
		),
		(
			4000,
			FixedU128::from_float(0.987804878_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.994989396_f64),
			FixedU128::from_float(0.99626401_f64),
		),
		(
			4400,
			FixedU128::from_float(0.9888888889_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.9954425367_f64),
			FixedU128::from_float(0.9966024915_f64),
		),
		(
			4700,
			FixedU128::from_float(0.9895833333_f64),
			FixedU128::from_float(1_f64),
			FixedU128::from_float(0.995732022_f64),
			FixedU128::from_float(0.9968186638_f64),
		),
	];

	//Special case: loyalty curve is None
	assert_eq!(
		LiquidityMining::get_loyalty_multiplier(10, None).unwrap(),
		FixedU128::one()
	);

	let precission_delta = FixedU128::from_inner(100_000_000); //0.000_000_000_1
	for (periods, expected_multiplier_1, expected_multiplier_2, expected_multiplier_3, expected_multiplier_4) in
		testing_values.iter()
	{
		//1-th curve test
		assert!(is_approx_eq_fixedu128(
			LiquidityMining::get_loyalty_multiplier(*periods, Some(loyalty_curve_1.clone())).unwrap(),
			*expected_multiplier_1,
			precission_delta
		));

		//2-nd curve test
		assert!(is_approx_eq_fixedu128(
			LiquidityMining::get_loyalty_multiplier(*periods, Some(loyalty_curve_2.clone())).unwrap(),
			*expected_multiplier_2,
			precission_delta
		));

		//3-rd curve test
		assert!(is_approx_eq_fixedu128(
			LiquidityMining::get_loyalty_multiplier(*periods, Some(loyalty_curve_3.clone())).unwrap(),
			*expected_multiplier_3,
			precission_delta
		));

		//-4th curve test
		assert!(is_approx_eq_fixedu128(
			LiquidityMining::get_loyalty_multiplier(*periods, Some(loyalty_curve_4.clone())).unwrap(),
			*expected_multiplier_4,
			precission_delta
		));
	}
}

#[test]
fn sync_global_farm_should_work() {
	let testing_values = vec![
		(
			26_u64,
			2501944769_u128,
			FixedU128::from_float(259.000000_f64),
			HDX,
			ACA_FARM,
			0_u128,
			206_u64,
			55563662_u128,
			FixedU128::from_float(259.000000000_f64),
			55563662_u128,
		),
		(
			188_u64,
			33769604_u128,
			FixedU128::from_float(1148.000000_f64),
			BSX,
			ACA_FARM,
			30080406306_u128,
			259_u64,
			56710169_u128,
			FixedU128::from_inner(1_183_500_000_000_000_000_000_u128),
			1255531111_u128,
		),
		(
			195_u64,
			26098384286056_u128,
			FixedU128::from_float(523.000000_f64),
			ACA,
			ACA_FARM,
			32055_u128,
			326_u64,
			61424428_u128,
			FixedU128::from_inner(523_000_000_001_189_920_405),
			61455483_u128,
		),
		(
			181_u64,
			9894090144_u128,
			FixedU128::from_float(317.000000_f64),
			KSM,
			ACA_FARM,
			36806694280_u128,
			1856_u64,
			52711084_u128,
			FixedU128::from_inner(320_720_068_520_127_685_628_u128),
			36859404364_u128,
		),
		(
			196_u64,
			26886423482043_u128,
			FixedU128::from_float(596.000000_f64),
			ACA,
			BSX_FARM,
			30560755872_u128,
			954_u64,
			34013971_u128,
			FixedU128::from_inner(596_001_136_661_218_343_563_u128),
			30594768843_u128,
		),
		(
			68_u64,
			1138057342_u128,
			FixedU128::from_float(4.000000_f64),
			ACA,
			ACA_FARM,
			38398062768_u128,
			161_u64,
			71071995_u128,
			FixedU128::from_inner(37_740_006_193_817_956_143_u128),
			38469133763_u128,
		),
		(
			161_u64,
			24495534649923_u128,
			FixedU128::from_float(213.000000_f64),
			KSM,
			BSX_FARM,
			11116735745_u128,
			448_u64,
			85963452_u128,
			FixedU128::from_inner(213_000_453_826_989_444_173_u128),
			11202698197_u128,
		),
		(
			27_u64,
			22108444_u128,
			FixedU128::from_float(970.000000_f64),
			KSM,
			BSX_FARM,
			8572779460_u128,
			132_u64,
			43974403_u128,
			FixedU128::from_float(1022.500000000_f64),
			1204667713_u128,
		),
		(
			97_u64,
			1593208_u128,
			FixedU128::from_float(6.000000_f64),
			HDX,
			BSX_FARM,
			18440792496_u128,
			146_u64,
			14437690_u128,
			FixedU128::from_float(30.500000000_f64),
			53471286_u128,
		),
		(
			154_u64,
			27279119649838_u128,
			FixedU128::from_float(713.000000_f64),
			BSX,
			KSM_FARM,
			28318566664_u128,
			202_u64,
			7533987_u128,
			FixedU128::from_inner(713_001_038_104_089_409_944_u128),
			28326099651_u128,
		),
		(
			104_u64,
			20462312838954_u128,
			FixedU128::from_float(833.000000_f64),
			BSX,
			BSX_FARM,
			3852003_u128,
			131_u64,
			75149021_u128,
			FixedU128::from_inner(833_000_000_188_199_791_016_u128),
			79000024_u128,
		),
		(
			90_u64,
			37650830596054_u128,
			FixedU128::from_float(586.000000_f64),
			HDX,
			KSM_FARM,
			27990338179_u128,
			110_u64,
			36765518_u128,
			FixedU128::from_inner(586_000_743_418_849_886_767_u128),
			28027102697_u128,
		),
		(
			198_u64,
			318777214_u128,
			FixedU128::from_float(251.000000_f64),
			ACA,
			ACA_FARM,
			3615346492_u128,
			582_u64,
			12876432_u128,
			FixedU128::from_inner(262_341_292_078_674_104_981_u128),
			3628221924_u128,
		),
		(
			29_u64,
			33478250_u128,
			FixedU128::from_float(77.000000_f64),
			BSX,
			ACA_FARM,
			39174031245_u128,
			100_u64,
			26611087_u128,
			FixedU128::from_float(112.500000000_f64),
			1215088962_u128,
		),
		(
			91_u64,
			393922835172_u128,
			FixedU128::from_float(2491.000000_f64),
			ACA,
			KSM_FARM,
			63486975129400_u128,
			260_u64,
			85100506_u128,
			FixedU128::from_inner(2_575_500_000_000_000_262_144_u128),
			33286564672540_u128,
		),
		(
			67_u64,
			1126422_u128,
			FixedU128::from_float(295.000000_f64),
			HDX,
			BSX_FARM,
			7492177402_u128,
			229_u64,
			35844776_u128,
			FixedU128::from_float(376.000000000_f64),
			127084958_u128,
		),
		(
			168_u64,
			28351324279041_u128,
			FixedU128::from_float(450.000000_f64),
			ACA,
			KSM_FARM,
			38796364068_u128,
			361_u64,
			35695723_u128,
			FixedU128::from_inner(450_001_368_414_494_016_443_u128),
			38832058791_u128,
		),
		(
			3_u64,
			17631376575792_u128,
			FixedU128::from_float(82.000000_f64),
			HDX,
			KSM_FARM,
			20473946880_u128,
			52_u64,
			93293564_u128,
			FixedU128::from_inner(82_001_161_222_199_071_561_u128),
			20567239444_u128,
		),
		(
			49_u64,
			94060_u128,
			FixedU128::from_float(81.000000_f64),
			HDX,
			ACA_FARM,
			11126653978_u128,
			132_u64,
			75841904_u128,
			FixedU128::from_float(122.500000000_f64),
			79745394_u128,
		),
		(
			38_u64,
			14086_u128,
			FixedU128::from_float(266.000000_f64),
			KSM,
			BSX_FARM,
			36115448964_u128,
			400000_u64,
			52402278_u128,
			FixedU128::from_inner(200_247_000_000_000_000_000_000_u128),
			2869334644_u128,
		),
		(
			158_u64,
			762784_u128,
			FixedU128::from_float(129.000000_f64),
			BSX,
			ACA_FARM,
			21814882774_u128,
			158_u64,
			86085676_u128,
			FixedU128::from_float(129.000000000_f64),
			86085676_u128,
		),
	];

	for (
		updated_at,
		total_shares_z,
		accumulated_rpz,
		reward_currency,
		id,
		rewards_left_to_distribute,
		current_period,
		accumulated_rewards,
		expected_accumulated_rpz,
		expected_pending_rewards,
	) in testing_values.iter()
	{
		let yield_per_period = Perquintill::from_percent(50);
		let planned_yielding_periods = 100;
		let blocks_per_period = 0;
		let owner = ALICE;
		let incentivized_asset = BSX;
		let max_reward_per_period = 10_000 * ONE;

		let global_farm_0 = GlobalFarmData {
			id: *id,
			owner,
			updated_at: *updated_at,
			total_shares_z: *total_shares_z,
			accumulated_rpz: *accumulated_rpz,
			reward_currency: *reward_currency,
			pending_rewards: *accumulated_rewards,
			accumulated_paid_rewards: 10,
			yield_per_period,
			planned_yielding_periods,
			blocks_per_period,
			max_reward_per_period,
			incentivized_asset,
			min_deposit: crate::MIN_DEPOSIT,
			live_yield_farms_count: Default::default(),
			total_yield_farms_count: Default::default(),
			price_adjustment: FixedU128::one(),
			state: FarmState::Active,
		};

		let mut global_farm = global_farm_0.clone();

		new_test_ext().execute_with(|| {
			//Add farm's account to whitelist
			let farm_account_id = LiquidityMining::farm_account_id(*id).unwrap();
			Whitelist::add_account(&farm_account_id).unwrap();

			Tokens::transfer(
				Origin::signed(TREASURY),
				farm_account_id,
				*reward_currency,
				*rewards_left_to_distribute,
			)
			.unwrap();

			assert_eq!(
				Tokens::free_balance(*reward_currency, &farm_account_id),
				*rewards_left_to_distribute
			);

			let r = with_transaction(|| {
				TransactionOutcome::Commit(LiquidityMining::sync_global_farm(&mut global_farm, *current_period))
			})
			.unwrap();

			if r.is_zero() && updated_at != current_period {
				frame_system::Pallet::<Test>::assert_has_event(mock::RuntimeEvent::LiquidityMining(
					Event::AllRewardsDistributed { global_farm_id: *id },
				));
			}

			let expected_global_farm = GlobalFarmData {
				accumulated_rpz: *expected_accumulated_rpz,
				pending_rewards: *expected_pending_rewards,
				updated_at: *current_period,
				..global_farm_0.clone()
			};

			assert_eq!(global_farm, expected_global_farm);

			if updated_at != current_period {
				frame_system::Pallet::<Test>::assert_has_event(mock::RuntimeEvent::LiquidityMining(
					Event::GlobalFarmAccRPZUpdated {
						global_farm_id: *id,
						accumulated_rpz: *expected_accumulated_rpz,
						total_shares_z: *total_shares_z,
					},
				));
			}
		});
	}
}

#[test]
fn sync_global_farm_should_not_update_farm_when_farm_is_not_active() {
	{
		let global_farm_0 = GlobalFarmData {
			id: 1,
			owner: ALICE,
			updated_at: 100,
			total_shares_z: 1_000_000 * ONE,
			accumulated_rpz: FixedU128::from(5),
			reward_currency: BSX,
			pending_rewards: 1_000 * ONE,
			accumulated_paid_rewards: 10,
			yield_per_period: Perquintill::from_float(0.5_f64),
			planned_yielding_periods: 1_000,
			blocks_per_period: 1,
			max_reward_per_period: 10_000 * ONE,
			incentivized_asset: BSX,
			min_deposit: crate::MIN_DEPOSIT,
			live_yield_farms_count: Default::default(),
			total_yield_farms_count: Default::default(),
			price_adjustment: FixedU128::one(),
			state: FarmState::Terminated,
		};

		let mut global_farm = global_farm_0.clone();

		new_test_ext().execute_with(|| {
			let current_period = global_farm_0.updated_at + 100;
			let r = with_transaction(|| {
				TransactionOutcome::Commit(LiquidityMining::sync_global_farm(&mut global_farm, current_period))
			})
			.unwrap();

			assert_eq!(r, 0);
			assert_eq!(global_farm, global_farm_0);
		});
	}
}

#[test]
fn sync_global_farm_should_only_update_updated_at_field_when_farm_has_no_shares() {
	{
		let global_farm_1 = GlobalFarmData {
			id: 1,
			owner: ALICE,
			updated_at: 200,
			total_shares_z: Balance::zero(),
			accumulated_rpz: FixedU128::from(5),
			reward_currency: BSX,
			pending_rewards: 1_000 * ONE,
			accumulated_paid_rewards: 10,
			yield_per_period: Perquintill::from_float(0.5_f64),
			planned_yielding_periods: 1_000,
			blocks_per_period: 1,
			max_reward_per_period: 10_000 * ONE,
			incentivized_asset: BSX,
			min_deposit: crate::MIN_DEPOSIT,
			live_yield_farms_count: Default::default(),
			total_yield_farms_count: Default::default(),
			price_adjustment: FixedU128::one(),
			state: FarmState::Active,
		};

		let mut global_farm = global_farm_1.clone();

		new_test_ext().execute_with(|| {
			let current_period = 200;
			let r = with_transaction(|| {
				TransactionOutcome::Commit(LiquidityMining::sync_global_farm(&mut global_farm, current_period))
			})
			.unwrap();

			assert_eq!(r, 0);
			assert_eq!(global_farm, global_farm_1);
		});
	}
}

#[test]
fn sync_global_farm_should_not_update_farm_when_farm_was_already_updated_in_this_period() {
	{
		let global_farm_0 = GlobalFarmData {
			id: 1,
			owner: ALICE,
			updated_at: 100,
			total_shares_z: 1_000_000 * ONE,
			accumulated_rpz: FixedU128::from(5),
			reward_currency: BSX,
			pending_rewards: 1_000 * ONE,
			accumulated_paid_rewards: 10,
			yield_per_period: Perquintill::from_float(0.5_f64),
			planned_yielding_periods: 1_000,
			blocks_per_period: 1,
			max_reward_per_period: 10_000 * ONE,
			incentivized_asset: BSX,
			min_deposit: crate::MIN_DEPOSIT,
			live_yield_farms_count: Default::default(),
			total_yield_farms_count: Default::default(),
			price_adjustment: FixedU128::one(),
			state: FarmState::Active,
		};

		let mut global_farm = global_farm_0.clone();

		new_test_ext().execute_with(|| {
			let current_period = global_farm_0.updated_at;
			let r = with_transaction(|| {
				TransactionOutcome::Commit(LiquidityMining::sync_global_farm(&mut global_farm, current_period))
			})
			.unwrap();

			assert_eq!(r, 0);
			assert_eq!(global_farm, global_farm_0);
		});
	}
}

#[test]
fn sync_yield_farm_should_work() {
	let testing_values = vec![
		(
			BSX_FARM,
			BSX_DOT_YIELD_FARM_ID,
			26_u64,
			206_u64,
			299_u128,
			0_u128,
			387_u128,
			BSX,
			299_u128,
			206_u64,
			0_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			188_u64,
			259_u64,
			1151_u128,
			33769603_u128,
			1225_u128,
			BSX,
			1299_u128,
			259_u64,
			4997901244_u128,
		),
		(
			BSX_FARM,
			BSX_KSM_YIELD_FARM_ID,
			195_u64,
			326_u64,
			823_u128,
			2604286056_u128,
			971_u128,
			BSX,
			1119_u128,
			326_u64,
			770868672576_u128,
		),
		(
			BSX_FARM,
			BSX_KSM_YIELD_FARM_ID,
			181_u64,
			1856_u64,
			320_u128,
			8940144_u128,
			398_u128,
			BSX,
			476_u128,
			1856_u64,
			1394662464_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			196_u64,
			954_u64,
			5684_u128,
			282043_u128,
			5758_u128,
			BSX,
			5832_u128,
			954_u64,
			41742364_u128,
		),
		(
			BSX_FARM,
			BSX_DOT_YIELD_FARM_ID,
			68_u64,
			161_u64,
			37_u128,
			1138057342_u128,
			126_u128,
			BSX,
			215_u128,
			161_u64,
			202574206876_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			161_u64,
			448_u64,
			678_u128,
			49923_u128,
			845_u128,
			BSX,
			1012_u128,
			448_u64,
			16674282_u128,
		),
		(
			BSX_FARM,
			BSX_DOT_YIELD_FARM_ID,
			27_u64,
			132_u64,
			978_u128,
			2444_u128,
			1135_u128,
			BSX,
			1292_u128,
			132_u64,
			767416_u128,
		),
		(
			BSX_FARM,
			BSX_KSM_YIELD_FARM_ID,
			97_u64,
			146_u64,
			28_u128,
			1593208_u128,
			205_u128,
			BSX,
			382_u128,
			146_u64,
			563995632_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			154_u64,
			202_u64,
			876_u128,
			9838_u128,
			888_u128,
			BSX,
			900_u128,
			202_u64,
			236112_u128,
		),
		(
			BSX_FARM,
			BSX_DOT_YIELD_FARM_ID,
			104_u64,
			131_u64,
			8373_u128,
			2046838954_u128,
			8412_u128,
			BSX,
			8451_u128,
			131_u64,
			159653438412_u128,
		),
		(
			BSX_FARM,
			BSX_KSM_YIELD_FARM_ID,
			90_u64,
			110_u64,
			5886_u128,
			596054_u128,
			6010_u128,
			BSX,
			6134_u128,
			110_u64,
			147821392_u128,
		),
		(
			BSX_FARM,
			BSX_DOT_YIELD_FARM_ID,
			198_u64,
			582_u64,
			2591_u128,
			377215_u128,
			2781_u128,
			BSX,
			2971_u128,
			582_u64,
			143341700_u128,
		),
		(
			BSX_FARM,
			BSX_KSM_YIELD_FARM_ID,
			29_u64,
			100_u64,
			80_u128,
			8250_u128,
			257_u128,
			BSX,
			434_u128,
			100_u64,
			2920500_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			91_u64,
			260_u64,
			2537_u128,
			35172_u128,
			2556_u128,
			BSX,
			2575_u128,
			260_u64,
			1336536_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			67_u64,
			229_u64,
			471_u128,
			1126422_u128,
			579_u128,
			BSX,
			687_u128,
			229_u64,
			243307152_u128,
		),
		(
			BSX_FARM,
			BSX_DOT_YIELD_FARM_ID,
			168_u64,
			361_u64,
			952_u128,
			28279041_u128,
			971_u128,
			BSX,
			990_u128,
			361_u64,
			1074603558_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			3_u64,
			52_u64,
			357_u128,
			2_u128,
			518_u128,
			BSX,
			679_u128,
			52_u64,
			644_u128,
		),
		(
			BSX_FARM,
			BSX_KSM_YIELD_FARM_ID,
			49_u64,
			132_u64,
			1557_u128,
			94059_u128,
			1651_u128,
			BSX,
			1745_u128,
			132_u64,
			17683092_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			38_u64,
			38_u64,
			2564373_u128,
			14085_u128,
			2564404_u128,
			BSX,
			2564373_u128,
			38_u64,
			0_u128,
		),
		(
			BSX_FARM,
			BSX_ACA_YIELD_FARM_ID,
			158_u64,
			158_u64,
			129_u128,
			762784_u128,
			286_u128,
			BSX,
			129_u128,
			158_u64,
			0_u128,
		),
	];

	for (
		global_farm_id,
		yield_farm_id,
		yield_farm_updated_at,
		current_period,
		yield_farm_accumulated_rpvs,
		yield_farm_total_valued_shares,
		global_farm_accumulated_rpz,
		reward_currency,
		expected_yield_farm_accumulated_rpvs,
		expected_updated_at,
		expected_yield_farm_reward_currency_balance,
	) in testing_values
	{
		let owner = ALICE;
		let yield_per_period = Perquintill::from_percent(50);
		let blocks_per_period = BlockNumber::from(1_u32);
		let planned_yielding_periods = 100;
		let incentivized_asset = BSX;
		let updated_at = 200_u64;
		let max_reward_per_period = Balance::from(10_000_u32);

		let global_farm_0 = GlobalFarmData {
			id: global_farm_id,
			owner,
			updated_at,
			total_shares_z: 1_000_000_u128,
			accumulated_rpz: FixedU128::from(global_farm_accumulated_rpz),
			reward_currency,
			pending_rewards: 1_000_000 * ONE,
			accumulated_paid_rewards: 1_000_000 * ONE,
			yield_per_period,
			planned_yielding_periods,
			blocks_per_period,
			max_reward_per_period,
			incentivized_asset,
			min_deposit: crate::MIN_DEPOSIT,
			live_yield_farms_count: Default::default(),
			total_yield_farms_count: Default::default(),
			price_adjustment: FixedU128::one(),
			state: FarmState::Active,
		};

		let yield_farm_0 = YieldFarmData {
			id: yield_farm_id,
			updated_at: yield_farm_updated_at,
			total_shares: 200_u128,
			total_valued_shares: yield_farm_total_valued_shares,
			accumulated_rpvs: FixedU128::from(yield_farm_accumulated_rpvs),
			accumulated_rpz: FixedU128::from(yield_farm_accumulated_rpvs),
			loyalty_curve: None,
			multiplier: FixedU128::from(2_u128),
			state: FarmState::Active,
			entries_count: 0,
			left_to_distribute: 0,
			total_stopped: 0,
			_phantom: PhantomData,
		};

		let mut global_farm = global_farm_0.clone();
		let mut yield_farm = yield_farm_0.clone();

		let global_farm_account_id = LiquidityMining::farm_account_id(global_farm_id).unwrap();
		let pot_account_id = LiquidityMining::pot_account_id().unwrap();

		new_test_ext().execute_with(|| {
			//Arrange
			let _ = Tokens::transfer(
				Origin::signed(TREASURY),
				global_farm_account_id,
				global_farm.reward_currency,
				9_000_000_000_000,
			);
			assert_eq!(
				Tokens::free_balance(global_farm.reward_currency, &global_farm_account_id),
				9_000_000_000_000_u128
			);

			//_0 - value before action
			let pot_balance_0 = 9_000_000_000_000;
			let _ = Tokens::transfer(
				Origin::signed(TREASURY),
				pot_account_id,
				global_farm.reward_currency,
				pot_balance_0,
			);

			//Act
			assert_transact_ok!(LiquidityMining::sync_yield_farm(
				&mut yield_farm,
				&mut global_farm,
				current_period,
			));

			//Assert
			//
			//NOTE: update in the same period should happen only if farm is empty. RPVS is used as starting value
			//for yield-farm's rpz in this test.
			let rpz = if current_period == yield_farm_updated_at {
				yield_farm_accumulated_rpvs
			} else {
				global_farm_accumulated_rpz
			};
			assert_eq!(
				global_farm,
				GlobalFarmData {
					updated_at: 200_u64,
					pending_rewards: global_farm_0.pending_rewards - expected_yield_farm_reward_currency_balance,
					accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards
						+ expected_yield_farm_reward_currency_balance,
					..global_farm_0.clone()
				}
			);

			assert_eq!(
				yield_farm,
				YieldFarmData {
					updated_at: expected_updated_at,
					accumulated_rpvs: FixedU128::from(expected_yield_farm_accumulated_rpvs),
					accumulated_rpz: rpz.into(),
					left_to_distribute: expected_yield_farm_reward_currency_balance,
					..yield_farm_0.clone()
				}
			);

			//yield-farm's rewards are left in the pot so its balance should not change
			assert_eq!(
				Tokens::free_balance(global_farm.reward_currency, &pot_account_id),
				pot_balance_0
			);

			if current_period != yield_farm_updated_at && !yield_farm_total_valued_shares.is_zero() {
				frame_system::Pallet::<Test>::assert_has_event(mock::RuntimeEvent::LiquidityMining(
					Event::YieldFarmAccRPVSUpdated {
						global_farm_id: global_farm_0.id,
						yield_farm_id: yield_farm_0.id,
						accumulated_rpvs: FixedU128::from(expected_yield_farm_accumulated_rpvs),
						total_valued_shares: yield_farm_0.total_valued_shares,
					},
				));
			}
		});
	}
}

#[test]
fn sync_yield_farm_should_not_update_when_yield_farm_is_not_active() {
	let global_farm_0 = GlobalFarmData {
		id: 1,
		owner: ALICE,
		updated_at: 1000,
		total_shares_z: 1_000_000_u128,
		accumulated_rpz: FixedU128::from(10),
		reward_currency: BSX,
		pending_rewards: 1_000_000 * ONE,
		accumulated_paid_rewards: 1_000_000 * ONE,
		yield_per_period: Perquintill::from_float(0.5_f64),
		planned_yielding_periods: 1_000,
		blocks_per_period: 1,
		max_reward_per_period: 10_000 * ONE,
		incentivized_asset: BSX,
		min_deposit: crate::MIN_DEPOSIT,
		live_yield_farms_count: Default::default(),
		total_yield_farms_count: Default::default(),
		price_adjustment: FixedU128::one(),
		state: FarmState::Active,
	};

	let yield_farm_0 = YieldFarmData {
		id: 2,
		updated_at: 50,
		total_shares: 200_u128,
		total_valued_shares: 10_000 * ONE,
		accumulated_rpvs: FixedU128::from(3),
		accumulated_rpz: FixedU128::from(4),
		loyalty_curve: None,
		multiplier: FixedU128::from(2_u128),
		state: FarmState::Active,
		entries_count: 0,
		left_to_distribute: 0,
		total_stopped: 0,
		_phantom: PhantomData,
	};

	let mut global_farm = global_farm_0.clone();
	let mut stopped_yield_farm = yield_farm_0.clone();
	stopped_yield_farm.state = FarmState::Stopped;
	let mut terminated_yield_farm = yield_farm_0.clone();
	terminated_yield_farm.state = FarmState::Terminated;

	new_test_ext().execute_with(|| {
		let current_period = yield_farm_0.updated_at + global_farm_0.updated_at;
		//Stopped yield-farm
		assert_transact_ok!(LiquidityMining::sync_yield_farm(
			&mut stopped_yield_farm,
			&mut global_farm,
			current_period,
		));

		assert_eq!(
			stopped_yield_farm,
			YieldFarmData {
				state: FarmState::Stopped,
				..yield_farm_0.clone()
			}
		);
		assert_eq!(global_farm, global_farm_0);

		//Terminated yield-farm
		assert_transact_ok!(LiquidityMining::sync_yield_farm(
			&mut terminated_yield_farm,
			&mut global_farm,
			current_period,
		));

		assert_eq!(
			terminated_yield_farm,
			YieldFarmData {
				state: FarmState::Terminated,
				..yield_farm_0
			}
		);
		assert_eq!(global_farm, global_farm_0);
	});
}

#[test]

fn sync_yield_farm_should_only_update_updated_at_field_when_farm_has_no_valued_shares() {
	let global_farm_0 = GlobalFarmData {
		id: 1,
		owner: ALICE,
		updated_at: 1000,
		total_shares_z: 1_000_000_u128,
		accumulated_rpz: FixedU128::from(10),
		reward_currency: BSX,
		pending_rewards: 1_000_000 * ONE,
		accumulated_paid_rewards: 1_000_000 * ONE,
		yield_per_period: Perquintill::from_float(0.5_f64),
		planned_yielding_periods: 1_000,
		blocks_per_period: 1,
		max_reward_per_period: 10_000 * ONE,
		incentivized_asset: BSX,
		min_deposit: crate::MIN_DEPOSIT,
		live_yield_farms_count: Default::default(),
		total_yield_farms_count: Default::default(),
		price_adjustment: FixedU128::one(),
		state: FarmState::Active,
	};

	//after action
	let yield_farm_1 = YieldFarmData {
		id: 2,
		updated_at: 1050,
		total_shares: 0,
		total_valued_shares: 0,
		accumulated_rpvs: FixedU128::from(3),
		accumulated_rpz: FixedU128::from(4),
		loyalty_curve: None,
		multiplier: FixedU128::from(2_u128),
		state: FarmState::Active,
		entries_count: 0,
		left_to_distribute: 0,
		total_stopped: 0,
		_phantom: PhantomData,
	};

	let mut global_farm = global_farm_0.clone();
	let mut yield_farm = yield_farm_1.clone();

	new_test_ext().execute_with(|| {
		let current_period = 1050;
		assert_transact_ok!(LiquidityMining::sync_yield_farm(
			&mut yield_farm,
			&mut global_farm,
			current_period,
		));

		assert_eq!(yield_farm, yield_farm_1);
		assert_eq!(global_farm, global_farm_0);
	});
}

#[test]
fn sync_yield_farm_should_not_update_when_yield_farm_was_already_updated_in_this_period() {
	let global_farm_0 = GlobalFarmData {
		id: 1,
		owner: ALICE,
		updated_at: 1000,
		total_shares_z: 1_000_000_u128,
		accumulated_rpz: FixedU128::from(10),
		reward_currency: BSX,
		pending_rewards: 1_000_000 * ONE,
		accumulated_paid_rewards: 1_000_000 * ONE,
		yield_per_period: Perquintill::from_float(0.5_f64),
		planned_yielding_periods: 1_000,
		blocks_per_period: 1,
		max_reward_per_period: 10_000 * ONE,
		incentivized_asset: BSX,
		min_deposit: crate::MIN_DEPOSIT,
		live_yield_farms_count: Default::default(),
		total_yield_farms_count: Default::default(),
		price_adjustment: FixedU128::one(),
		state: FarmState::Active,
	};

	let yield_farm_0 = YieldFarmData {
		id: 2,
		updated_at: global_farm_0.updated_at,
		total_shares: 10_000 * ONE,
		total_valued_shares: 15_000 * ONE,
		accumulated_rpvs: FixedU128::from(3),
		accumulated_rpz: FixedU128::from(4),
		loyalty_curve: None,
		multiplier: FixedU128::from(2_u128),
		state: FarmState::Active,
		entries_count: 0,
		left_to_distribute: 0,
		total_stopped: 0,
		_phantom: PhantomData,
	};

	let mut global_farm = global_farm_0.clone();
	let mut yield_farm = yield_farm_0.clone();

	new_test_ext().execute_with(|| {
		let current_period = global_farm_0.updated_at;
		assert_transact_ok!(LiquidityMining::sync_yield_farm(
			&mut yield_farm,
			&mut global_farm,
			current_period,
		));

		assert_eq!(yield_farm, yield_farm_0);
		assert_eq!(global_farm, global_farm_0);
	});
}

#[test]
fn get_next_farm_id_should_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(LiquidityMining::get_next_farm_id().unwrap(), 1);
		assert_eq!(LiquidityMining::last_farm_id(), 1);

		assert_eq!(LiquidityMining::get_next_farm_id().unwrap(), 2);
		assert_eq!(LiquidityMining::last_farm_id(), 2);

		assert_eq!(LiquidityMining::get_next_farm_id().unwrap(), 3);
		assert_eq!(LiquidityMining::last_farm_id(), 3);

		assert_eq!(LiquidityMining::get_next_farm_id().unwrap(), 4);
		assert_eq!(LiquidityMining::last_farm_id(), 4);
	});
}

#[test]
fn farm_account_id_should_work() {
	let ids: Vec<FarmId> = vec![1, 100, 543, u32::max_value()];

	for id in ids {
		assert_ok!(LiquidityMining::farm_account_id(id));
	}
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "Defensive failure has been triggered!"))]
fn farm_account_id_should_fail_when_farm_id_is_zero() {
	let ids: Vec<FarmId> = vec![0];
	new_test_ext().execute_with(|| {
		for id in ids {
			assert_noop!(
				LiquidityMining::farm_account_id(id),
				Error::<Test, Instance1>::InconsistentState(InconsistentStateError::InvalidFarmId)
			);
		}
	});
}

#[test]
fn get_next_deposit_id_should_work() {
	new_test_ext().execute_with(|| {
		let test_data = vec![1, 2, 3, 4, 5];

		for expected_deposit_id in test_data {
			assert_eq!(LiquidityMining::get_next_deposit_id().unwrap(), expected_deposit_id);
		}
	});
}

#[test]
fn depositdata_add_farm_entry_to_should_work() {
	new_test_ext().execute_with(|| {
		let mut deposit = DepositData::<Test, Instance1> {
			shares: 10,
			amm_pool_id: BSX_TKN1_AMM,
			yield_farm_entries: vec![].try_into().unwrap(),
		};

		let test_farm_entries = vec![
			YieldFarmEntry::<Test, Instance1>::new(1, 50, 20, FixedU128::from(12), 2, 1),
			YieldFarmEntry::<Test, Instance1>::new(2, 18, 20, FixedU128::from(14), 18, 1),
			YieldFarmEntry::<Test, Instance1>::new(3, 60, 20, FixedU128::from(1), 1, 1),
			YieldFarmEntry::<Test, Instance1>::new(4, 1, 20, FixedU128::from(10), 13, 1),
			YieldFarmEntry::<Test, Instance1>::new(7, 2, 20, FixedU128::from(10), 13, 1),
			YieldFarmEntry::<Test, Instance1>::new(5, 100, 20, FixedU128::from(10), 13, 1),
			YieldFarmEntry::<Test, Instance1>::new(6, 4, 20, FixedU128::from(10), 13, 1),
		];

		assert_ok!(deposit.add_yield_farm_entry(test_farm_entries[0].clone()));

		assert_ok!(deposit.add_yield_farm_entry(test_farm_entries[2].clone()));

		assert_ok!(deposit.add_yield_farm_entry(test_farm_entries[3].clone()));

		//`yield_farm_id` must be unique in `yield_farm_entries`
		assert_noop!(
			deposit.add_yield_farm_entry(test_farm_entries[2].clone()),
			Error::<Test, Instance1>::DoubleLock
		);
		assert_noop!(
			deposit.add_yield_farm_entry(YieldFarmEntry::<Test, Instance1>::new(
				1,
				50,
				10,
				FixedU128::from(1),
				1,
				0
			)),
			Error::<Test, Instance1>::DoubleLock
		);

		assert_ok!(deposit.add_yield_farm_entry(test_farm_entries[4].clone()));

		assert_ok!(deposit.add_yield_farm_entry(test_farm_entries[6].clone()));

		assert_eq!(
			deposit,
			DepositData::<Test, Instance1> {
				shares: 10,
				amm_pool_id: BSX_TKN1_AMM,
				yield_farm_entries: vec![
					test_farm_entries[0].clone(),
					test_farm_entries[2].clone(),
					test_farm_entries[3].clone(),
					test_farm_entries[4].clone(),
					test_farm_entries[6].clone(),
				]
				.try_into()
				.unwrap(),
			}
		);

		//5 is max farm entries.
		assert_noop!(
			deposit.add_yield_farm_entry(test_farm_entries[5].clone()),
			Error::<Test, Instance1>::MaxEntriesPerDeposit
		);
	});
}

#[test]
fn deposit_remove_yield_farm_entry_should_work() {
	new_test_ext().execute_with(|| {
		let mut deposit = DepositData::<Test, Instance1> {
			shares: 10,
			amm_pool_id: BSX_TKN1_AMM,
			yield_farm_entries: vec![
				YieldFarmEntry::<Test, Instance1>::new(4, 1, 20, FixedU128::from(10), 13, 0),
				YieldFarmEntry::<Test, Instance1>::new(7, 2, 20, FixedU128::from(1), 13, 0),
				YieldFarmEntry::<Test, Instance1>::new(6, 4, 20, FixedU128::from(10), 13, 0),
				YieldFarmEntry::<Test, Instance1>::new(2, 18, 20, FixedU128::from(14), 18, 0),
				YieldFarmEntry::<Test, Instance1>::new(3, 60, 20, FixedU128::from(1), 1, 0),
			]
			.try_into()
			.unwrap(),
		};

		const NON_EXISTING_YIELD_FARM_ID: YieldFarmId = 999_999_999;
		assert_noop!(
			deposit.remove_yield_farm_entry(NON_EXISTING_YIELD_FARM_ID),
			Error::<Test, Instance1>::YieldFarmEntryNotFound
		);

		assert_ok!(deposit.remove_yield_farm_entry(2));
		assert_ok!(deposit.remove_yield_farm_entry(18));
		assert_ok!(deposit.remove_yield_farm_entry(1));
		assert_ok!(deposit.remove_yield_farm_entry(4));
		assert_ok!(deposit.remove_yield_farm_entry(60));

		//This state should never happen, deposit should be flushed from storage when have no more
		//entries.
		assert_eq!(
			deposit.yield_farm_entries,
			TryInto::<BoundedVec<YieldFarmEntry<Test, Instance1>, ConstU32<5>>>::try_into(vec![]).unwrap()
		);

		assert_noop!(
			deposit.remove_yield_farm_entry(60),
			Error::<Test, Instance1>::YieldFarmEntryNotFound
		);
	});
}

#[test]
fn deposit_get_yield_farm_entry_should_work() {
	let mut deposit = DepositData::<Test, Instance1> {
		shares: 10,
		amm_pool_id: BSX_TKN1_AMM,
		yield_farm_entries: vec![
			YieldFarmEntry::<Test, Instance1>::new(4, 1, 20, FixedU128::from(10), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(7, 2, 20, FixedU128::from(1), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(6, 4, 20, FixedU128::from(10), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(2, 18, 20, FixedU128::from(14), 18, 0),
			YieldFarmEntry::<Test, Instance1>::new(3, 60, 20, FixedU128::from(1), 1, 0),
		]
		.try_into()
		.unwrap(),
	};

	assert_eq!(
		deposit.get_yield_farm_entry(18).unwrap(),
		&mut YieldFarmEntry::<Test, Instance1>::new(2, 18, 20, FixedU128::from(14), 18, 0)
	);

	const NON_EXISTING_YIELD_FARM_ID: YieldFarmId = 98_908;
	assert!(deposit.get_yield_farm_entry(NON_EXISTING_YIELD_FARM_ID).is_none())
}

#[test]
fn deposit_search_yield_farm_entry_should_work() {
	let deposit = DepositData::<Test, Instance1> {
		shares: 10,
		amm_pool_id: BSX_TKN1_AMM,
		yield_farm_entries: vec![
			YieldFarmEntry::<Test, Instance1>::new(4, 1, 20, FixedU128::from(10), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(7, 2, 20, FixedU128::from(1), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(6, 4, 20, FixedU128::from(10), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(2, 18, 20, FixedU128::from(14), 18, 0),
			YieldFarmEntry::<Test, Instance1>::new(3, 60, 20, FixedU128::from(1), 1, 0),
		]
		.try_into()
		.unwrap(),
	};

	assert!(deposit.search_yield_farm_entry(1).is_some());
	assert!(deposit.search_yield_farm_entry(60).is_some());
	assert!(deposit.search_yield_farm_entry(4).is_some());

	const NON_EXISTING_YIELD_FARM_ID: YieldFarmId = 98_908;

	assert!(deposit.search_yield_farm_entry(NON_EXISTING_YIELD_FARM_ID).is_none());
}

#[test]
fn deposit_can_be_flushed_should_work() {
	//non empty deposit can't be flushed
	let deposit = DepositData::<Test, Instance1> {
		shares: 10,
		amm_pool_id: BSX_TKN1_AMM,
		yield_farm_entries: vec![
			YieldFarmEntry::<Test, Instance1>::new(4, 1, 20, FixedU128::from(10), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(7, 2, 20, FixedU128::from(1), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(6, 4, 20, FixedU128::from(10), 13, 0),
			YieldFarmEntry::<Test, Instance1>::new(2, 18, 20, FixedU128::from(14), 18, 0),
			YieldFarmEntry::<Test, Instance1>::new(3, 60, 20, FixedU128::from(1), 1, 0),
		]
		.try_into()
		.unwrap(),
	};

	assert!(!deposit.can_be_removed());

	let deposit = DepositData::<Test, Instance1> {
		shares: 10,
		amm_pool_id: BSX_TKN1_AMM,
		yield_farm_entries: vec![YieldFarmEntry::<Test, Instance1>::new(
			4,
			1,
			20,
			FixedU128::from(10),
			13,
			0,
		)]
		.try_into()
		.unwrap(),
	};

	assert!(!deposit.can_be_removed());

	//deposit with no entries can be flushed
	let deposit = DepositData::<Test, Instance1> {
		shares: 10,
		amm_pool_id: BSX_TKN1_AMM,
		yield_farm_entries: vec![].try_into().unwrap(),
	};

	assert!(deposit.can_be_removed());
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "Defensive failure has been triggered!"))]
fn yield_farm_data_should_work() {
	new_test_ext().execute_with(|| {
		let mut yield_farm =
			YieldFarmData::<Test, Instance1>::new(1, 10, Some(LoyaltyCurve::default()), FixedU128::from(10_000));

		//new farm should be created active
		assert!(yield_farm.state.is_active());
		assert!(!yield_farm.state.is_stopped());
		assert!(!yield_farm.state.is_terminated());

		yield_farm.state = FarmState::Stopped;
		assert!(!yield_farm.state.is_active());
		assert!(yield_farm.state.is_stopped());
		assert!(!yield_farm.state.is_terminated());

		yield_farm.state = FarmState::Terminated;
		assert!(!yield_farm.state.is_active());
		assert!(!yield_farm.state.is_stopped());
		assert!(yield_farm.state.is_terminated());

		assert_ok!(yield_farm.increase_entries_count());
		assert_eq!(yield_farm.entries_count, 1);
		assert_ok!(yield_farm.increase_entries_count());
		assert_ok!(yield_farm.increase_entries_count());
		assert_ok!(yield_farm.increase_entries_count());
		assert_eq!(yield_farm.entries_count, 4);

		assert_ok!(yield_farm.decrease_entries_count());
		assert_eq!(yield_farm.entries_count, 3);
		assert_ok!(yield_farm.decrease_entries_count());
		assert_ok!(yield_farm.decrease_entries_count());
		assert_ok!(yield_farm.decrease_entries_count());
		assert_eq!(yield_farm.entries_count, 0);
		assert_noop!(
			yield_farm.decrease_entries_count(),
			Error::<Test, Instance1>::InconsistentState(InconsistentStateError::InvalidYieldFarmEntriesCount)
		);

		//no entries in the farm
		yield_farm.entries_count = 0;
		assert!(!yield_farm.has_entries());
		assert_ok!(yield_farm.increase_entries_count());
		assert!(yield_farm.has_entries());

		yield_farm.state = FarmState::Active;
		yield_farm.entries_count = 0;
		//active farm can't be flushed
		assert!(!yield_farm.can_be_removed());

		//stopped farm can't be flushed
		yield_farm.state = FarmState::Stopped;
		assert!(!yield_farm.can_be_removed());

		//deleted farm with entries can't be flushed
		yield_farm.state = FarmState::Terminated;
		yield_farm.entries_count = 1;
		assert!(!yield_farm.can_be_removed());

		//deleted farm with no entries can be flushed
		yield_farm.entries_count = 0;
		assert!(yield_farm.can_be_removed());
	});
}

#[test]
fn global_farm_should_work() {
	let mut global_farm = GlobalFarmData::<Test, Instance1>::new(
		1,
		10,
		BSX,
		Perquintill::from_float(0.2),
		1_000,
		100,
		GC,
		BSX,
		1_000_000,
		1_000,
		One::one(),
	);

	//new farm should be created active
	assert!(global_farm.state.is_active());
	global_farm.state = FarmState::Terminated;
	assert!(!global_farm.state.is_active());

	global_farm.state = FarmState::Active;

	assert_ok!(global_farm.increase_yield_farm_counts());
	assert_ok!(global_farm.increase_yield_farm_counts());
	assert_eq!(global_farm.live_yield_farms_count, 2);
	assert_eq!(global_farm.total_yield_farms_count, 2);
	assert_ok!(global_farm.increase_yield_farm_counts());
	assert_ok!(global_farm.increase_yield_farm_counts());
	assert_eq!(global_farm.live_yield_farms_count, 4);
	assert_eq!(global_farm.total_yield_farms_count, 4);
	assert_ok!(global_farm.decrease_live_yield_farm_count());
	assert_ok!(global_farm.decrease_live_yield_farm_count());
	//removing farm changes only live farms, total count is not changed
	assert_eq!(global_farm.live_yield_farms_count, 2);
	assert_eq!(global_farm.total_yield_farms_count, 4);
	assert_ok!(global_farm.increase_yield_farm_counts());
	assert_eq!(global_farm.live_yield_farms_count, 3);
	assert_eq!(global_farm.total_yield_farms_count, 5);
	assert_ok!(global_farm.decrease_total_yield_farm_count());
	assert_ok!(global_farm.decrease_total_yield_farm_count());
	//removing farm changes only total count(farm has to removed and deleted before it can be
	//flushed)
	assert_eq!(global_farm.live_yield_farms_count, 3);
	assert_eq!(global_farm.total_yield_farms_count, 3);

	assert!(global_farm.has_live_farms());
	global_farm.live_yield_farms_count = 0;
	global_farm.total_yield_farms_count = 3;
	assert!(!global_farm.has_live_farms());

	//active farm can't be flushed
	assert!(!global_farm.can_be_removed());
	global_farm.state = FarmState::Terminated;
	//deleted farm with yield farm can't be flushed
	assert!(!global_farm.can_be_removed());
	//deleted farm with no yield farms can be flushed
	global_farm.live_yield_farms_count = 0;
	global_farm.total_yield_farms_count = 0;
	assert!(global_farm.can_be_removed());
}

#[test]
fn global_farm_add_stake_should_work_when_amount_is_provided() {
	let global_farm_0 = GlobalFarmData::<Test, Instance1>::new(
		1,
		10,
		BSX,
		Perquintill::from_float(0.2),
		1_000,
		100,
		GC,
		BSX,
		1_000_000,
		1_000,
		One::one(),
	);

	let mut global_farm = global_farm_0.clone();

	assert_ok!(global_farm.add_stake(1));
	assert_eq!(
		global_farm,
		GlobalFarmData {
			total_shares_z: 1,
			..global_farm_0.clone()
		}
	);

	assert_ok!(global_farm.add_stake(1_000_000 * ONE));
	assert_eq!(
		global_farm,
		GlobalFarmData {
			total_shares_z: 1_000_000 * ONE + 1, //+1 from previous add_stake
			..global_farm_0
		}
	);
}

#[test]
fn global_farm_remove_stake_should_work_when_amount_is_provided() {
	new_test_ext().execute_with(|| {
		let mut global_farm_0 = GlobalFarmData::<Test, Instance1>::new(
			1,
			10,
			BSX,
			Perquintill::from_float(0.2),
			1_000,
			100,
			GC,
			BSX,
			1_000_000,
			1_000,
			One::one(),
		);

		global_farm_0.total_shares_z = 1_000_000 * ONE;

		let mut global_farm = global_farm_0.clone();

		assert_ok!(global_farm.remove_stake(1));
		assert_eq!(
			global_farm,
			GlobalFarmData {
				total_shares_z: 1_000_000 * ONE - 1,
				..global_farm_0.clone()
			}
		);

		assert_ok!(global_farm.remove_stake(1_000_000 * ONE - 1)); //-1 from previous remove_stake
		assert_eq!(
			global_farm,
			GlobalFarmData {
				total_shares_z: 0,
				..global_farm_0
			}
		);
	});
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "Defensive failure has been triggered!"))]
fn global_farm_remove_stake_should_not_work_when_math_overflow() {
	new_test_ext().execute_with(|| {
		let mut global_farm_0 = GlobalFarmData::<Test, Instance1>::new(
			1,
			10,
			BSX,
			Perquintill::from_float(0.2),
			1_000,
			100,
			GC,
			BSX,
			1_000_000,
			1_000,
			One::one(),
		);

		global_farm_0.total_shares_z = 1_000_000 * ONE;
		let mut global_farm = global_farm_0;

		//sub with overflow
		assert_noop!(
			global_farm.remove_stake(1 + 1_000_000 * ONE),
			Error::InconsistentState(InconsistentStateError::InvalidTotalSharesZ)
		);
	});
}

#[test]
fn is_yield_farm_clamable_should_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			//active farm
			assert!(LiquidityMining::is_yield_farm_claimable(
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			//invalid amm_pool_id
			assert!(!LiquidityMining::is_yield_farm_claimable(
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN2_AMM
			));

			//farm withouth deposits
			assert!(!LiquidityMining::is_yield_farm_claimable(
				EVE_FARM,
				EVE_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			//termianted yield farm
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));
			assert_ok!(LiquidityMining::terminate_yield_farm(
				GC,
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			assert!(!LiquidityMining::is_yield_farm_claimable(
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn get_global_farm_id_should_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			//happy path
			assert_eq!(
				LiquidityMining::get_global_farm_id(PREDEFINED_DEPOSIT_IDS[0], GC_BSX_TKN1_YIELD_FARM_ID),
				Some(GC_FARM)
			);

			//happy path deposit with multiple farm entries
			//create second farm entry
			assert_ok!(LiquidityMining::redeposit_lp_shares(
				EVE_FARM,
				EVE_BSX_TKN1_YIELD_FARM_ID,
				PREDEFINED_DEPOSIT_IDS[0],
				|_, _, _| { Ok(1_000_u128) }
			));

			assert_eq!(
				LiquidityMining::get_global_farm_id(PREDEFINED_DEPOSIT_IDS[0], EVE_BSX_TKN1_YIELD_FARM_ID),
				Some(EVE_FARM)
			);

			//deposit doesn't exists
			assert!(LiquidityMining::get_global_farm_id(999_9999, GC_BSX_TKN1_YIELD_FARM_ID).is_none());

			//farm's entry doesn't exists in the deposit
			assert!(
				LiquidityMining::get_global_farm_id(PREDEFINED_DEPOSIT_IDS[0], DAVE_BSX_TKN1_YIELD_FARM_ID).is_none()
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn farm_state_should_work() {
	let active = FarmState::Active;
	let deleted = FarmState::Terminated;
	let stopped = FarmState::Stopped;

	assert_eq!(active.is_active(), true);
	assert_eq!(active.is_stopped(), false);
	assert_eq!(active.is_terminated(), false);

	assert_eq!(stopped.is_active(), false);
	assert_eq!(stopped.is_stopped(), true);
	assert_eq!(stopped.is_terminated(), false);

	assert_eq!(deleted.is_active(), false);
	assert_eq!(deleted.is_stopped(), false);
	assert_eq!(deleted.is_terminated(), true);
}

#[test]
fn min_yield_farm_multiplier_should_be_ge_1_when_multiplied_by_min_deposit() {
	//WARN: don't remove this test. This rule is important.
	// min_yield_farm_multiplier * min_deposit >=1 otherwise non-zero deposit can result in a zero
	// stake in global-farm and farm can be falsely identified as empty.
	//https://github.com/galacticcouncil/warehouse/issues/127

	assert_eq!(
		crate::MIN_YIELD_FARM_MULTIPLIER
			.checked_mul_int(crate::MIN_DEPOSIT)
			.unwrap()
			.ge(&1_u128),
		true
	);
}

#[test]
fn sync_global_farm_should_emit_all_rewards_distributed_when_reward_is_zero() {
	new_test_ext().execute_with(|| {
		let global_farm_id = 10;

		let mut global_farm = GlobalFarmData::new(
			global_farm_id,
			10,
			BSX,
			Perquintill::from_percent(1),
			10_000,
			1,
			ALICE,
			BSX,
			1_000_000 * ONE,
			1_000,
			One::one(),
		);
		global_farm.total_shares_z = 1_000 * ONE;

		let farm_account_id = LiquidityMining::farm_account_id(global_farm_id).unwrap();
		Whitelist::add_account(&farm_account_id).unwrap();

		assert_eq!(Tokens::free_balance(BSX, &farm_account_id), Balance::zero());

		assert_eq!(
			with_transaction(|| {
				TransactionOutcome::Commit(LiquidityMining::sync_global_farm(&mut global_farm, 1_000_000_000))
			})
			.unwrap(),
			Balance::zero()
		);

		frame_system::Pallet::<Test>::assert_has_event(mock::RuntimeEvent::LiquidityMining(
			Event::AllRewardsDistributed { global_farm_id },
		));
	});
}
