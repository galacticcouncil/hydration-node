// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use pretty_assertions::assert_eq;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| set_block_number(1));
    ext
}

pub fn predefined_test_ext() -> sp_io::TestExternalities {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let _ = with_transaction(|| {
            let expected_farm = get_predefined_global_farm_ins1(0);
            assert_ok!(LiquidityMining::create_global_farm(
                100_000_000_000 * ONE,
                expected_farm.planned_yielding_periods,
                expected_farm.blocks_per_period,
                expected_farm.incentivized_asset,
                expected_farm.reward_currency,
                ALICE,
                expected_farm.yield_per_period,
                expected_farm.min_deposit,
                expected_farm.price_adjustment,
            ));

            let expected_farm = get_predefined_global_farm_ins1(1);
            assert_ok!(LiquidityMining::create_global_farm(
                1_000_000_000 * ONE,
                expected_farm.planned_yielding_periods,
                expected_farm.blocks_per_period,
                expected_farm.incentivized_asset,
                expected_farm.reward_currency,
                BOB,
                expected_farm.yield_per_period,
                expected_farm.min_deposit,
                expected_farm.price_adjustment,
            ));

            let expected_farm = get_predefined_global_farm_ins1(2);
            assert_ok!(LiquidityMining::create_global_farm(
                30_000_000_000 * ONE,
                expected_farm.planned_yielding_periods,
                expected_farm.blocks_per_period,
                expected_farm.incentivized_asset,
                expected_farm.reward_currency,
                GC,
                expected_farm.yield_per_period,
                expected_farm.min_deposit,
                expected_farm.price_adjustment,
            ));

            let expected_farm = get_predefined_global_farm_ins1(3);
            assert_ok!(LiquidityMining::create_global_farm(
                30_000_000_000 * ONE,
                expected_farm.planned_yielding_periods,
                expected_farm.blocks_per_period,
                expected_farm.incentivized_asset,
                expected_farm.reward_currency,
                CHARLIE,
                expected_farm.yield_per_period,
                expected_farm.min_deposit,
                expected_farm.price_adjustment,
            ));

            let expected_farm = get_predefined_global_farm_ins1(4);
            assert_ok!(LiquidityMining::create_global_farm(
                30_000_000_000 * ONE,
                expected_farm.planned_yielding_periods,
                expected_farm.blocks_per_period,
                expected_farm.incentivized_asset,
                expected_farm.reward_currency,
                DAVE,
                expected_farm.yield_per_period,
                expected_farm.min_deposit,
                expected_farm.price_adjustment,
            ));

            let expected_farm = get_predefined_global_farm_ins1(5);
            assert_ok!(LiquidityMining::create_global_farm(
                30_000_000_000 * ONE,
                expected_farm.planned_yielding_periods,
                expected_farm.blocks_per_period,
                expected_farm.incentivized_asset,
                expected_farm.reward_currency,
                EVE,
                expected_farm.yield_per_period,
                expected_farm.min_deposit,
                expected_farm.price_adjustment,
            ));

            let amm_mock_data = vec![
                (
                    AssetPair {
                        asset_in: BSX,
                        asset_out: ACA,
                    },
                    (BSX_ACA_AMM, BSX_ACA_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: KSM,
                        asset_out: BSX,
                    },
                    (BSX_KSM_AMM, BSX_KSM_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: BSX,
                        asset_out: DOT,
                    },
                    (BSX_DOT_AMM, BSX_DOT_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: BSX,
                        asset_out: ETH,
                    },
                    (BSX_ETH_AMM, BSX_ETH_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: BSX,
                        asset_out: HDX,
                    },
                    (BSX_HDX_AMM, BSX_HDX_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: BSX,
                        asset_out: TKN1,
                    },
                    (BSX_TKN1_AMM, BSX_TKN1_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: BSX,
                        asset_out: TKN2,
                    },
                    (BSX_TKN2_AMM, BSX_TKN2_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: KSM,
                        asset_out: DOT,
                    },
                    (KSM_DOT_AMM, KSM_DOT_SHARE_ID),
                ),
                (
                    AssetPair {
                        asset_in: ACA,
                        asset_out: KSM,
                    },
                    (ACA_KSM_AMM, ACA_KSM_SHARE_ID),
                ),
            ];

            AMM_POOLS.with(|h| {
                let mut hm = h.borrow_mut();
                for (k, v) in amm_mock_data {
                    hm.insert(asset_pair_to_map_key(k), v);
                }
            });

            let yield_farm = get_predefined_yield_farm_ins1(0);
            init_yield_farm_ins1(GC, GC_FARM, BSX_TKN1_AMM, BSX, TKN1, yield_farm);

            let yield_farm = get_predefined_yield_farm_ins1(1);
            init_yield_farm_ins1(GC, GC_FARM, BSX_TKN2_AMM, BSX, TKN2, yield_farm);

            let yield_farm = get_predefined_yield_farm_ins1(2);
            init_yield_farm_ins1(CHARLIE, CHARLIE_FARM, ACA_KSM_AMM, ACA, KSM, yield_farm);

            let yield_farm = get_predefined_yield_farm_ins1(3);
            init_yield_farm_ins1(DAVE, DAVE_FARM, BSX_TKN1_AMM, BSX, TKN1, yield_farm);

            let yield_farm = get_predefined_yield_farm_ins1(4);
            init_yield_farm_ins1(EVE, EVE_FARM, BSX_TKN1_AMM, BSX, TKN1, yield_farm);

            let yield_farm = get_predefined_yield_farm_ins1(5);
            init_yield_farm_ins1(EVE, EVE_FARM, BSX_TKN2_AMM, BSX, TKN2, yield_farm);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });

    ext
}

