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
fn terminate_yield_farm_with_deposits_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let global_farm_bsx_balance = Tokens::free_balance(BSX, &global_farm_account);

            let pot = LiquidityMining::pot_account_id().unwrap();
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);

            // Cancel yield farm before removing.
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();
            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();

            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    live_yield_farms_count: global_farm_0.live_yield_farms_count.checked_sub(1).unwrap(),
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - yield_farm_0.left_to_distribute,
                    ..global_farm_0
                }
            );

            //Yield farm is removed from storage only if there are no more farm entries.
            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    state: FarmState::Terminated,
                    left_to_distribute: 0,
                    ..yield_farm_0
                }
            );

            //Yield-farm's `left_to_distribute`(unpaid rewards) should be transferred from pot to
            //global-farm's account.
            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_farm_bsx_balance
                    .checked_add(yield_farm_0.left_to_distribute)
                    .unwrap()
            );

            assert_eq!(
                Tokens::free_balance(BSX, &pot),
                pot_balance_0.checked_sub(yield_farm_0.left_to_distribute).unwrap()
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_yield_farm_without_deposits_should_work() {
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let yield_farm_account = LiquidityMining::farm_account_id(GC_BSX_TKN1_YIELD_FARM_ID).unwrap();

            let yield_farm_bsx_balance = Tokens::free_balance(BSX, &yield_farm_account);
            let global_farm_bsx_balance = Tokens::free_balance(BSX, &global_farm_account);

            //Stop yield farm before removing
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    live_yield_farms_count: global_farm.live_yield_farms_count.checked_sub(1).unwrap(),
                    //yield farm was removed from storage so this should change
                    total_yield_farms_count: global_farm.total_yield_farms_count.checked_sub(1).unwrap(),
                    ..global_farm
                }
            );

            //Yield farm without deposits should be flushed.
            assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).is_none());

            assert_eq!(Tokens::free_balance(BSX, &yield_farm_account), 0);

            //Unpaid rewards from yield farm account should be transferred back to global farm's account.
            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_farm_bsx_balance.checked_add(yield_farm_bsx_balance).unwrap()
            );

            //Non-dustable check
            assert_eq!(Whitelist::contains(&yield_farm_account), false);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_yield_farm_not_stopped_yield_farming_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            assert_noop!(
                LiquidityMining::terminate_yield_farm(GC, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID, BSX_TKN1_AMM),
                Error::<Test, Instance1>::LiquidityMiningIsActive
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_yield_farm_not_owner_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const NOT_OWNER: u128 = ALICE;

            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            assert_noop!(
                LiquidityMining::terminate_yield_farm(NOT_OWNER, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID, BSX_TKN1_AMM),
                Error::<Test, Instance1>::Forbidden
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_yield_farm_yield_farm_does_not_exists_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            assert_noop!(
                LiquidityMining::terminate_yield_farm(GC, GC_FARM, BSX_DOT_YIELD_FARM_ID, BSX_DOT_AMM),
                Error::<Test, Instance1>::YieldFarmNotFound
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_yield_farm_should_work_when_farm_is_stopped_and_active_yield_farm_exists_for_same_amm_pool_id() {
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let yield_farm_acoount = LiquidityMining::farm_account_id(GC_BSX_TKN1_YIELD_FARM_ID).unwrap();

            let yield_farm_bsx_balance = Tokens::free_balance(BSX, &yield_farm_acoount);
            let global_farm_bsx_balance = Tokens::free_balance(BSX, &global_farm_account);

            //Stop yield farm before removing
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            LiquidityMining::create_yield_farm(GC, GC_FARM, One::one(), None, BSX_TKN1_AMM, vec![BSX, TKN1]).unwrap();

            let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    live_yield_farms_count: global_farm.live_yield_farms_count.checked_sub(1).unwrap(),
                    //yield farm was removed from storage so this should change
                    total_yield_farms_count: global_farm.total_yield_farms_count.checked_sub(1).unwrap(),
                    ..global_farm
                }
            );

            //Yield farm without deposits should be flushed.
            assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).is_none());

            assert_eq!(Tokens::free_balance(BSX, &yield_farm_acoount), 0);

            //Unpaid rewards from yield farm account should be transferred back to global farm's account.
            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_farm_bsx_balance.checked_add(yield_farm_bsx_balance).unwrap()
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}
