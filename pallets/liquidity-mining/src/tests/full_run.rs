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

use crate::tests::mock::LiquidityMining2;
use test_utils::assert_balance_approx;

use rand::Rng;

//This test test full run LM. Global farm is not full but it's running longer than expected. Users
//should be able to claim expected amount.
//This test case is without loyalty factor.
#[test]
fn non_full_farm_running_longer_than_expected() {
    new_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const GLOBAL_FARM: GlobalFarmId = 1;
            const YIELD_FARM_A: YieldFarmId = 2;
            const YIELD_FARM_B: YieldFarmId = 3;

            const ALICE_DEPOSIT: DepositId = 1;
            const BOB_DEPOSIT: DepositId = 2;
            const CHARLIE_DEPOSIT: DepositId = 3;

            //initialize farms
            set_block_number(100);
            assert_ok!(LiquidityMining2::create_global_farm(
                200_000 * ONE,
                20,
                10,
                BSX,
                BSX,
                GC,
                Perquintill::from_float(0.5),
                1_000,
                One::one(),
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(2_u128),
                None,
                BSX_TKN1_AMM,
                vec![BSX, TKN1],
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(1_u128),
                None,
                BSX_TKN2_AMM,
                vec![BSX, TKN2],
            ));

            set_block_number(120);
            //alice
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_A,
                BSX_TKN1_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            //bob
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                2_500 * ONE,
                |_, _, _| { Ok(2_500 * ONE) }
            ));

            //charlie
            assert_ok!(LiquidityMining2::deposit_lp_shares(
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
                LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                ALICE_DEPOSIT,
                YIELD_FARM_A,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                BOB_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                CHARLIE_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let alice_claimed = Tokens::free_balance(BSX, &ALICE) - alice_bsx_balance_0;
            let bob_claimed = Tokens::free_balance(BSX, &BOB) - bob_bsx_balance_0;
            let charlie_claimed = Tokens::free_balance(BSX, &CHARLIE) - charlie_bsx_balance_0;

            let claimed_total = alice_claimed + bob_claimed + charlie_claimed;

            assert_eq!(claimed_total.abs_diff(200_000 * ONE), 1002);

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

//This test tests `update_global_farm` and `update_yield_farm` after farm distributed all the
//rewards.
#[test]
fn non_full_farm_distribute_everything_and_update_farms() {
    new_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const GLOBAL_FARM: GlobalFarmId = 1;
            const YIELD_FARM_A: YieldFarmId = 2;
            const YIELD_FARM_B: YieldFarmId = 3;

            const ALICE_DEPOSIT: DepositId = 1;
            const BOB_DEPOSIT: DepositId = 2;
            const CHARLIE_DEPOSIT: DepositId = 3;

            //initialize farms
            set_block_number(100);
            assert_ok!(LiquidityMining2::create_global_farm(
                200_000 * ONE,
                20,
                10,
                BSX,
                BSX,
                GC,
                Perquintill::from_float(0.5),
                1_000,
                One::one(),
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(2_u128),
                None,
                BSX_TKN1_AMM,
                vec![BSX, TKN1],
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(1_u128),
                None,
                BSX_TKN2_AMM,
                vec![BSX, TKN2],
            ));

            set_block_number(120);
            //alice
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_A,
                BSX_TKN1_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            set_block_number(140);
            //bob
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                2_500 * ONE,
                |_, _, _| { Ok(2_500 * ONE) }
            ));

            //charlie
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                2_500 * ONE,
                |_, _, _| { Ok(2_500 * ONE) }
            ));

            set_block_number(401);

            //last farms update and claim everything
            let _ = LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();

            let _ = LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();

            let _ = LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();

            assert_eq!(
                Tokens::free_balance(BSX, &LiquidityMining2::farm_account_id(GLOBAL_FARM).unwrap()),
                1_000
            );

            assert_eq!(
                Tokens::free_balance(BSX, &LiquidityMining2::pot_account_id().unwrap()),
                0
            );

            set_block_number(501);
            let (_, _, claimed, unclaimable) =
                LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();

            assert_eq!(claimed, 0);
            assert_eq!(unclaimable, 0);

            assert_eq!(LiquidityMining2::global_farm(GLOBAL_FARM).unwrap().updated_at, 50);
            assert_eq!(
                LiquidityMining2::yield_farm((BSX_TKN1_AMM, GLOBAL_FARM, YIELD_FARM_A))
                    .unwrap()
                    .updated_at,
                50
            );

            set_block_number(1000);
            let _ = LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(LiquidityMining2::global_farm(GLOBAL_FARM).unwrap().updated_at, 100);
            assert_eq!(
                LiquidityMining2::yield_farm((BSX_TKN2_AMM, GLOBAL_FARM, YIELD_FARM_B))
                    .unwrap()
                    .updated_at,
                100
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

//This test tests `update_global_farm` and `update_yield_farm` after farm distributed all the
//rewards.
#[test]
fn overcrowded_farm_running_longer_than_expected() {
    new_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const GLOBAL_FARM: GlobalFarmId = 1;
            const YIELD_FARM_A: YieldFarmId = 2;
            const YIELD_FARM_B: YieldFarmId = 3;

            const ALICE_DEPOSIT: DepositId = 1;
            const BOB_DEPOSIT: DepositId = 2;
            const CHARLIE_DEPOSIT: DepositId = 3;

            //initialize farms
            set_block_number(100);
            assert_ok!(LiquidityMining2::create_global_farm(
                200_000 * ONE,
                20,
                10,
                BSX,
                BSX,
                GC,
                Perquintill::from_float(0.5),
                1_000,
                One::one(),
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(2_u128),
                None,
                BSX_TKN1_AMM,
                vec![BSX, TKN1],
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(1_u128),
                None,
                BSX_TKN2_AMM,
                vec![BSX, TKN2],
            ));

            //NOTE: farm is overcrowded when Z > 20_000
            set_block_number(120);
            //alice
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_A,
                BSX_TKN1_AMM,
                10_000 * ONE,
                |_, _, _| { Ok(10_000 * ONE) }
            ));

            //bob
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            //charlie
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            let mut block_number = 131;

            let alice_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
            let bob_bsx_balance_0 = Tokens::free_balance(BSX, &BOB);
            let charlie_bsx_balance_0 = Tokens::free_balance(BSX, &CHARLIE);

            let mut last_alice_balance = alice_bsx_balance_0;
            let mut last_bob_balance = bob_bsx_balance_0;
            let mut last_charlie_balance = charlie_bsx_balance_0;
            //NOTE: we must be able to pay at least for 20 periods (131 + (20 * 10))
            while block_number < 331 {
                set_block_number(block_number);

                //alice
                let _ = LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
                assert!(Tokens::free_balance(BSX, &ALICE).gt(&last_alice_balance));
                last_alice_balance = Tokens::free_balance(BSX, &ALICE);

                //bob
                let _ = LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
                assert!(Tokens::free_balance(BSX, &BOB).gt(&last_bob_balance));
                last_bob_balance = Tokens::free_balance(BSX, &BOB);

                //charlie
                let _ = LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
                assert!(Tokens::free_balance(BSX, &CHARLIE).gt(&last_charlie_balance));
                last_charlie_balance = Tokens::free_balance(BSX, &CHARLIE);

                block_number += 10;
            }

            set_block_number(401);

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                ALICE_DEPOSIT,
                YIELD_FARM_A,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                BOB_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                CHARLIE_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let alice_claimed = Tokens::free_balance(BSX, &ALICE) - alice_bsx_balance_0;
            let bob_claimed = Tokens::free_balance(BSX, &BOB) - bob_bsx_balance_0;
            let charlie_claimed = Tokens::free_balance(BSX, &CHARLIE) - charlie_bsx_balance_0;

            let claimed_total = alice_claimed + bob_claimed + charlie_claimed;

            assert_eq!((200_000 * ONE) - claimed_total, 1_020); //NOTE: ED = 1_000

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

