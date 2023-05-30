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
fn deposit_lp_shares_should_work() {
	//NOTE: farm incentivize BSX token.
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			let global_farm_id = GC_FARM;
			let global_farm_account = LiquidityMining::farm_account_id(global_farm_id).unwrap();
			let pot = LiquidityMining::pot_account_id().unwrap();

			//DEPOSIT 1:
			set_block_number(1_800); //18-th period

			let deposited_amount = 50 * ONE;
			let yield_farm_id = GC_BSX_TKN1_YIELD_FARM_ID;
			//_0 - value berfore act
			let bsx_tkn1_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN1_AMM,
					deposited_amount,
					|_, _, _| { Ok(2_500 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[0]
			);

			assert_eq!(
				LiquidityMining::global_farm(GC_FARM).unwrap(),
				GlobalFarmData {
					total_shares_z: 12_500 * ONE,
					updated_at: 18,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, yield_farm_id)).unwrap(),
				YieldFarmData {
					total_shares: deposited_amount,
					total_valued_shares: 2_500 * ONE,
					updated_at: 18,
					entries_count: 1,
					..bsx_tkn1_yield_farm_0
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN1_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN1_YIELD_FARM_ID,
						2_500 * ONE,
						Zero::zero(),
						18,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			// DEPOSIT 2 (deposit in the same period):
			let deposited_amount = 80 * ONE;
			let yield_farm_id = GC_BSX_TKN1_YIELD_FARM_ID;
			//_0 - value berfore act
			let bsx_tkn1_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN1_AMM,
					deposited_amount,
					|_, _, _| { Ok(4_160 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[1]
			);

			assert_eq!(
				LiquidityMining::global_farm(global_farm_id).unwrap(),
				GlobalFarmData {
					accumulated_rpz: Zero::zero(),
					updated_at: 18,
					accumulated_paid_rewards: 0,
					total_shares_z: 33_300 * ONE,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, yield_farm_id)).unwrap(),
				YieldFarmData {
					updated_at: 18,
					total_shares: 130 * ONE,
					total_valued_shares: 6_660 * ONE,
					entries_count: 2,
					..bsx_tkn1_yield_farm_0
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[1]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN1_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN1_YIELD_FARM_ID,
						4_160 * ONE,
						Zero::zero(),
						18,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			//Nohtig was claimed by yield-farm so nothings should change.
			assert_eq!(Tokens::free_balance(BSX, &global_farm_account), 30_000_000_000 * ONE);
			assert_eq!(Tokens::free_balance(BSX, &pot), 0);

			// DEPOSIT 3 (same period, second yield farm):
			let deposited_amount = 25 * ONE;
			let yield_farm_id = GC_BSX_TKN2_YIELD_FARM_ID;

			//_0 - value berfore act.
			let bsx_tkn2_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN2_AMM,
					deposited_amount,
					|_, _, _| { Ok(200 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[2]
			);

			assert_eq!(
				LiquidityMining::global_farm(global_farm_id).unwrap(),
				GlobalFarmData {
					updated_at: 18,
					accumulated_rpz: Zero::zero(),
					accumulated_paid_rewards: 0,
					total_shares_z: 35_300 * ONE,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, yield_farm_id)).unwrap(),
				YieldFarmData {
					updated_at: 18,
					total_shares: 25 * ONE,
					total_valued_shares: 200 * ONE,
					entries_count: 1,
					..bsx_tkn2_yield_farm_0
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[2]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN2_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN2_YIELD_FARM_ID,
						200 * ONE,
						Zero::zero(),
						18,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			//Garms wasn't updated in this period so no claim from global farm happened.
			assert_eq!(Tokens::free_balance(BSX, &global_farm_account), 30_000_000_000 * ONE);
			assert_eq!(Tokens::free_balance(BSX, &pot), 0);

			// DEPOSIT 4 (new period):
			set_block_number(2051); //period 20

			let deposited_amount = 800 * ONE;
			let yield_farm_id = GC_BSX_TKN2_YIELD_FARM_ID;

			//_0 - value berfore act.
			let bsx_tkn2_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN2_AMM,
					deposited_amount,
					|_, _, _| { Ok(46_400 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[3]
			);

			assert_eq!(
				LiquidityMining::global_farm(global_farm_id).unwrap(),
				GlobalFarmData {
					updated_at: 20,
					accumulated_rpz: FixedU128::one(),
					pending_rewards: 33_300 * ONE,
					accumulated_paid_rewards: 2_000 * ONE,
					total_shares_z: 499_300 * ONE,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap(),
				YieldFarmData {
					updated_at: 20,
					accumulated_rpvs: FixedU128::from(10),
					accumulated_rpz: FixedU128::one(),
					total_shares: 825 * ONE,
					total_valued_shares: 46_600 * ONE,
					entries_count: 2,
					left_to_distribute: 2_000 * ONE,
					..bsx_tkn2_yield_farm_0
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[3]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN2_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN2_YIELD_FARM_ID,
						46_400 * ONE,
						FixedU128::from(10),
						20,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			let reserved_for_both_farms = 35_300 * ONE;
			assert_eq!(
				Tokens::free_balance(BSX, &global_farm_account),
				(30_000_000_000 * ONE - reserved_for_both_farms)
			);

			assert_eq!(Tokens::free_balance(BSX, &pot), reserved_for_both_farms);

			// DEPOSIT 5 (same period, second liq pool yield farm):
			set_block_number(2_586); //period 20

			let deposited_amount = 87 * ONE;
			let yield_farm_id = GC_BSX_TKN2_YIELD_FARM_ID;

			//_0 - value berfore act.
			let bsx_tkn2_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN2_AMM,
					deposited_amount,
					|_, _, _| { Ok(261 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[4]
			);

			assert_eq!(
				LiquidityMining::global_farm(global_farm_id).unwrap(),
				GlobalFarmData {
					updated_at: 25,
					accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
					total_shares_z: 501_910 * ONE,
					pending_rewards: 116_550 * ONE,
					accumulated_paid_rewards: 1_167_000 * ONE,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap(),
				YieldFarmData {
					updated_at: 25,
					accumulated_rpvs: FixedU128::from_inner(35_000_000_000_000_000_000_u128),
					accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
					total_shares: 912 * ONE,
					total_valued_shares: 46_861 * ONE,
					entries_count: 3,
					left_to_distribute: 1_167_000 * ONE,
					..bsx_tkn2_yield_farm_0
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[4]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN2_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN2_YIELD_FARM_ID,
						261 * ONE,
						FixedU128::from_inner(35_000_000_000_000_000_000_u128),
						25,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			let reserved_for_both_farms = 1_283_550 * ONE;
			assert_eq!(
				Tokens::free_balance(BSX, &global_farm_account),
				(30_000_000_000 * ONE - reserved_for_both_farms)
			);
			assert_eq!(Tokens::free_balance(BSX, &pot), reserved_for_both_farms);

			// DEPOSIT 6 (same period):
			set_block_number(2_596); //period 20

			let deposited_amount = 48 * ONE;
			let yield_farm_id = GC_BSX_TKN2_YIELD_FARM_ID;

			//_0 - value berfore act.
			let bsx_tkn2_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN2_AMM,
					deposited_amount,
					|_, _, _| { Ok(768 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[5]
			);

			assert_eq!(
				LiquidityMining::global_farm(global_farm_id).unwrap(),
				GlobalFarmData {
					updated_at: 25,
					accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
					total_shares_z: 509_590 * ONE,
					pending_rewards: 116_550 * ONE,
					accumulated_paid_rewards: 1_167_000 * ONE,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, yield_farm_id)).unwrap(),
				YieldFarmData {
					updated_at: 25,
					accumulated_rpvs: FixedU128::from_inner(35_000_000_000_000_000_000_u128),
					accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
					total_shares: 960 * ONE,
					total_valued_shares: 47_629 * ONE,
					entries_count: 4,
					..bsx_tkn2_yield_farm_0 //NOTE: same period so nothing claimed by farm
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[5]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN2_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN2_YIELD_FARM_ID,
						768 * ONE,
						FixedU128::from_inner(35_000_000_000_000_000_000_u128),
						25,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			let reserved_for_both_farms = 1_283_550 * ONE;
			assert_eq!(
				Tokens::free_balance(BSX, &global_farm_account),
				(30_000_000_000 * ONE - reserved_for_both_farms)
			);
			assert_eq!(Tokens::free_balance(BSX, &pot), reserved_for_both_farms);

			// DEPOSIT 7 : (same period different yield farm)
			set_block_number(2_596); //period 20

			let deposited_amount = 486 * ONE;
			let yield_farm_id = GC_BSX_TKN1_YIELD_FARM_ID;

			//_0 - value berfore act.
			let bsx_tkn1_yield_farm_0 =
				LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, yield_farm_id)).unwrap();

			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					BSX_TKN1_AMM,
					deposited_amount,
					|_, _, _| { Ok(38_880 * ONE) }
				)
				.unwrap(),
				PREDEFINED_DEPOSIT_IDS[6]
			);

			assert_eq!(
				LiquidityMining::global_farm(global_farm_id).unwrap(),
				GlobalFarmData {
					updated_at: 25,
					accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
					total_shares_z: 703_990 * ONE,
					pending_rewards: 0,
					accumulated_paid_rewards: 1_283_550 * ONE,
					..get_predefined_global_farm_ins1(2)
				}
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, yield_farm_id)).unwrap(),
				YieldFarmData {
					updated_at: 25,
					accumulated_rpvs: FixedU128::from_inner(17_500_000_000_000_000_000_u128),
					accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
					total_shares: 616 * ONE,
					total_valued_shares: 45_540 * ONE,
					entries_count: 3,
					left_to_distribute: 116_550 * ONE,
					..bsx_tkn1_yield_farm_0
				},
			);

			assert_eq!(
				LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[6]).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: BSX_TKN1_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						global_farm_id,
						GC_BSX_TKN1_YIELD_FARM_ID,
						38_880 * ONE,
						FixedU128::from_inner(17_500_000_000_000_000_000_u128),
						25,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			let reserved_for_both_farms = 1_283_550 * ONE;
			assert_eq!(
				Tokens::free_balance(BSX, &global_farm_account),
				(30_000_000_000 * ONE - reserved_for_both_farms)
			);
			assert_eq!(Tokens::free_balance(BSX, &pot), reserved_for_both_farms);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	//Deposit to farm with different incentivized_asset and reward_currency.
	//Charlie's farm incentivize KSM and reward currency is ACA
	//This test only check if valued shares are correctly calculated if reward and incentivized
	//assets are different, otherwise farm behavior is same as in test above.
	predefined_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			set_block_number(2_596); //period 25

			let deposited_amount = 1_000_000 * ONE;
			let deposit_id = 1; //1 - because new test ext
			let yield_farm_id = CHARLIE_ACA_KSM_YIELD_FARM_ID;
			assert_eq!(
				LiquidityMining::deposit_lp_shares(
					CHARLIE_FARM,
					yield_farm_id,
					ACA_KSM_AMM,
					deposited_amount,
					|_, _, _| { Ok(16_000_000 * ONE) }
				)
				.unwrap(),
				deposit_id
			);

			assert_eq!(
				LiquidityMining::deposit(deposit_id).unwrap(),
				DepositData {
					shares: deposited_amount,
					amm_pool_id: ACA_KSM_AMM,
					yield_farm_entries: vec![YieldFarmEntry::new(
						CHARLIE_FARM,
						CHARLIE_ACA_KSM_YIELD_FARM_ID,
						16_000_000 * ONE,
						Zero::zero(),
						25,
						0
					)]
					.try_into()
					.unwrap(),
				},
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn deposit_lp_shares_bellow_min_deposit_should_not_work() {
	let _ = predefined_test_ext_with_deposits().execute_with(|| {
		with_transaction(|| {
			//NOTE: min. deposit is 10
			let yield_farm_id = GC_BSX_TKN1_YIELD_FARM_ID;

			assert_noop!(
				LiquidityMining::deposit_lp_shares(GC_FARM, yield_farm_id, BSX_TKN1_AMM, 0, |_, _, _| { Ok(10_u128) }),
				Error::<Test, Instance1>::InvalidDepositAmount
			);

			assert_noop!(
				LiquidityMining::deposit_lp_shares(GC_FARM, yield_farm_id, BSX_TKN1_AMM, 1, |_, _, _| { Ok(10_u128) }),
				Error::<Test, Instance1>::InvalidDepositAmount
			);

			assert_noop!(
				LiquidityMining::deposit_lp_shares(GC_FARM, yield_farm_id, BSX_TKN1_AMM, 8, |_, _, _| { Ok(10_u128) }),
				Error::<Test, Instance1>::InvalidDepositAmount
			);

			//margin value should works
			assert_ok!(LiquidityMining::deposit_lp_shares(
				GC_FARM,
				yield_farm_id,
				BSX_TKN1_AMM,
				crate::MIN_DEPOSIT,
				|_, _, _| { Ok(crate::MIN_DEPOSIT) }
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		})
	});
}

#[test]
fn deposit_lp_shares_non_existing_yield_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			assert_noop!(
				LiquidityMining::deposit_lp_shares(GC_FARM, BSX_DOT_YIELD_FARM_ID, BSX_DOT_AMM, 10_000, |_, _, _| {
					Ok(10_u128)
				}),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn deposit_lp_shares_stop_yield_farm_should_not_work() {
	let _ = predefined_test_ext_with_deposits().execute_with(|| {
		with_transaction(|| {
			assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

			assert_noop!(
				LiquidityMining::deposit_lp_shares(
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					10_000,
					|_, _, _| { Ok(10_u128) }
				),
				Error::<Test, Instance1>::LiquidityMiningCanceled
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		})
	});
}

#[test]
fn deposit_lp_shares_should_not_work_when_valued_shares_are_bellow_min_deposit() {
	let _ = predefined_test_ext_with_deposits().execute_with(|| {
		with_transaction(|| {
			assert_noop!(
				LiquidityMining::deposit_lp_shares(
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					BSX_TKN1_AMM,
					100_000,
					|_, _, _| { Ok(MIN_DEPOSIT - 1) }
				),
				Error::<Test, Instance1>::IncorrectValuedShares
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		})
	});
}
