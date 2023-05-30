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
fn terminate_global_farm_should_work() {
    //Test with flushing - global farm should be removed from storage if it has no yield farms.
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            let predefined_global_farm = get_predefined_global_farm_ins1(1);
            let farm_account = LiquidityMining::farm_account_id(BOB_FARM).unwrap();
            let bob_reward_currency_balance = Tokens::free_balance(predefined_global_farm.reward_currency, &BOB);

            let undistributed_rewards =
                Tokens::free_balance(get_predefined_global_farm_ins1(1).reward_currency, &farm_account);

            assert_eq!(
                LiquidityMining::terminate_global_farm(BOB, BOB_FARM).unwrap(),
                (
                    get_predefined_global_farm_ins1(1).reward_currency,
                    undistributed_rewards,
                    BOB
                )
            );

            //Global farm with no yield farms should be flushed.
            assert!(LiquidityMining::global_farm(BOB_FARM).is_none());

            //Undistributed rewards should be transferred to owner.
            assert_eq!(
                Tokens::free_balance(predefined_global_farm.reward_currency, &BOB),
                bob_reward_currency_balance + undistributed_rewards
            );

            //Non-dustable check
            assert_eq!(Whitelist::contains(&farm_account), false);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });

    //Without flushing - global farm should stay in the storage marked as deleted.
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            let predefined_global_farm = get_predefined_global_farm_ins1(3);
            let farm_account = LiquidityMining::farm_account_id(CHARLIE_FARM).unwrap();
            let charlie_reward_currency_balance =
                Tokens::free_balance(predefined_global_farm.reward_currency, &CHARLIE);
            let undistributed_rewards = Tokens::free_balance(predefined_global_farm.reward_currency, &farm_account);
            let yield_farm_id = PREDEFINED_YIELD_FARMS_INS1.with(|v| v[2].id);

            //Add deposit to yield farm so it will not be flushed on destroy.
            assert_ok!(LiquidityMining::deposit_lp_shares(
                CHARLIE_FARM,
                yield_farm_id,
                ACA_KSM_AMM,
                1_000 * ONE,
                |_, _, _| { Ok(10 * ONE) },
            ));

            //Stop farming.
            assert_ok!(LiquidityMining::stop_yield_farm(CHARLIE, CHARLIE_FARM, ACA_KSM_AMM));

            //Destroy yield farm (yield farm is destroyed but not flushed)
            assert_ok!(LiquidityMining::terminate_yield_farm(
                CHARLIE,
                CHARLIE_FARM,
                yield_farm_id,
                ACA_KSM_AMM
            ));

            //Destroy global farm.
            assert_ok!(LiquidityMining::terminate_global_farm(CHARLIE, CHARLIE_FARM));

            //Global farm with yield farms should NOT be flushed.
            assert_eq!(
                LiquidityMining::global_farm(CHARLIE_FARM).unwrap(),
                GlobalFarmData {
                    live_yield_farms_count: 0,
                    total_yield_farms_count: 1,
                    state: FarmState::Terminated,
                    ..predefined_global_farm
                }
            );

            assert_eq!(
                Tokens::free_balance(predefined_global_farm.reward_currency, &CHARLIE),
                charlie_reward_currency_balance + undistributed_rewards
            );

            //Farm's account should be removed when farm is destroyed.
            assert_eq!(Whitelist::contains(&farm_account), false);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    })
}

#[test]
fn terminate_global_farm_not_owner_should_not_work() {
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            assert_noop!(
                LiquidityMining::terminate_global_farm(ALICE, BOB_FARM),
                Error::<Test, Instance1>::Forbidden
            );

            assert_eq!(
                LiquidityMining::global_farm(BOB_FARM).unwrap(),
                get_predefined_global_farm_ins1(1)
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_global_farm_farm_not_exists_should_not_work() {
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const NON_EXISTING_FARM: u32 = 999_999_999;
            assert_noop!(
                LiquidityMining::terminate_global_farm(ALICE, NON_EXISTING_FARM),
                Error::<Test, Instance1>::GlobalFarmNotFound
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_global_farm_with_yield_farms_should_not_work() {
    //Global farm CAN'T be destroyed if it has active or stopped yield farms.
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            //Destroy farm with active yield farms should not work.
            let yield_farm_id = PREDEFINED_YIELD_FARMS_INS1.with(|v| v[2].id);
            assert_eq!(
                LiquidityMining::active_yield_farm(ACA_KSM_AMM, CHARLIE_FARM).unwrap(),
                yield_farm_id
            );

            assert_noop!(
                LiquidityMining::terminate_global_farm(CHARLIE, CHARLIE_FARM),
                Error::<Test, Instance1>::GlobalFarmIsNotEmpty
            );

            assert_eq!(
                LiquidityMining::global_farm(CHARLIE_FARM).unwrap(),
                get_predefined_global_farm_ins1(3)
            );

            //Destroy farm with stopped yield farms should not work.
            //Stop yield farm
            assert_ok!(LiquidityMining::stop_yield_farm(CHARLIE, CHARLIE_FARM, ACA_KSM_AMM));
            assert!(LiquidityMining::active_yield_farm(ACA_KSM_AMM, CHARLIE_FARM).is_none());

            assert_noop!(
                LiquidityMining::terminate_global_farm(CHARLIE, CHARLIE_FARM),
                Error::<Test, Instance1>::GlobalFarmIsNotEmpty
            );

            assert_eq!(
                LiquidityMining::global_farm(CHARLIE_FARM).unwrap(),
                get_predefined_global_farm_ins1(3)
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_global_farm_healthy_farm_should_not_work() {
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            let farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let predefined_global_farm = get_predefined_global_farm_ins1(2);
            assert!(!Tokens::free_balance(predefined_global_farm.reward_currency, &farm_account).is_zero());

            assert_noop!(
                LiquidityMining::terminate_global_farm(GC, GC_FARM),
                Error::<Test, Instance1>::GlobalFarmIsNotEmpty
            );

            assert_eq!(LiquidityMining::global_farm(GC_FARM).unwrap(), predefined_global_farm);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn terminate_global_farm_should_fail_with_global_farm_not_found_when_farm_is_already_terminated() {
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            //Arrange
            let yield_farm_id = PREDEFINED_YIELD_FARMS_INS1.with(|v| v[2].id);

            //Add deposit to yield farm so it will not be flushed on destroy.
            assert_ok!(LiquidityMining::deposit_lp_shares(
                CHARLIE_FARM,
                yield_farm_id,
                ACA_KSM_AMM,
                1_000 * ONE,
                |_, _, _| { Ok(10 * ONE) },
            ));

            //Stop farming.
            assert_ok!(LiquidityMining::stop_yield_farm(CHARLIE, CHARLIE_FARM, ACA_KSM_AMM));

            //Destroy yield farm (yield farm is destroyed but not flushed)
            assert_ok!(LiquidityMining::terminate_yield_farm(
                CHARLIE,
                CHARLIE_FARM,
                yield_farm_id,
                ACA_KSM_AMM
            ));

            //Destroy global farm.
            assert_ok!(LiquidityMining::terminate_global_farm(CHARLIE, CHARLIE_FARM));

            //Global farm with yield farms should NOT be flushed.
            pretty_assertions::assert_eq!(LiquidityMining::global_farm(CHARLIE_FARM).is_some(), true);

            //Act & assert
            assert_noop!(
                LiquidityMining::terminate_global_farm(CHARLIE, CHARLIE_FARM),
                Error::<Test, Instance1>::GlobalFarmNotFound
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    })
}
