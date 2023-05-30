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
use crate::tests::mock::LiquidityMining2;
use pretty_assertions::assert_eq;
use test_ext::*;

#[test]
fn claim_rewards_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            const REWARD_CURRENCY: AssetId = BSX;
            let global_farm_id = GC_FARM;
            let pot = LiquidityMining::pot_account_id().unwrap();
            let global_farm_account = LiquidityMining::farm_account_id(global_farm_id).unwrap();
            let global_farm_total_rewards_start = 30_000_000_000 * ONE;

            //_0 - value before act.
            let alice_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
            let bsx_tkn1_yield_farm_key = (BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID);
            let bsx_tkn2_yield_farm_key = (BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID);
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let yield_farm_0 = LiquidityMining::yield_farm(bsx_tkn1_yield_farm_key).unwrap();

            let expected_claimed_rewards = 23_306_074_766_355_140;
            let unclaimable_rewards = 20_443_925_233_644_860;

            //claim A1.1  (dep. A1 1-th time)
            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (
                    global_farm_id,
                    REWARD_CURRENCY,
                    expected_claimed_rewards,
                    unclaimable_rewards
                )
            );

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap(),
                DepositData {
                    shares: 50 * ONE,
                    amm_pool_id: BSX_TKN1_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_rpvs: Zero::zero(),
                        accumulated_claimed_rewards: expected_claimed_rewards,
                        entered_at: 18,
                        updated_at: 25,
                        valued_shares: 2_500 * ONE,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            assert_eq!(
                LiquidityMining::yield_farm(bsx_tkn1_yield_farm_key)
                    .unwrap()
                    .left_to_distribute,
                yield_farm_0.left_to_distribute - expected_claimed_rewards
            );

            //Check if claimed rewards are transferred.
            assert_eq!(
                Tokens::free_balance(BSX, &ALICE),
                alice_bsx_balance_0 + expected_claimed_rewards
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - expected_claimed_rewards
            );

            // claim B3.1
            set_block_number(3_056);
            //_0 - value before act.
            let alice_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let yield_farm_0 = LiquidityMining::yield_farm(bsx_tkn2_yield_farm_key).unwrap();

            let expected_claimed_rewards = 3_417_857_142_857_142;
            let unclaimable_rewards = 3_107_142_857_142_858;
            let reserved_for_both_farms = 1_759_975 * ONE;
            let claimed_from_global = 1_190_725 * ONE;

            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[4],
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (
                    global_farm_id,
                    REWARD_CURRENCY,
                    expected_claimed_rewards,
                    unclaimable_rewards
                )
            );

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[4]).unwrap(),
                DepositData {
                    shares: 87 * ONE,
                    amm_pool_id: BSX_TKN2_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: GC_BSX_TKN2_YIELD_FARM_ID,
                        valued_shares: 261 * ONE,
                        accumulated_rpvs: FixedU128::from(35),
                        accumulated_claimed_rewards: expected_claimed_rewards,
                        entered_at: 25,
                        updated_at: 30,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 30,
                    accumulated_rpz: FixedU128::from(6),
                    total_shares_z: 703_990 * ONE,
                    pending_rewards: 569_250 * ONE,
                    accumulated_paid_rewards: 2_474_275 * ONE,
                    ..get_predefined_global_farm_ins1(2)
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, global_farm_id, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 30,
                    accumulated_rpvs: FixedU128::from(60),
                    accumulated_rpz: FixedU128::from(6),
                    total_shares: 960 * ONE,
                    total_valued_shares: 47_629 * ONE,
                    entries_count: 4,
                    left_to_distribute: yield_farm_0.left_to_distribute - expected_claimed_rewards
                        + claimed_from_global,
                    ..yield_farm_0
                },
            );

            //Check if claimed rewards are transferred.
            assert_eq!(
                Tokens::free_balance(BSX, &ALICE),
                alice_bsx_balance_0 + expected_claimed_rewards
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 + reserved_for_both_farms - expected_claimed_rewards
            );

            //Run for log time(longer than planned_yielding_periods) without interactions with farms.
            //planned_yielding_periods = 500; 100 blocks per period
            //claim A1.2
            set_block_number(125_879);
            //_0 - value before act.
            let alice_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
            let bsx_tkn1_yield_farm_key = (BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID);
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let yield_farm_0 = LiquidityMining::yield_farm(bsx_tkn1_yield_farm_key).unwrap();

            let expected_claimed_rewards = 7_437_514_820_756_032_916;
            let unclaimable_rewards = 289_179_104_477_611_944;

            let reserved_for_both_farms = 432_249_860 * ONE;
            let yield_farm_claim_from_global = 140_377_050 * ONE;

            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (
                    global_farm_id,
                    REWARD_CURRENCY,
                    expected_claimed_rewards,
                    unclaimable_rewards
                )
            );

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap(),
                DepositData {
                    shares: 50 * ONE,
                    amm_pool_id: BSX_TKN1_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        valued_shares: 2_500 * ONE,
                        accumulated_rpvs: Zero::zero(),
                        accumulated_claimed_rewards: 7_460_820_895_522_388_056,
                        entered_at: 18,
                        updated_at: 1_258,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            let global_farm_1 = LiquidityMining::global_farm(GC_FARM).unwrap();
            pretty_assertions::assert_eq!(
                global_farm_1,
                GlobalFarmData {
                    updated_at: 1_258,
                    accumulated_rpz: FixedU128::from(620),
                    total_shares_z: 703_990 * ONE,
                    pending_rewards: 292_442_060 * ONE,
                    accumulated_paid_rewards: 142_851_325 * ONE,
                    ..get_predefined_global_farm_ins1(2)
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, global_farm_id, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 1_258,
                    accumulated_rpvs: FixedU128::from(3_100),
                    accumulated_rpz: FixedU128::from(620),
                    total_shares: 616 * ONE,
                    total_valued_shares: 45_540 * ONE,
                    entries_count: 3,
                    left_to_distribute: yield_farm_0.left_to_distribute + yield_farm_claim_from_global
                        - expected_claimed_rewards,
                    ..yield_farm_0
                },
            );

            //Check if claimed rewards are transferred.
            assert_eq!(
                Tokens::free_balance(BSX, &ALICE),
                alice_bsx_balance_0 + expected_claimed_rewards
            );

            assert_eq!(
                Tokens::free_balance(BSX, &pot),
                pot_balance_0 + reserved_for_both_farms - expected_claimed_rewards
            );

            let distributed_from_global =
                global_farm_total_rewards_start - Tokens::total_balance(REWARD_CURRENCY, &global_farm_account);

            let tracked_distributed_rewards = global_farm_1.accumulated_paid_rewards + global_farm_1.pending_rewards;

            pretty_assertions::assert_eq!(distributed_from_global, tracked_distributed_rewards);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });

    //Charlie's farm incentivize KSM and reward currency is ACA.
    //This test check if correct currency is transferred if rewards and incentivized
    //assets are different, otherwise farm behavior is the same as in tests above.
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            set_block_number(1_800); //period 18

            let global_farm_id = CHARLIE_FARM;
            let expected_claimed_rewards = 23_306_074_766_355_140; //ACA
            let unclaimable_rewards = 20_443_925_233_644_860;
            let deposited_amount = 50 * ONE;
            let deposit_id = 1;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                CHARLIE_FARM,
                CHARLIE_ACA_KSM_YIELD_FARM_ID,
                ACA_KSM_AMM,
                deposited_amount,
                |_, _, _| { Ok(2_500 * ONE) }
            ));

            assert_eq!(
                LiquidityMining::deposit(deposit_id).unwrap(),
                DepositData {
                    shares: deposited_amount,
                    amm_pool_id: ACA_KSM_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: CHARLIE_ACA_KSM_YIELD_FARM_ID,
                        accumulated_rpvs: Zero::zero(),
                        accumulated_claimed_rewards: 0,
                        entered_at: 18,
                        updated_at: 18,
                        valued_shares: 2_500 * ONE,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            set_block_number(2_596); //period 25

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, deposit_id, CHARLIE_ACA_KSM_YIELD_FARM_ID, FAIL_ON_DOUBLECLAIM)
                    .unwrap(),
                (CHARLIE_FARM, ACA, expected_claimed_rewards, unclaimable_rewards)
            );

            //Alice had 0 ACA before claim.
            assert_eq!(Tokens::free_balance(ACA, &ALICE), expected_claimed_rewards);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_deposit_with_multiple_entries_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            //predefined_deposit[0] - GC_FARM, BSX_TKN1_AMM
            set_block_number(50_000);
            assert_ok!(LiquidityMining::redeposit_lp_shares(
                EVE_FARM,
                EVE_BSX_TKN1_YIELD_FARM_ID,
                PREDEFINED_DEPOSIT_IDS[0],
                |_, _, _| { Ok(4_000 * ONE) }
            ));

            set_block_number(800_000);
            assert_ok!(LiquidityMining::redeposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                PREDEFINED_DEPOSIT_IDS[0],
                |_, _, _| { Ok(5_000 * ONE) }
            ));

            let deposit = LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap();

            assert_eq!(
                deposit.yield_farm_entries,
                vec![
                    YieldFarmEntry {
                        global_farm_id: GC_FARM,
                        valued_shares: 2_500 * ONE,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 0,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 18,
                        updated_at: 18,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                    YieldFarmEntry {
                        global_farm_id: EVE_FARM,
                        valued_shares: 4_000 * ONE,
                        yield_farm_id: EVE_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 0,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 50,
                        updated_at: 50,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                    YieldFarmEntry {
                        global_farm_id: DAVE_FARM,
                        valued_shares: 5_000 * ONE,
                        yield_farm_id: DAVE_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 0,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 800,
                        updated_at: 800,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                ]
            );

            set_block_number(1_000_000);
            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    EVE_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (EVE_FARM, KSM, 7_238_095_238_095_238_088, 361_904_761_904_761_912)
            );

            assert_noop!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    EVE_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                ),
                Error::<Test, Instance1>::DoubleClaimInPeriod
            );

            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (GC_FARM, BSX, 62_078_099_583_415_988_875, 309_400_416_584_011_125)
            );

            let deposit = LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap();
            assert_eq!(
                deposit.yield_farm_entries,
                vec![
                    YieldFarmEntry {
                        global_farm_id: GC_FARM,
                        valued_shares: 2_500 * ONE,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 62_078_099_583_415_988_875,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 18,
                        updated_at: 10_000,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                    YieldFarmEntry {
                        global_farm_id: EVE_FARM,
                        valued_shares: 4_000 * ONE,
                        yield_farm_id: EVE_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 7_238_095_238_095_238_088,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 50,
                        updated_at: 1_000,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                    YieldFarmEntry {
                        global_farm_id: DAVE_FARM,
                        valued_shares: 5_000 * ONE,
                        yield_farm_id: DAVE_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 0,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 800,
                        updated_at: 800,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                ]
            );

            //Same period different block.
            set_block_number(1_000_050);
            assert_noop!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    EVE_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                ),
                Error::<Test, Instance1>::DoubleClaimInPeriod
            );

            assert_noop!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                ),
                Error::<Test, Instance1>::DoubleClaimInPeriod
            );

            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    DAVE_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (DAVE_FARM, ACA, 1_666_666_666_666_666_666, 333_333_333_333_333_334)
            );

            let deposit = LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap();
            assert_eq!(
                deposit.yield_farm_entries,
                vec![
                    YieldFarmEntry {
                        global_farm_id: GC_FARM,
                        valued_shares: 2_500 * ONE,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 62_078_099_583_415_988_875,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 18,
                        updated_at: 10_000,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                    YieldFarmEntry {
                        global_farm_id: EVE_FARM,
                        valued_shares: 4_000 * ONE,
                        yield_farm_id: EVE_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 7_238_095_238_095_238_088,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 50,
                        updated_at: 1_000,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                    YieldFarmEntry {
                        global_farm_id: DAVE_FARM,
                        valued_shares: 5_000 * ONE,
                        yield_farm_id: DAVE_BSX_TKN1_YIELD_FARM_ID,
                        accumulated_claimed_rewards: 1_666_666_666_666_666_666,
                        accumulated_rpvs: Zero::zero(),
                        entered_at: 800,
                        updated_at: 1_000,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    },
                ]
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_doubleclaim_in_the_same_period_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            let global_farm_id = GC_FARM;
            let alice_bsx_balance = Tokens::free_balance(BSX, &ALICE);
            let pot = LiquidityMining::pot_account_id().unwrap();

            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);

            //1-th claim should works.
            assert_ok!(LiquidityMining::claim_rewards(
                ALICE,
                PREDEFINED_DEPOSIT_IDS[0],
                GC_BSX_TKN1_YIELD_FARM_ID,
                FAIL_ON_DOUBLECLAIM
            ));

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap(),
                DepositData {
                    shares: 50 * ONE,
                    amm_pool_id: BSX_TKN1_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        valued_shares: 2_500 * ONE,
                        accumulated_rpvs: Zero::zero(),
                        accumulated_claimed_rewards: 23_306_074_766_355_140,
                        entered_at: 18,
                        updated_at: 25,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID))
                    .unwrap()
                    .left_to_distribute,
                yield_farm_0.left_to_distribute - 23_306_074_766_355_140
            );

            assert_eq!(
                Tokens::free_balance(BSX, &ALICE),
                alice_bsx_balance + 23_306_074_766_355_140
            );
            assert_eq!(Tokens::free_balance(BSX, &pot), pot_balance_0 - 23_306_074_766_355_140);

            //Second claim should fail.
            assert_noop!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                ),
                Error::<Test, Instance1>::DoubleClaimInPeriod
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_should_claim_correct_amount_when_yield_farm_is_stopped() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            let global_farm_id = GC_FARM;
            let alibe_bsx_balance_0 = Tokens::free_balance(BSX, &ALICE);
            let pot = LiquidityMining::pot_account_id().unwrap();
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);
            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();

            let expected_global_pending_rewards = 41_675_375_000_000_000_000_u128;
            //Stop yield farming before claiming.
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            set_block_number(20_000);

            let expected_claimed_rewards = 23_306_074_766_355_140;
            let unclaimable_rewards = 20_443_925_233_644_860;

            //claim A1.1  (dep. A1 1-th time)
            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (global_farm_id, BSX, expected_claimed_rewards, unclaimable_rewards)
            );

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap(),
                DepositData {
                    shares: 50 * ONE,
                    amm_pool_id: BSX_TKN1_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
                        valued_shares: 2_500 * ONE,
                        accumulated_rpvs: Zero::zero(),
                        accumulated_claimed_rewards: expected_claimed_rewards,
                        entered_at: 18,
                        updated_at: 200,
                        stopped_at_creation: 0,
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID))
                    .unwrap()
                    .left_to_distribute,
                yield_farm_0.left_to_distribute - expected_claimed_rewards
            );

            //Check if claimed rewards are transferred.
            assert_eq!(
                Tokens::free_balance(BSX, &ALICE),
                alibe_bsx_balance_0 + expected_claimed_rewards
            );

            assert_eq!(
                Tokens::free_balance(BSX, &pot),
                pot_balance_0 + expected_global_pending_rewards - expected_claimed_rewards
            );

            //global-farm should be synced independetly of yield-farm (even it yield-farm is
            //stopped).
            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 200,
                    accumulated_rpz: FixedU128::from_inner(91_000_000_000_000_000_000_u128),
                    pending_rewards: 41_675_375_000_000_000_000_u128,
                    ..global_farm_0
                }
            );

            //Second claim on same deposit from stopped yield farm.
            //This should claim 0 rewards.
            set_block_number(300_000);
            //claim A1.1  (dep. A1 1-th time)
            assert_eq!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                )
                .unwrap(),
                (global_farm_id, BSX, 0, unclaimable_rewards)
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_should_fail_with_liqudity_mining_canceled_when_yield_farm_is_destroyed() {
    const FAIL_ON_DOUBLECLAIM: bool = true;
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            //Stop yield farming before removing.
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            //Delete yield farm before claim test.
            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            assert_noop!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM
                ),
                Error::<Test, Instance1>::LiquidityMiningCanceled
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn second_claim_rewards_should_work_when_doubleclaim_is_allowed() {
    const FAIL_ON_DOUBLECLAIM: bool = true;

    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            let (_, _, claimable_rewards, unclaimable_rewards) = LiquidityMining::claim_rewards(
                ALICE,
                PREDEFINED_DEPOSIT_IDS[0],
                GC_BSX_TKN1_YIELD_FARM_ID,
                !FAIL_ON_DOUBLECLAIM,
            )
            .unwrap();

            assert_eq!(claimable_rewards, 23_306_074_766_355_140);
            assert_eq!(unclaimable_rewards, 20_443_925_233_644_860);

            //Second claim in the same period should return 0 for `claimable_rewards` and real value for
            //`unclaimable_rewards`
            let (_, _, claimable_rewards, unclaimable_rewards) = LiquidityMining::claim_rewards(
                ALICE,
                PREDEFINED_DEPOSIT_IDS[0],
                GC_BSX_TKN1_YIELD_FARM_ID,
                !FAIL_ON_DOUBLECLAIM,
            )
            .unwrap();

            assert_eq!(claimable_rewards, 0);
            assert_eq!(unclaimable_rewards, 20_443_925_233_644_860);

            //check if double claim fails
            assert_noop!(
                LiquidityMining::claim_rewards(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    FAIL_ON_DOUBLECLAIM,
                ),
                Error::<Test, Instance1>::DoubleClaimInPeriod
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

//NOTE: farms are initialize like this intentionally. Bug may not appear with only 1 yield farm.
#[test]
fn deposits_should_claim_same_amount_when_created_in_the_same_period() {
    new_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            const GLOBAL_FARM: GlobalFarmId = 1;
            const YIELD_FARM_A: YieldFarmId = 2;
            const YIELD_FARM_B: YieldFarmId = 3;

            const BOB_DEPOSIT: DepositId = 2;
            const CHARLIE_DEPOSIT: DepositId = 3;

            const PLANNED_PERIODS: u64 = 10_000;
            const BLOCKS_PER_PERIOD: u64 = 10;
            const TOTAL_REWARDS_TO_DISTRIBUTE: u128 = 1_000_000 * ONE;

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
                1_000 * ONE,
                |_, _, _| { Ok(ONE) }
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
                1_000 * ONE,
                |_, _, _| { Ok(ONE) }
            ));

            //charlie
            assert_ok!(LiquidityMining2::deposit_lp_shares(
                GLOBAL_FARM,
                YIELD_FARM_B,
                BSX_TKN2_AMM,
                1_000 * ONE,
                |_, _, _| { Ok(ONE) }
            ));

            let bob_bsx_balance_0 = Tokens::free_balance(BSX, &BOB);
            let charlie_bsx_balance_0 = Tokens::free_balance(BSX, &CHARLIE);

            set_block_number(2_500);
            let _ = LiquidityMining2::claim_rewards(BOB, BOB_DEPOSIT, YIELD_FARM_B, false).unwrap();

            let _ = LiquidityMining2::claim_rewards(CHARLIE, CHARLIE_DEPOSIT, YIELD_FARM_B, false).unwrap();

            let bob_rewards = Tokens::free_balance(BSX, &BOB) - bob_bsx_balance_0;
            let charlie_rewards = Tokens::free_balance(BSX, &CHARLIE) - charlie_bsx_balance_0;

            pretty_assertions::assert_eq!(bob_rewards, charlie_rewards);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_should_claim_correct_amount_when_yield_farm_was_resumed() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            const REWARD_CURRENCY: AssetId = ACA;

            //Arrange
            //periods timeline:
            // |--- 10 active ---|--- 20 stopped ---|--- 10 active ---|claim_rewards()
            //
            set_block_number(20_000);

            let first_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                1_000_000 * ONE,
                |_, _, _| Ok(10_000_000 * ONE),
            )
            .unwrap();

            let second_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                2_000_000 * ONE,
                |_, _, _| Ok(20_000_000 * ONE),
            )
            .unwrap();

            set_block_number(30_000);

            // stop yield-farm
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));
            // resume yield-farm after 20 stopped periods.
            set_block_number(50_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            //Act & assert
            // claim rewards after 10 periods so deposit should claim in total for 20(active)
            // periods.
            set_block_number(60_000);

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, first_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    233_333_333_333_333_333_200,
                    166_666_666_666_666_666_800,
                )
            );

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, second_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    466_666_666_666_666_666_400,
                    333_333_333_333_333_333_600,
                )
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_should_claim_correct_amount_when_deposit_is_created_after_yield_farm_was_resumed() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            const REWARD_CURRENCY: AssetId = ACA;

            //Arrange
            set_block_number(20_000);

            let first_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                1_000_000 * ONE,
                |_, _, _| Ok(10_000_000 * ONE),
            )
            .unwrap();

            set_block_number(30_000);

            // stop yield-farm
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm after 20 stopped periods.
            set_block_number(50_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            let second_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                2_000_000 * ONE,
                |_, _, _| Ok(20_000_000 * ONE),
            )
            .unwrap();

            //Act & assert
            //dp 1 = total periods: 40, mining periods: 20
            //dp 2 = total periods: 10, mining periods: 10, (this dp was created after yield-farm
            //was resumed
            set_block_number(60_000);

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, first_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    233_333_333_333_333_333_200,
                    166_666_666_666_666_666_800,
                )
            );

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, second_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    218_181_818_181_818_181_600,
                    181_818_181_818_181_818_400,
                )
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_should_claim_correct_amount_when_yield_was_resumed_multiple_times() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            const REWARD_CURRENCY: AssetId = ACA;

            //Arrange
            set_block_number(20_000);

            let first_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                1_000_000 * ONE,
                |_, _, _| Ok(10_000_000 * ONE),
            )
            .unwrap();

            // stop yield-farm
            set_block_number(30_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm
            set_block_number(50_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            // create second deposit
            set_block_number(60_000);
            let second_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                2_000_000 * ONE,
                |_, _, _| Ok(20_000_000 * ONE),
            )
            .unwrap();

            // stop yield-farm
            set_block_number(80_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm
            set_block_number(90_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            // stop yield-farm
            set_block_number(100_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm
            set_block_number(120_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            //Act & assert
            //claim rewards
            set_block_number(140_000);
            //dp 1 = total periods: 120, mining periods: 70
            //dp 2 = total periods: 80, mining periods: 50

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, first_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    988_235_294_117_647_058_000,
                    411_764_705_882_352_942_000,
                )
            );

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, second_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    1_333_333_333_333_333_332_000,
                    666_666_666_666_666_668_000,
                )
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn claim_rewards_should_claim_correct_amount_when_yield_was_resumed_multiple_times_and_is_stopped_now() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const FAIL_ON_DOUBLECLAIM: bool = true;
            const REWARD_CURRENCY: AssetId = ACA;

            //Arrange
            set_block_number(20_000);

            let first_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                1_000_000 * ONE,
                |_, _, _| Ok(10_000_000 * ONE),
            )
            .unwrap();

            // stop yield-farm
            set_block_number(30_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm
            set_block_number(50_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            // create second deposit
            set_block_number(60_000);
            let second_deposit_id = LiquidityMining::deposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                2_000_000 * ONE,
                |_, _, _| Ok(20_000_000 * ONE),
            )
            .unwrap();

            // stop yield-farm
            set_block_number(80_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm
            set_block_number(90_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            // stop yield-farm
            set_block_number(100_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            // resume yield-farm
            set_block_number(120_000);
            assert_ok!(LiquidityMining::resume_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                FixedU128::from(10)
            ));

            // stop yield-farm
            set_block_number(140_000);
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));

            //Act & assert
            //claim rewards

            set_block_number(200_000);

            //dp 1 = total periods: 120, mining periods: 70
            //dp 2 = total periods: 80, mining periods: 50

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, first_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    988_235_294_117_647_058_000,
                    411_764_705_882_352_942_000,
                )
            );

            assert_eq!(
                LiquidityMining::claim_rewards(ALICE, second_deposit_id, DAVE_BSX_TKN1_YIELD_FARM_ID, true,).unwrap(),
                (
                    DAVE_FARM,
                    REWARD_CURRENCY,
                    1_333_333_333_333_333_332_000,
                    666_666_666_666_666_668_000,
                )
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}
