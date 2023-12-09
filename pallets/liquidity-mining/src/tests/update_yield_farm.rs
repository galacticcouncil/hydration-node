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
fn update_yield_farm_should_work() {
	//Yield farm without deposits.
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			let new_multiplier: FarmMultiplier = FixedU128::from(5_000_u128);
			let yield_farm = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
			let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

			assert_eq!(
				LiquidityMining::update_yield_farm_multiplier(GC, GC_FARM, BSX_TKN1_AMM, new_multiplier).unwrap(),
				GC_BSX_TKN1_YIELD_FARM_ID
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
				YieldFarmData {
					multiplier: new_multiplier,
					..yield_farm
				}
			);

			assert_eq!(LiquidityMining::global_farm(GC_FARM).unwrap(), global_farm);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	//Yield farm with deposits.
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			//Same period as last yield farm update so no farms(global or yield) need to be updated.
			let new_multiplier: FarmMultiplier = FixedU128::from(10_000_u128);
			let yield_farm = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
			let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

			assert_ok!(LiquidityMining::update_yield_farm_multiplier(
				GC,
				GC_FARM,
				BSX_TKN1_AMM,
				new_multiplier,
			));

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
				YieldFarmData {
					multiplier: new_multiplier,
					..yield_farm
				}
			);

			assert_eq!(
				LiquidityMining::global_farm(GC_FARM).unwrap(),
				GlobalFarmData {
					total_shares_z: 455_876_290 * ONE,
					..global_farm
				}
			);

			//Different period so farms update should happen.
			set_block_number(5_000);
			let new_multiplier: FarmMultiplier = FixedU128::from(5_000_u128);
			let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
			let pot = LiquidityMining::pot_account_id().unwrap();
			let expected_claimed_from_global_farm = 1_498_432_831_415_733_421_593_u128;
			//This is not claimed by other yield farms.
			let expected_allocated_for_other_yield_farms = 1_567_168_584_266_578_407_u128;

			//_0 - value before action.
			let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
			let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();
			let global_farm_balance_0 = Tokens::free_balance(BSX, &global_farm_account);
			let pot_balance_0 = Tokens::free_balance(BSX, &pot);

			assert_ok!(LiquidityMining::update_yield_farm_multiplier(
				GC,
				GC_FARM,
				BSX_TKN1_AMM,
				new_multiplier,
			));

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
				YieldFarmData {
					updated_at: 50,
					accumulated_rpvs: FixedU128::from_inner(32_921_163_403_946_715_450_000_u128),
					accumulated_rpz: FixedU128::from_inner(6_790_366_340_394_671_545_u128),
					multiplier: new_multiplier,
					left_to_distribute: yield_farm_0.left_to_distribute + expected_claimed_from_global_farm,
					..yield_farm_0
				}
			);

			assert_eq!(
				LiquidityMining::global_farm(GC_FARM).unwrap(),
				GlobalFarmData {
					updated_at: 50,
					accumulated_rpz: FixedU128::from_inner(6_790_366_340_394_671_545_u128),
					total_shares_z: 228_176_290 * ONE,
					pending_rewards: global_farm_0.pending_rewards + expected_allocated_for_other_yield_farms,
					accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards
						+ expected_claimed_from_global_farm,
					..global_farm_0
				}
			);

			assert_eq!(
				Tokens::free_balance(BSX, &global_farm_account),
				global_farm_balance_0 - expected_claimed_from_global_farm - expected_allocated_for_other_yield_farms
			);
			assert_eq!(
				Tokens::free_balance(BSX, &pot),
				pot_balance_0 + expected_claimed_from_global_farm + expected_allocated_for_other_yield_farms
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn update_yield_farm_multiplier_should_not_work_when_multiplier_is_lt_min_yield_farm_multiplier() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			assert_noop!(
				LiquidityMining::update_yield_farm_multiplier(
					GC,
					GC_FARM,
					BSX_TKN1_AMM,
					MIN_YIELD_FARM_MULTIPLIER - FixedU128::from_inner(1_u128),
				),
				Error::<Test, Instance1>::InvalidMultiplier
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn update_yield_farm_stopped_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			//Yield farm must be in the active yield farm storage to update works.
			assert_noop!(
				LiquidityMining::update_yield_farm_multiplier(GC, GC_FARM, BSX_TKN1_AMM, FixedU128::from(10_001),),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn update_yield_farm_termianted_farm_should_not_work() {
	//NOTE: yield farm is in the storage but it's deleted.
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			assert_ok!(LiquidityMining::terminate_yield_farm(
				GC,
				GC_FARM,
				GC_BSX_TKN1_YIELD_FARM_ID,
				BSX_TKN1_AMM
			));

			assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).is_some());

			//Yield farm must be in the active yield farm storage to update works
			assert_noop!(
				LiquidityMining::update_yield_farm_multiplier(GC, GC_FARM, BSX_TKN1_AMM, FixedU128::from(10_001),),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn update_yield_farm_not_owner_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let not_owner = ALICE;
			assert_noop!(
				LiquidityMining::update_yield_farm_multiplier(
					not_owner,
					GC_FARM,
					BSX_TKN1_AMM,
					FixedU128::from(10_001_u128),
				),
				Error::<Test, Instance1>::Forbidden
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
