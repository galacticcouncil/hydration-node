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
fn resume_yield_farm_should_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			//Stop yield farming before resuming.
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			let yield_farm = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
			let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

			let new_multiplier = FixedU128::from(7_490_000);

			assert!(yield_farm.state.is_stopped());
			assert!(yield_farm.multiplier.is_zero());
			assert!(LiquidityMining::active_yield_farm(BSX_TKN1_AMM, GC_FARM).is_none());

			set_block_number(13_420_000);

			assert_ok!(LiquidityMining::resume_yield_farm(
				GC,
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM,
				new_multiplier
			));

			let yield_farm_stake_in_global_farm = new_multiplier.checked_mul_int(45_540 * ONE).unwrap();

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
				YieldFarmData {
					state: FarmState::Active,
					accumulated_rpz: FixedU128::from_inner(62_987_640_859_560_351_884_356_u128),
					multiplier: new_multiplier,
					updated_at: 134_200,
					total_stopped: 134_175,
					..yield_farm
				}
			);

			assert_eq!(
				LiquidityMining::global_farm(GC_FARM).unwrap(),
				GlobalFarmData {
					total_shares_z: global_farm.total_shares_z + yield_farm_stake_in_global_farm,
					updated_at: 134_200,
					accumulated_rpz: FixedU128::from_inner(62_987_640_859_560_351_884_356_u128),
					pending_rewards: 29_998_716_449_999_999_999_000,
					..global_farm
				}
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn resume_yield_farm_non_existing_yield_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier = FixedU128::from(7_490_000);

			assert_noop!(
				LiquidityMining::resume_yield_farm(GC, GC_FARM, BSX_KSM_YIELD_FARM_ID, BSX_KSM_AMM, new_multiplier),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn resume_yield_farm_non_canceled_yield_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier = FixedU128::from(7_490_000);

			assert_noop!(
				LiquidityMining::resume_yield_farm(
					GC,
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					new_multiplier
				),
				Error::<Test, Instance1>::YieldFarmAlreadyExists
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn resume_yield_farm_not_owner_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier = FixedU128::from(7_490_000);

			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			assert_noop!(
				LiquidityMining::resume_yield_farm(
					ALICE,
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					new_multiplier
				),
				Error::<Test, Instance1>::Forbidden
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn resume_yield_farm_terminated_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier = FixedU128::from(7_490_000);

			//Farm have to be stopped before delete.
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));
			//Terminate farm.
			assert_ok!(LiquidityMining::terminate_yield_farm(
				GC,
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			assert_noop!(
				LiquidityMining::resume_yield_farm(
					ALICE,
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					new_multiplier
				),
				Error::<Test, Instance1>::LiquidityMiningIsNotStopped
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

//This function is testing case when new yield farm for the same asset pair was created in the global
//farm while first yield farm was stopped.
#[test]
fn resume_yield_farm_same_amm_farm_active_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier = FixedU128::from(7_490_000);

			//Stop 1-th farm.
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			//Create new farm for same assert pair.
			assert_ok!(with_transaction(|| TransactionOutcome::Commit({
				LiquidityMining::create_yield_farm(
					GC,
					GC_FARM,
					FixedU128::from(10_000_u128),
					None,
					BSX_TKN1_AMM,
					vec![BSX, TKN1],
				)
			})));

			assert_noop!(
				LiquidityMining::resume_yield_farm(
					ALICE,
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					new_multiplier
				),
				Error::<Test, Instance1>::YieldFarmAlreadyExists
			);
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn resume_yield_farm_should_not_work_when_multiplier_is_lt_min_yield_farm_multiplier() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier = MIN_YIELD_FARM_MULTIPLIER - FixedU128::from_inner(1_u128);

			//Arrange
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			//Act & assert
			assert_noop!(
				LiquidityMining::resume_yield_farm(
					GC,
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					new_multiplier
				),
				Error::<Test, Instance1>::InvalidMultiplier
			);
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