fn init_yield_farm_ins1(
    owner: AccountId,
    farm_id: GlobalFarmId,
    amm_id: AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    yield_farm: YieldFarmData<Test, Instance1>,
) {
    assert_ok!(LiquidityMining::create_yield_farm(
        owner,
        farm_id,
        yield_farm.multiplier,
        yield_farm.loyalty_curve.clone(),
        amm_id,
        vec![asset_a, asset_b],
    ));

    assert_eq!(
        LiquidityMining::yield_farm((amm_id, farm_id, yield_farm.id)).unwrap(),
        yield_farm
    );
}

pub fn predefined_test_ext_with_deposits() -> sp_io::TestExternalities {
    let mut ext = predefined_test_ext();

    ext.execute_with(|| {
        let _ = with_transaction(|| {
            let farm_id = GC_FARM;

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let pot_account = LiquidityMining::pot_account_id().unwrap();

            //DEPOSIT 1:
            set_block_number(1_800); //18-th period

            let deposited_amount = 50 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                farm_id,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                deposited_amount,
                |_, _, _| { Ok(2_500 * ONE) },
            ));

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).is_some());

            // DEPOSIT 2 (deposit in same period):
            let deposited_amount = 80 * ONE;
            assert_eq!(
                LiquidityMining::deposit_lp_shares(
                    farm_id,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_AMM,
                    deposited_amount,
                    |_, _, _| { Ok(4_160 * ONE) },
                )
                .unwrap(),
                PREDEFINED_DEPOSIT_IDS[1]
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[1]).is_some());

            // DEPOSIT 3 (same period, second yield farm):
            let deposited_amount = 25 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                farm_id,
                GC_BSX_TKN2_YIELD_FARM_ID,
                BSX_TKN2_AMM,
                deposited_amount,
                |_, _, _| { Ok(200 * ONE) },
            ));

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[2]).is_some());

            // DEPOSIT 4 (new period):
            set_block_number(2051); //period 20

            let deposited_amount = 800 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                farm_id,
                GC_BSX_TKN2_YIELD_FARM_ID,
                BSX_TKN2_AMM,
                deposited_amount,
                |_, _, _| { Ok(46_400 * ONE) },
            ));

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[3]).is_some());

            // DEPOSIT 5 (same period, second yield farm):
            set_block_number(2_586); //period 25

            let deposited_amount = 87 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                farm_id,
                GC_BSX_TKN2_YIELD_FARM_ID,
                BSX_TKN2_AMM,
                deposited_amount,
                |_, _, _| { Ok(261 * ONE) },
            ));

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[4]).is_some());

            // DEPOSIT 6 (same period):
            set_block_number(2_596); //period 25

            let deposited_amount = 48 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                farm_id,
                GC_BSX_TKN2_YIELD_FARM_ID,
                BSX_TKN2_AMM,
                deposited_amount,
                |_, _, _| { Ok(768 * ONE) },
            ));

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[5]).is_some());

            // DEPOSIT 7 : (same period different liq poll farm)
            set_block_number(2_596); //period 25

            let deposited_amount = 486 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                farm_id,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM,
                deposited_amount,
                |_, _, _| { Ok(38_880 * ONE) },
            ));

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[6]).is_some());

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    id: GC_FARM,
                    updated_at: 25,
                    reward_currency: BSX,
                    yield_per_period: Perquintill::from_percent(50),
                    planned_yielding_periods: 500_u64,
                    blocks_per_period: 100_u64,
                    owner: GC,
                    incentivized_asset: BSX,
                    max_reward_per_period: 60_000_000 * ONE,
                    accumulated_rpz: FixedU128::from_float(3.5_f64),
                    live_yield_farms_count: 2,
                    total_yield_farms_count: 2,
                    total_shares_z: 703_990 * ONE,
                    pending_rewards: 0,
                    accumulated_paid_rewards: 1_283_550 * ONE,
                    state: FarmState::Active,
                    min_deposit: 1_000,
                    price_adjustment: One::one(),
                }
            );

            let bsx_tkn1_yield_farm_left_to_distribute = 116_550 * ONE;
            let bsx_tkn2_yield_farm_left_to_distribute = 1_167_000 * ONE;

            let yield_farm_id = PREDEFINED_YIELD_FARMS_INS1.with(|v| v[0].id);
            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, yield_farm_id)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_float(17.5_f64),
                    accumulated_rpz: FixedU128::from_float(3.5_f64),
                    total_shares: 616 * ONE,
                    total_valued_shares: 45_540 * ONE,
                    entries_count: 3,
                    left_to_distribute: bsx_tkn1_yield_farm_left_to_distribute,
                    ..PREDEFINED_YIELD_FARMS_INS1.with(|v| v[0].clone())
                },
            );

            let yield_farm_id = PREDEFINED_YIELD_FARMS_INS1.with(|v| v[1].id);
            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, yield_farm_id)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from(35),
                    accumulated_rpz: FixedU128::from_float(3.5_f64),
                    total_shares: 960 * ONE,
                    total_valued_shares: 47_629 * ONE,
                    entries_count: 4,
                    left_to_distribute: bsx_tkn2_yield_farm_left_to_distribute,
                    ..PREDEFINED_YIELD_FARMS_INS1.with(|v| v[1].clone())
                },
            );

            //Reward currency balance check.
            //total_rewards - (global_farm_paid_accumulated_rewards + global_farm_accumualted_rewards)
            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                (30_000_000_000 * ONE - (1_033_900 * ONE + 249_650 * ONE))
            );

            //Pot account balance check
            assert_eq!(
                Tokens::free_balance(BSX, &pot_account),
                bsx_tkn1_yield_farm_left_to_distribute + bsx_tkn2_yield_farm_left_to_distribute
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });

    ext
}
