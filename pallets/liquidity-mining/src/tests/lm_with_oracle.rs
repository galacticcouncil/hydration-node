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

use crate::tests::mock::LiquidityMining3;

//This test is using dummy oracle for price_adjustment. DummyOracle always returns .5 as
//price_adjusment.
#[test]
fn non_full_farm_should_pay_rewards_with_half_speed_when_price_adjustmnet_is_from_dummy_oracle() {
	new_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			const GLOBAL_FARM: GlobalFarmId = 1;
			const YIELD_FARM_A: YieldFarmId = 2;
			const YIELD_FARM_B: YieldFarmId = 3;

			const ALICE_DEPOSIT: DepositId = 1;
			const BOB_DEPOSIT: DepositId = 2;
			const CHARLIE_DEPOSIT: DepositId = 3;

			const TOTAL_REWARDS: u128 = 200_000 * ONE;

			//initialize farms
			set_block_number(100);
			assert_ok!(LiquidityMining3::create_global_farm(
				TOTAL_REWARDS,
				20,
				10,
				BSX,
				BSX,
				GC,
				Perquintill::from_float(0.5),
				1_000,
				One::one(),
			));

			assert_ok!(LiquidityMining3::create_yield_farm(
				GC,
				GLOBAL_FARM,
				FixedU128::from(2_u128),
				None,
				BSX_TKN1_AMM,
				vec![BSX, TKN1],
			));

			assert_ok!(LiquidityMining3::create_yield_farm(
				GC,
				GLOBAL_FARM,
				FixedU128::from(1_u128),
				None,
				BSX_TKN2_AMM,
				vec![BSX, TKN2],
			));

			set_block_number(120);
			//alice
			assert_ok!(LiquidityMining3::deposit_lp_shares(
				GLOBAL_FARM,
				YIELD_FARM_A,
				BSX_TKN1_AMM,
				5_000 * ONE,
				|_, _, _| { Ok(5_000 * ONE) }
			));

			//bob
			assert_ok!(LiquidityMining3::deposit_lp_shares(
				GLOBAL_FARM,
				YIELD_FARM_B,
				BSX_TKN2_AMM,
				2_500 * ONE,
				|_, _, _| { Ok(2_500 * ONE) }
			));

			//charlie
			assert_ok!(LiquidityMining3::deposit_lp_shares(
				GLOBAL_FARM,
				YIELD_FARM_B,
				BSX_TKN2_AMM,
				2_500 * ONE,
				|_, _, _| { Ok(2_500 * ONE) }
			));

			set_block_number(401);

			let alice_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
			let bob_bsx_balance_0 = Tokens::free_balance(BSX, &BOB);
			let charlie_bsx_balance_0 = Tokens::free_balance(BSX, &CHARLIE);

			let (_, _, _, unclaimable) =
				LiquidityMining3::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
			assert_eq!(unclaimable, 0);
			assert_ok!(LiquidityMining3::withdraw_lp_shares(
				ALICE_DEPOSIT,
				YIELD_FARM_A,
				unclaimable
			));

			let (_, _, _, unclaimable) =
				LiquidityMining3::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
			assert_eq!(unclaimable, 0);
			assert_ok!(LiquidityMining3::withdraw_lp_shares(
				BOB_DEPOSIT,
				YIELD_FARM_B,
				unclaimable
			));

			let (_, _, _, unclaimable) =
				LiquidityMining3::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
			assert_eq!(unclaimable, 0);
			assert_ok!(LiquidityMining3::withdraw_lp_shares(
				CHARLIE_DEPOSIT,
				YIELD_FARM_B,
				unclaimable
			));

			let alice_claimed = Tokens::free_balance(BSX, &ALICE) - alice_bsx_balance_0;
			let bob_claimed = Tokens::free_balance(BSX, &BOB) - bob_bsx_balance_0;
			let charlie_claimed = Tokens::free_balance(BSX, &CHARLIE) - charlie_bsx_balance_0;

			assert_eq!(alice_claimed, 70_000 * ONE);
			assert_eq!(bob_claimed, 17_500 * ONE);
			assert_eq!(charlie_claimed, 17_500 * ONE);

			let claimed_total = alice_claimed + bob_claimed + charlie_claimed;

			assert_eq!(claimed_total.abs_diff(TOTAL_REWARDS), 95_000 * ONE);

			let yield_farm_a_claimed = alice_claimed;
			let yield_farm_b_claimed = bob_claimed + charlie_claimed;

			const TOLERANCE: u128 = 10;
			assert!(
				yield_farm_a_claimed.abs_diff(2 * yield_farm_b_claimed).le(&TOLERANCE),
				"yield_farm_a_claimed == 2 * yield_farm_b_claimed"
			);

			assert!(
				alice_claimed.abs_diff(4 * bob_claimed).le(&TOLERANCE),
				"alice_claimed == 4 * bob_claimed"
			);

			assert_eq!(bob_claimed, charlie_claimed, "bob_claimed == charlie_claimed");

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn oracles_price_adjustment_should_be_used_and_saved_when_oracle_is_available() {
	new_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			const GLOBAL_FARM: GlobalFarmId = 1;
			const YIELD_FARM_A: YieldFarmId = 2;

			const ALICE_DEPOSIT: DepositId = 1;

			const TOTAL_REWARDS: u128 = 200_000 * ONE;

			//Arrange
			set_block_number(100);
			assert_ok!(LiquidityMining3::create_global_farm(
				TOTAL_REWARDS,
				20,
				10,
				BSX,
				BSX,
				GC,
				Perquintill::from_float(0.5),
				1_000,
				FixedU128::from_float(5.5_f64), //default price_adjustment, won't be used.
			));

			assert_ok!(LiquidityMining3::create_yield_farm(
				GC,
				GLOBAL_FARM,
				FixedU128::from(2_u128),
				None,
				BSX_TKN1_AMM,
				vec![BSX, TKN1],
			));

			set_block_number(120);
			//alice
			assert_ok!(LiquidityMining3::deposit_lp_shares(
				GLOBAL_FARM,
				YIELD_FARM_A,
				BSX_TKN1_AMM,
				5_000 * ONE,
				|_, _, _| { Ok(5_000 * ONE) }
			));

			//Act
			set_block_number(401);
			let (_, _, claimed_amount, unclaimable) =
				LiquidityMining3::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();

			//Assert
			assert_eq!(unclaimable, 0);
			assert_eq!(claimed_amount, 70_000 * ONE);
			//NOTE: global-farm's price_adjustment should be updated
			assert_eq!(
				LiquidityMining3::global_farm(GLOBAL_FARM).unwrap().price_adjustment,
				FixedU128::from_float(0.5_f64)
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn last_saved_price_adjustment_should_be_used_when_oracle_is_not_available() {
	new_test_ext().execute_with(|| {
		let _ = with_transaction(|| {
			const GLOBAL_FARM: GlobalFarmId = 1;
			const YIELD_FARM_A: YieldFarmId = 2;

			const ALICE_DEPOSIT: DepositId = 1;

			const TOTAL_REWARDS: u128 = 200_000 * ONE;

			//Arrange
			set_block_number(100);
			assert_ok!(LiquidityMining3::create_global_farm(
				TOTAL_REWARDS,
				20,
				1,
				BSX,
				BSX,
				GC,
				Perquintill::from_float(0.5),
				1_000,
				FixedU128::from(2_u128), //default price_adjustment
			));

			assert_ok!(LiquidityMining3::create_yield_farm(
				GC,
				GLOBAL_FARM,
				FixedU128::one(),
				None,
				BSX_TKN1_AMM,
				vec![BSX, TKN1],
			));

			//NOTE: This is special period. Oracle will fail if global-farm was updated in this
			//period.
			set_block_number(999_666_333);
			//alice
			assert_ok!(LiquidityMining3::deposit_lp_shares(
				GLOBAL_FARM,
				YIELD_FARM_A,
				BSX_TKN1_AMM,
				5_000 * ONE,
				|_, _, _| { Ok(5_000 * ONE) }
			));

			assert_eq!(
				LiquidityMining3::global_farm(GLOBAL_FARM).unwrap().price_adjustment,
				FixedU128::from(2_u128)
			);
			//Act
			set_block_number(999_666_334);
			//NOTE: Oracle will fail because global_farm.updated_at == 999_666_333 when oracle is
			//called.
			let (_, _, claimed_amount, unclaimable) =
				LiquidityMining3::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();

			//Assert
			assert_eq!(unclaimable, 0);
			assert_eq!(claimed_amount, 5_000 * ONE);
			//NOTE: oracle is not available so value should not change.
			assert_eq!(
				LiquidityMining3::global_farm(GLOBAL_FARM).unwrap().price_adjustment,
				FixedU128::from(2_u128)
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
