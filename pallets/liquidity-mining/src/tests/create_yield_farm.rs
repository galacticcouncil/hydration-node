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

#[test]
fn create_yield_farm_should_work() {
	//NOTE: global_farm.updated_at isn't changed because global farm is empty.
	let test_data = vec![
		(
			AssetPair {
				asset_in: BSX,
				asset_out: ACA,
			},
			YieldFarmData {
				id: 13,
				updated_at: 17,
				total_shares: 0,
				total_valued_shares: 0,
				accumulated_rpvs: Zero::zero(),
				accumulated_rpz: Zero::zero(),
				multiplier: FixedU128::from(20_000_u128),
				loyalty_curve: Some(LoyaltyCurve::default()),
				entries_count: 0,
				state: FarmState::Active,
				left_to_distribute: 0,
				total_stopped: 0,
				_phantom: PhantomData,
			},
			BSX_ACA_AMM,
			ALICE,
			ALICE_FARM,
			17_850,
			GlobalFarmData {
				updated_at: 17,
				live_yield_farms_count: 1,
				total_yield_farms_count: 1,
				..get_predefined_global_farm_ins1(0)
			},
		),
		(
			AssetPair {
				asset_in: KSM,
				asset_out: BSX,
			},
			YieldFarmData {
				id: 14,
				updated_at: 17,
				total_shares: 0,
				total_valued_shares: 0,
				accumulated_rpvs: Zero::zero(),
				accumulated_rpz: Zero::zero(),
				multiplier: FixedU128::from(10_000_u128),
				loyalty_curve: None,
				entries_count: 0,
				state: FarmState::Active,
				left_to_distribute: 0,
				total_stopped: 0,
				_phantom: PhantomData,
			},
			BSX_KSM_AMM,
			ALICE,
			ALICE_FARM,
			17_850,
			GlobalFarmData {
				updated_at: 17,
				live_yield_farms_count: 2,
				total_yield_farms_count: 2,
				..get_predefined_global_farm_ins1(0)
			},
		),
		(
			AssetPair {
				asset_in: BSX,
				asset_out: ETH,
			},
			YieldFarmData {
				id: 15,
				updated_at: 20,
				total_shares: 0,
				total_valued_shares: 0,
				accumulated_rpvs: Zero::zero(),
				accumulated_rpz: Zero::zero(),
				multiplier: FixedU128::from(10_000_u128),
				loyalty_curve: Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from_inner(100_000_000_000_000_000),
					scale_coef: 50,
				}),
				state: FarmState::Active,
				entries_count: 0,
				left_to_distribute: 0,
				total_stopped: 0,
				_phantom: PhantomData,
			},
			BSX_ETH_AMM,
			ALICE,
			ALICE_FARM,
			20_000,
			GlobalFarmData {
				updated_at: 20,
				live_yield_farms_count: 3,
				total_yield_farms_count: 3,
				..get_predefined_global_farm_ins1(0)
			},
		),
		(
			AssetPair {
				asset_in: BSX,
				asset_out: ETH,
			},
			YieldFarmData {
				id: 16,
				updated_at: 2,
				total_shares: 0,
				total_valued_shares: 0,
				accumulated_rpvs: Zero::zero(),
				accumulated_rpz: Zero::zero(),
				multiplier: FixedU128::from(50_000_128),
				loyalty_curve: Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from_inner(1),
					scale_coef: 0,
				}),
				state: FarmState::Active,
				entries_count: 0,
				left_to_distribute: 0,
				total_stopped: 0,
				_phantom: PhantomData,
			},
			BSX_ETH_AMM,
			BOB,
			BOB_FARM,
			20_000,
			GlobalFarmData {
				updated_at: 2,
				live_yield_farms_count: 1,
				total_yield_farms_count: 1,
				..get_predefined_global_farm_ins1(1)
			},
		),
	];

	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			for (assets, yield_farm, amm_id, who, global_farm_id, now, global_farm) in test_data.clone() {
				set_block_number(now);

				assert_eq!(
					LiquidityMining::create_yield_farm(
						who,
						global_farm_id,
						yield_farm.multiplier,
						yield_farm.loyalty_curve.clone(),
						amm_id,
						vec![assets.asset_in, assets.asset_out],
					)
					.unwrap(),
					yield_farm.id
				);

				assert_eq!(LiquidityMining::global_farm(global_farm_id).unwrap(), global_farm);

				assert_eq!(
					LiquidityMining::active_yield_farm(amm_id, global_farm_id).unwrap(),
					yield_farm.id
				);

				assert_eq!(
					LiquidityMining::yield_farm((amm_id, global_farm_id, yield_farm.id)).unwrap(),
					YieldFarmData { ..yield_farm }
				);
			}

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn add_yield_farm_missing_incentivized_asset_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			assert_noop!(
				LiquidityMining::create_yield_farm(
					ALICE,
					ALICE_FARM,
					FixedU128::from(10_000_u128),
					None,
					KSM_DOT_AMM,
					//Neither KSM nor DOT is incentivized by the farm.
					vec![KSM, DOT],
				),
				Error::<Test, Instance1>::MissingIncentivizedAsset
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn add_yield_farm_not_owner_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			assert_noop!(
				LiquidityMining::create_yield_farm(
					BOB,
					ALICE_FARM,
					FixedU128::from(10_000_u128),
					None,
					BSX_HDX_AMM,
					vec![BSX, HDX],
				),
				Error::<Test, Instance1>::Forbidden
			);

			assert_noop!(
				LiquidityMining::create_yield_farm(
					BOB,
					ALICE_FARM,
					FixedU128::from(10_000_u128),
					Some(LoyaltyCurve::default()),
					BSX_HDX_AMM,
					vec![BSX, HDX],
				),
				Error::<Test, Instance1>::Forbidden
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn add_yield_farm_invalid_loyalty_curve_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			let curves = vec![
				Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::one(),
					scale_coef: 0,
				}),
				Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from_float(1.0),
					scale_coef: 1_000_000,
				}),
				Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from_float(1.000_000_000_000_000_001),
					scale_coef: 25_996_000,
				}),
				Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from(1_u128),
					scale_coef: 25_996_000,
				}),
				Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from(5_u128),
					scale_coef: 25_996_000,
				}),
				Some(LoyaltyCurve {
					initial_reward_percentage: FixedU128::from(16_874_354_654_u128),
					scale_coef: 25_996_000,
				}),
			];

			for c in curves {
				assert_noop!(
					LiquidityMining::create_yield_farm(
						ALICE,
						ALICE_FARM,
						FixedU128::from(10_000_u128),
						c,
						BSX_HDX_AMM,
						vec![BSX, HDX],
					),
					Error::<Test, Instance1>::InvalidInitialRewardPercentage
				);
			}

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn add_yield_farm_invalid_multiplier_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			assert_noop!(
				LiquidityMining::create_yield_farm(
					ALICE,
					ALICE_FARM,
					MIN_YIELD_FARM_MULTIPLIER - FixedU128::from_inner(1_u128),
					Some(LoyaltyCurve::default()),
					BSX_HDX_AMM,
					vec![BSX, HDX],
				),
				Error::<Test, Instance1>::InvalidMultiplier
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn add_yield_farm_add_duplicate_amm_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			set_block_number(20_000);

			let aca_ksm_assets = AssetPair {
				asset_in: ACA,
				asset_out: KSM,
			};

			let aca_ksm_amm_account =
				AMM_POOLS.with(|v| v.borrow().get(&asset_pair_to_map_key(aca_ksm_assets)).unwrap().0);

			assert!(LiquidityMining::active_yield_farm(aca_ksm_amm_account, CHARLIE_FARM).is_some());

			//Try to add same amm second time in the same block(period)
			assert_noop!(
				LiquidityMining::create_yield_farm(
					CHARLIE,
					CHARLIE_FARM,
					FixedU128::from(9_000_u128),
					Some(LoyaltyCurve::default()),
					ACA_KSM_AMM,
					vec![ACA, KSM],
				),
				Error::<Test, Instance1>::YieldFarmAlreadyExists
			);

			//Try to add same amm second time in later block(period).
			set_block_number(30_000);

			assert_noop!(
				LiquidityMining::create_yield_farm(
					CHARLIE,
					CHARLIE_FARM,
					FixedU128::from(9_000_u128),
					Some(LoyaltyCurve::default()),
					ACA_KSM_AMM,
					vec![ACA, KSM],
				),
				Error::<Test, Instance1>::YieldFarmAlreadyExists
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn add_yield_farm_global_farm_full_should_not_work() {
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			//GC FARM is initialized with 2 yield farms and mock limit is 4.

			// This should works 3-rd yield farm
			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				GC_FARM,
				10_000.into(),
				Some(LoyaltyCurve::default()),
				BSX_ACA_AMM,
				vec![BSX, ACA],
			));

			// This should works 4-th yield farm
			let bsx_dot_yield_farm_id = LiquidityMining::create_yield_farm(
				GC,
				GC_FARM,
				10_000.into(),
				Some(LoyaltyCurve::default()),
				BSX_DOT_AMM,
				vec![BSX, DOT],
			)
			.unwrap();

			// This should fail.
			assert_noop!(
				LiquidityMining::create_yield_farm(
					GC,
					GC_FARM,
					10_000.into(),
					Some(LoyaltyCurve::default()),
					BSX_HDX_AMM,
					vec![BSX, HDX],
				),
				Error::<Test, Instance1>::GlobalFarmIsFull
			);

			//Stop farm
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_DOT_AMM));

			//This still should now work because yield farms in storage are included in counts.
			assert_noop!(
				LiquidityMining::create_yield_farm(
					GC,
					GC_FARM,
					10_000.into(),
					Some(LoyaltyCurve::default()),
					BSX_HDX_AMM,
					vec![BSX, HDX],
				),
				Error::<Test, Instance1>::GlobalFarmIsFull
			);

			//Stop and destroy yield farm (destroyed farm stays in the storage because of deposit)
			//deposit to yield so it's not flushed on destroy
			assert_ok!(LiquidityMining::deposit_lp_shares(
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM,
				1_000,
				|_, _, _| { Ok(1_000_u128) }
			));

			//stop and destroy
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));
			assert_ok!(LiquidityMining::terminate_yield_farm(
				GC,
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID))
					.unwrap()
					.state,
				FarmState::Terminated
			);

			//This still should now work because deleted yield farms in storage are included in counts.
			assert_noop!(
				LiquidityMining::create_yield_farm(
					GC,
					GC_FARM,
					10_000.into(),
					Some(LoyaltyCurve::default()),
					BSX_HDX_AMM,
					vec![BSX, HDX],
				),
				Error::<Test, Instance1>::GlobalFarmIsFull
			);

			//Destroy stopped empty farm(it will be flushed from storage).
			assert_ok!(LiquidityMining::terminate_yield_farm(
				GC,
				GC_FARM,
				bsx_dot_yield_farm_id,
				BSX_DOT_AMM
			));

			//This should pass because yield farm was flushed from storage.
			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				GC_FARM,
				10_000.into(),
				Some(LoyaltyCurve::default()),
				BSX_HDX_AMM,
				vec![BSX, HDX],
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
