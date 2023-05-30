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
fn stop_yield_farm_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            let yield_farm_account = LiquidityMining::farm_account_id(GC_BSX_TKN1_YIELD_FARM_ID).unwrap();
            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let yield_farm_bsx_balance = Tokens::free_balance(BSX, &yield_farm_account);
            let global_farm_bsx_balance = Tokens::free_balance(BSX, &global_farm_account);
            let yield_farm = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert!(yield_farm.state.is_active());

            assert_eq!(
                LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM).unwrap(),
                yield_farm.id
            );

            let stake_in_global_farm = yield_farm
                .multiplier
                .checked_mul_int(yield_farm.total_valued_shares)
                .unwrap();

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    state: FarmState::Stopped,
                    multiplier: 0.into(),
                    ..yield_farm
                }
            );

            assert!(LiquidityMining::active_yield_farm(BSX_TKN1_AMM, GC_FARM).is_none());

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    total_shares_z: global_farm.total_shares_z.checked_sub(stake_in_global_farm).unwrap(),
                    ..global_farm
                }
            );

            assert_eq!(Tokens::free_balance(BSX, &yield_farm_account), yield_farm_bsx_balance);
            assert_eq!(Tokens::free_balance(BSX, &global_farm_account), global_farm_bsx_balance);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });

    //Cancel yield farming with farms update.
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let pot = LiquidityMining::pot_account_id().unwrap();

            //_0 - value before act.
            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);
            let global_balance_0 = Tokens::free_balance(BSX, &global_farm_account);

            let last_yield_farm_rewards = 8_538_750 * ONE;
            let allocated_for_other_yield_farms = 17_860_875 * ONE;

            assert!(yield_farm_0.state.is_active());

            set_block_number(10_000);

            assert_eq!(
                LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM).unwrap(),
                yield_farm_0.id
            );

            let stake_in_global_farm = yield_farm_0
                .multiplier
                .checked_mul_int(yield_farm_0.total_valued_shares)
                .unwrap();

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 100,
                    accumulated_rpvs: FixedU128::from_inner(205_000_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(41_000_000_000_000_000_000_u128),
                    state: FarmState::Stopped,
                    multiplier: 0.into(),
                    left_to_distribute: yield_farm_0.left_to_distribute + last_yield_farm_rewards,
                    ..yield_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 100,
                    accumulated_rpz: FixedU128::from_inner(41_000_000_000_000_000_000_u128),
                    total_shares_z: global_farm_0.total_shares_z.checked_sub(stake_in_global_farm).unwrap(),
                    pending_rewards: global_farm_0.pending_rewards + allocated_for_other_yield_farms,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards + last_yield_farm_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                Tokens::free_balance(BSX, &pot),
                pot_balance_0 + last_yield_farm_rewards + allocated_for_other_yield_farms
            );

            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_balance_0 - last_yield_farm_rewards - allocated_for_other_yield_farms
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn stop_yield_farm_invalid_yield_farm_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            assert_noop!(
                LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_DOT_AMM),
                Error::<Test, Instance1>::YieldFarmNotFound
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn stop_yield_farm_liquidity_mining_already_canceled() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            //1-th stop should pass ok.
            assert_eq!(
                LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM).unwrap(),
                GC_BSX_TKN1_YIELD_FARM_ID
            );

            assert_noop!(
                LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM),
                Error::<Test, Instance1>::YieldFarmNotFound
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn stop_yield_farm_not_owner_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const NOT_FARMS_OWNER: u128 = ALICE;

            assert_noop!(
                LiquidityMining::stop_yield_farm(NOT_FARMS_OWNER, GC_FARM, BSX_TKN1_AMM),
                Error::<Test, Instance1>::Forbidden
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}