//Note: This test is running for like 4 min. and run it with `--nocapture` to see progress.
#[ignore = "This test takes too much time."]
#[test]
fn full_farm_running_planned_time() {
    new_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const GLOBAL_FARM: GlobalFarmId = 1;
            const YIELD_FARM_A: YieldFarmId = 2;
            const YIELD_FARM_B: YieldFarmId = 3;

            const ALICE_DEPOSIT: DepositId = 1;
            const BOB_DEPOSIT: DepositId = 2;
            const CHARLIE_DEPOSIT: DepositId = 3;

            const PLANNED_PERIODS: u64 = 525_600; //1 year with 10 blocks per period and 6s blocktime.
            const BLOCKS_PER_PERIOD: u64 = 10;
            const TOTAL_REWARDS_TO_DISTRIBUTE: u128 = 5_256_000 * ONE;

            //initialize farms
            set_block_number(100);
            //NOTE: This farm is distributing 10BSX per period(10block) for 1 year on chain with 6s
            //blocktime if it's full. This farm is full when Z(locked bsx value) = 20_000.
            assert_ok!(LiquidityMining2::create_global_farm(
                TOTAL_REWARDS_TO_DISTRIBUTE,
                PLANNED_PERIODS, //1 year, 6s blocktime
                BLOCKS_PER_PERIOD,
                BSX,
                BSX,
                GC,
                Perquintill::from_float(0.000_5),
                1_000,
                One::one(),
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(2_u128),
                None,
                BSX_TKN1_AMM,
                vec![BSX, TKN1]
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                FixedU128::from(1_u128),
                None,
                BSX_TKN2_AMM,
                vec![BSX, TKN2]
            ));

            set_block_number(120);
            //alice
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_A,
                BSX_TKN1_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            //bob
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            //charlie
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                5_000 * ONE,
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            let alice_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
            let bob_bsx_balance_0 = Tokens::free_balance(BSX, &BOB);
            let charlie_bsx_balance_0 = Tokens::free_balance(BSX, &CHARLIE);

            let mut last_alice_balance = alice_bsx_balance_0;
            let mut last_bob_balance = bob_bsx_balance_0;
            let mut last_charlie_balance = charlie_bsx_balance_0;

            //NOTE: This farm should distribute rewards for at leas 525_600 periods
            let mut current_block = 121;
            let last_rewarded_period = current_block + PLANNED_PERIODS * BLOCKS_PER_PERIOD - BLOCKS_PER_PERIOD;
            let mut rng = rand::thread_rng();
            let mut i: u32 = 0;
            while current_block <= last_rewarded_period {
                current_block += BLOCKS_PER_PERIOD;
                set_block_number(current_block);

                match rng.gen_range(1..=3) {
                    1 => {
                        //alice
                        let (_, _, _, unclaimable) =
                            LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
                        assert!(Tokens::free_balance(BSX, &ALICE).gt(&last_alice_balance));
                        assert_eq!(unclaimable, 0);
                        last_alice_balance = Tokens::free_balance(BSX, &ALICE);
                    }
                    2 => {
                        //Bob
                        let (_, _, _, unclaimable) =
                            LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
                        assert!(Tokens::free_balance(BSX, &BOB).gt(&last_bob_balance));
                        assert_eq!(unclaimable, 0);
                        last_bob_balance = Tokens::free_balance(BSX, &BOB);
                    }
                    x => {
                        //charlie
                        let (_, _, _, unclaimable) =
                            LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
                        assert!(Tokens::free_balance(BSX, &CHARLIE).gt(&last_charlie_balance));
                        assert_eq!(unclaimable, 0);
                        last_charlie_balance = Tokens::free_balance(BSX, &CHARLIE);
                        assert!(x == 3);
                    }
                }

                i += 1;
                if i % 50_000 == 0 {
                    println!("periods: {i}");
                }
            }

            set_block_number(last_rewarded_period + 100);
            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                ALICE_DEPOSIT,
                YIELD_FARM_A,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                BOB_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                CHARLIE_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let alice_claimed = Tokens::free_balance(BSX, &ALICE) - alice_bsx_balance_0;
            let bob_claimed = Tokens::free_balance(BSX, &BOB) - bob_bsx_balance_0;
            let charlie_claimed = Tokens::free_balance(BSX, &CHARLIE) - charlie_bsx_balance_0;

            let claimed_total = alice_claimed + bob_claimed + charlie_claimed;

            assert_eq!(TOTAL_REWARDS_TO_DISTRIBUTE - claimed_total, 1_000);

            let yield_farm_a_claimed = alice_claimed;
            let yield_farm_b_claimed = bob_claimed + charlie_claimed;

            const TOLERANCE: u128 = 10;
            assert!(
                yield_farm_a_claimed.abs_diff(yield_farm_b_claimed).le(&TOLERANCE),
                "yield_farm_a_claimed == yield_farm_b_claimed"
            );

            assert!(
                alice_claimed.abs_diff(2 * bob_claimed).le(&TOLERANCE),
                "alice_claimed == 2 * bob_claimed"
            );

            assert_eq!(bob_claimed, charlie_claimed, "bob_claimed == charlie_claimed");

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

// This tests that yield farm claims expected amount from global farm.
#[test]
fn yield_farm_should_claim_expected_amount() {
    new_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const GLOBAL_FARM: GlobalFarmId = 1;
            const YIELD_FARM_A: YieldFarmId = 2;
            const YIELD_FARM_B: YieldFarmId = 3;

            const ALICE_DEPOSIT: DepositId = 1;
            const BOB_DEPOSIT: DepositId = 2;
            const CHARLIE_DEPOSIT: DepositId = 3;

            const PLANNED_PERIODS: u64 = 10_000;
            const BLOCKS_PER_PERIOD: u64 = 10;
            const TOTAL_REWARDS_TO_DISTRIBUTE: u128 = 1_000_000 * ONE;

            let yield_farm_a_key = (BSX_TKN1_AMM, GLOBAL_FARM, YIELD_FARM_A);
            let yield_farm_b_key = (BSX_TKN2_AMM, GLOBAL_FARM, YIELD_FARM_B);

            //initialize farms
            set_block_number(1000);
            assert_ok!(LiquidityMining2::create_global_farm(
                TOTAL_REWARDS_TO_DISTRIBUTE,
                PLANNED_PERIODS,
                BLOCKS_PER_PERIOD,
                BSX,
                BSX,
                GC,
                Perquintill::from_float(0.005),
                1_000,
                One::one(),
            ));

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                One::one(),
                None,
                BSX_TKN1_AMM,
                vec![BSX, TKN1]
            ));

            //alice
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_A,
                BSX_TKN1_AMM,
                10_000 * ONE,
                |_, _, _| { Ok(10_000 * ONE) }
            ));

            set_block_number(1_500);

            assert_ok!(LiquidityMining2::create_yield_farm(
                GC,
                GLOBAL_FARM,
                One::one(),
                None,
                BSX_TKN2_AMM,
                vec![BSX, TKN2]
            ));

            set_block_number(2_000);
            //bob
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                10_000 * ONE,
                |_, _, _| { Ok(10_000 * ONE) }
            ));

            set_block_number(2_500);
            //charlie
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                10_000 * ONE,
                |_, _, _| { Ok(10_000 * ONE) }
            ));

            let pot = LiquidityMining2::pot_account_id().unwrap();
            assert_eq!(
                LiquidityMining2::yield_farm(yield_farm_a_key)
                    .unwrap()
                    .left_to_distribute,
                0
            );
            assert_eq!(
                LiquidityMining2::yield_farm(yield_farm_b_key)
                    .unwrap()
                    .left_to_distribute,
                2_500 * ONE
            );
            assert_eq!(Tokens::free_balance(BSX, &pot), 10_000 * ONE);

            //Global farm had rewards for 100_000 blocks.
            set_block_number(120_000);

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(ALICE, ALICE_DEPOSIT, YIELD_FARM_A, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                ALICE_DEPOSIT,
                YIELD_FARM_A,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                BOB_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let (_, _, _, unclaimable) =
                LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();
            assert_eq!(unclaimable, 0);
            assert_ok!(LiquidityMining2::withdraw_lp_shares(
                CHARLIE_DEPOSIT,
                YIELD_FARM_B,
                unclaimable
            ));

            let global_farm_account = LiquidityMining2::farm_account_id(GLOBAL_FARM).unwrap();
            //leftovers in the pot because of rounding errors
            assert_balance_approx!(pot, BSX, 0, 2);

            assert_eq!(
                LiquidityMining2::yield_farm(yield_farm_a_key)
                    .unwrap()
                    .left_to_distribute,
                0
            );
            assert_eq!(
                LiquidityMining2::yield_farm(yield_farm_b_key)
                    .unwrap()
                    .left_to_distribute,
                1
            );
            assert_eq!(Tokens::free_balance(BSX, &global_farm_account), 1_000);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}
