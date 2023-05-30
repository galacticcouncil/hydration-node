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
use hydradx_traits::liquidity_mining::Mutate;
use pretty_assertions::assert_eq;
use test_ext::*;

#[test]
fn withdraw_shares_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const REWARD_CURRENCY: u32 = BSX;
            const GLOBAL_FARM_ID: GlobalFarmId = GC_FARM;

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let global_farm_total_rewards_start = 30_000_000_000 * ONE;

            let pot = LiquidityMining::pot_account_id().unwrap();
            let pot_initial_balance = 1_000_000_000 * ONE;

            // This balance is used to transfer unclaimable_rewards from yield farm to global farm.
            // Claiming is not part of withdraw_shares() so some balance need to be set.
            Tokens::set_balance(Origin::root(), pot, REWARD_CURRENCY, pot_initial_balance, 0).unwrap();

            // withdraw 1A
            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);

            let unclaimable_rewards = 100_000 * ONE;
            let withdrawn_amount = 50 * ONE;
            let expected_deposit_destroyed = true;

            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards,
                )
                .unwrap(),
                (GLOBAL_FARM_ID, withdrawn_amount, expected_deposit_destroyed,)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    reward_currency: BSX,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares_z: 691_490 * ONE,
                    pending_rewards: 0,
                    accumulated_paid_rewards: 1_283_550 * ONE - unclaimable_rewards,
                    ..get_predefined_global_farm_ins1(2)
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(17_500_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 566 * ONE,
                    total_valued_shares: 43_040 * ONE,
                    entries_count: 2,
                    left_to_distribute: bsx_tkn1_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn1_yield_farm_0
                },
            );

            //Unclaimabe rewards was transferred from pot to global-farm's account.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            //Global farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]), None);

            set_block_number(12_800);

            // withdraw 3B

            let unclaimable_rewards = 32_786 * ONE;
            let withdrawn_amount = 87 * ONE;
            let expected_deposit_destroyed = true;

            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let bsx_tkn2_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[4],
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (GLOBAL_FARM_ID, withdrawn_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    total_shares_z: 688_880 * ONE,
                    ..global_farm_0
                }
            );

            // This farm should not change.
            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                bsx_tkn1_yield_farm_0
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(35_000_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 873 * ONE,
                    total_valued_shares: 47_368 * ONE,
                    entries_count: 3,
                    left_to_distribute: bsx_tkn2_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn2_yield_farm_0
                },
            );

            //Pot's balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[4]).is_none());

            // withdraw 3A

            let unclaimable_rewards = 2_441 * ONE;
            let withdrawn_amount = 486 * ONE;
            let expected_deposit_destroyed = true;

            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[6],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards,
                )
                .unwrap(),
                (GLOBAL_FARM_ID, withdrawn_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    total_shares_z: 494480 * ONE,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(17_500_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 80 * ONE,
                    total_valued_shares: 4_160 * ONE,
                    entries_count: 1,
                    left_to_distribute: bsx_tkn1_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn1_yield_farm_0
                },
            );

            //Yield farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            //Global farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[6]).is_none());

            // withdraw 2A

            let unclaimable_rewards = 429 * ONE;
            let withdrawn_amount = 80 * ONE;
            let expected_deposit_destroyed = true;

            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[1],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards,
                )
                .unwrap(),
                (GLOBAL_FARM_ID, withdrawn_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    total_shares_z: 473_680 * ONE,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(17_500_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 0,
                    total_valued_shares: 0,
                    entries_count: 0,
                    left_to_distribute: bsx_tkn1_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn1_yield_farm_0
                },
            );

            //Yield farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[1]).is_none());

            // withdraw 1B
            let unclaimable_rewards = 30_001 * ONE;

            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let bsx_tkn2_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_ok!(LiquidityMining::withdraw_lp_shares(
                PREDEFINED_DEPOSIT_IDS[2],
                GC_BSX_TKN2_YIELD_FARM_ID,
                unclaimable_rewards
            ));

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    total_shares_z: 471_680 * ONE,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                bsx_tkn1_yield_farm_0
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(35_000_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 848 * ONE,
                    total_valued_shares: 47_168 * ONE,
                    entries_count: 2,
                    left_to_distribute: bsx_tkn2_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn2_yield_farm_0
                },
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            //Global farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[2]), None);

            // withdraw 4B
            let unclaimable_rewards = 96_473 * ONE;
            let withdrawn_shares = 48 * ONE;
            let expected_deposit_destroyed = true;

            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn2_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[5],
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    unclaimable_rewards,
                )
                .unwrap(),
                (GLOBAL_FARM_ID, withdrawn_shares, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    total_shares_z: 464_000 * ONE,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(35_000_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 800 * ONE,
                    total_valued_shares: 46_400 * ONE,
                    entries_count: 1,
                    left_to_distribute: bsx_tkn2_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn2_yield_farm_0
                },
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            //Global farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[5]).is_none());

            // withdraw 2B
            let unclaimable_rewards = 5_911 * ONE;
            let withdrawn_shares = 800 * ONE;
            let expected_deposit_destroyed = true;

            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn2_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();

            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[3],
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (GLOBAL_FARM_ID, withdrawn_shares, expected_deposit_destroyed)
            );

            let global_farm_1 = LiquidityMining::global_farm(GC_FARM).unwrap();
            pretty_assertions::assert_eq!(
                global_farm_1,
                GlobalFarmData {
                    updated_at: 25,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    total_shares_z: 0,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(35_000_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 0,
                    total_valued_shares: 0,
                    entries_count: 0,
                    left_to_distribute: bsx_tkn2_yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..bsx_tkn2_yield_farm_0
                },
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards
            );

            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[2]).is_none());

            let distributed_from_global =
                global_farm_total_rewards_start - Tokens::total_balance(REWARD_CURRENCY, &global_farm_account);

            let tracked_distributed_rewards = global_farm_1.accumulated_paid_rewards + global_farm_1.pending_rewards;

            pretty_assertions::assert_eq!(distributed_from_global, tracked_distributed_rewards);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });

    //Charlie's farm incentivize KSM and reward currency is ACA
    //This test check if correct currency is transferred if rewards and incentvized
    //assets are different, otherwise farm behavior is the same as in test above.
    predefined_test_ext().execute_with(|| {
        let _ = with_transaction(|| {
            set_block_number(1_800); //period 18

            let deposited_amount = 50 * ONE;
            assert_ok!(LiquidityMining::deposit_lp_shares(
                CHARLIE_FARM,
                CHARLIE_ACA_KSM_YIELD_FARM_ID,
                ACA_KSM_AMM,
                deposited_amount,
                |_, _, _| { Ok(2_500 * ONE) }
            ));

            const DEPOSIT_ID: DepositId = 1;
            let global_farm_id = CHARLIE_FARM;
            assert_eq!(
                LiquidityMining::deposit(DEPOSIT_ID).unwrap(),
                DepositData {
                    shares: 50 * ONE,
                    amm_pool_id: ACA_KSM_AMM,
                    yield_farm_entries: vec![YieldFarmEntry {
                        global_farm_id,
                        yield_farm_id: CHARLIE_ACA_KSM_YIELD_FARM_ID,
                        accumulated_rpvs: Zero::zero(),
                        accumulated_claimed_rewards: 0,
                        entered_at: 18,
                        updated_at: 18,
                        valued_shares: 2_500 * ONE,
                        stopped_at_creation: Zero::zero(),
                        _phantom: PhantomData::default(),
                    }]
                    .try_into()
                    .unwrap(),
                },
            );

            set_block_number(2_596); //period 25

            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(DEPOSIT_ID, CHARLIE_ACA_KSM_YIELD_FARM_ID, 0).unwrap(),
                (CHARLIE_FARM, deposited_amount, expected_deposit_destroyed)
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn withdraw_with_multiple_entries_and_flush_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            let alice_bsx_tkn1_lp_shares_balance = Tokens::free_balance(BSX_TKN1_SHARE_ID, &ALICE);

            //Redeposit to multiple yield farms.
            assert_ok!(LiquidityMining::redeposit_lp_shares(
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                PREDEFINED_DEPOSIT_IDS[0],
                |_, _, _| { Ok(10 * ONE) },
            ));

            assert_ok!(LiquidityMining::redeposit_lp_shares(
                EVE_FARM,
                EVE_BSX_TKN1_YIELD_FARM_ID,
                PREDEFINED_DEPOSIT_IDS[0],
                |_, _, _| { Ok(10 * ONE) },
            ));
            //NOTE: predefined_deposit_ids[0] is deposited in 3 yield farms now.

            //Stop yield farm.
            assert_ok!(LiquidityMining::stop_yield_farm(EVE, EVE_FARM, BSX_TKN1_AMM));
            //Stop and destroy all yield farms so it can be flushed.
            assert_ok!(LiquidityMining::stop_yield_farm(DAVE, DAVE_FARM, BSX_TKN1_AMM));
            assert_ok!(LiquidityMining::terminate_yield_farm(
                DAVE,
                DAVE_FARM,
                DAVE_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            assert_ok!(LiquidityMining::terminate_global_farm(DAVE, DAVE_FARM));

            let unclaimable_rewards = 0;
            let shares_amount = 50 * ONE;
            let expected_deposit_destroyed = false;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (GC_FARM, shares_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0])
                    .unwrap()
                    .yield_farm_entries
                    .len(),
                2
            );

            //LP tokens should not be unlocked.
            assert_eq!(
                Tokens::free_balance(BSX_TKN1_SHARE_ID, &ALICE),
                alice_bsx_tkn1_lp_shares_balance
            );

            //This withdraw should flush yield and global farms.
            let expected_deposit_destroyed = false;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[0],
                    DAVE_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (DAVE_FARM, shares_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0])
                    .unwrap()
                    .yield_farm_entries
                    .len(),
                1
            );

            //LP tokens should not be unlocked.
            assert_eq!(
                Tokens::free_balance(BSX_TKN1_SHARE_ID, &ALICE),
                alice_bsx_tkn1_lp_shares_balance
            );

            assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, DAVE_FARM, DAVE_BSX_TKN1_YIELD_FARM_ID)).is_none());
            assert!(LiquidityMining::global_farm(DAVE_FARM).is_none());

            //Non-dustable check
            let global_farm_account = LiquidityMining::farm_account_id(DAVE_FARM).unwrap();
            assert_eq!(Whitelist::contains(&global_farm_account), false);

            let yield_farm_account = LiquidityMining::farm_account_id(DAVE_BSX_TKN1_YIELD_FARM_ID).unwrap();
            assert_eq!(Whitelist::contains(&yield_farm_account), false);

            //This withdraw should flush yield and global farms.
            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[0],
                    EVE_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (EVE_FARM, shares_amount, expected_deposit_destroyed)
            );

            //Last withdraw from deposit should flush deposit.
            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).is_none());

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn withdraw_shares_from_destroyed_farm_should_work() {
    //This is the case when yield farm is removed and global farm is destroyed.
    //In this case only amm shares should be withdrawn.

    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            //Stop all yield farms in the global farm.
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN2_AMM));

            //Remove all yield farms from global farm.
            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));
            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN2_YIELD_FARM_ID,
                BSX_TKN2_AMM
            ));

            //Destroy farm.
            assert_ok!(LiquidityMining::terminate_global_farm(GC, GC_FARM));

            assert!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID))
                    .unwrap()
                    .state
                    .is_terminated()
            );
            assert!(
                LiquidityMining::yield_farm((BSX_TKN2_AMM, GC_FARM, GC_BSX_TKN2_YIELD_FARM_ID))
                    .unwrap()
                    .state
                    .is_terminated()
            );
            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap().state,
                FarmState::Terminated
            );

            let test_data = vec![
                (
                    ALICE,
                    0,
                    50 * ONE,
                    2_u64,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_SHARE_ID,
                    BSX_TKN1_AMM,
                ),
                (
                    BOB,
                    1,
                    80 * ONE,
                    1_u64,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_SHARE_ID,
                    BSX_TKN1_AMM,
                ),
                (
                    BOB,
                    2,
                    25 * ONE,
                    3_u64,
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    BSX_TKN2_SHARE_ID,
                    BSX_TKN2_AMM,
                ),
                (
                    BOB,
                    3,
                    800 * ONE,
                    2_u64,
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    BSX_TKN2_SHARE_ID,
                    BSX_TKN2_AMM,
                ),
                (
                    ALICE,
                    4,
                    87 * ONE,
                    1_u64,
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    BSX_TKN2_SHARE_ID,
                    BSX_TKN2_AMM,
                ),
                (
                    ALICE,
                    5,
                    48 * ONE,
                    0_u64,
                    GC_BSX_TKN2_YIELD_FARM_ID,
                    BSX_TKN2_SHARE_ID,
                    BSX_TKN2_AMM,
                ),
                (
                    ALICE,
                    6,
                    486 * ONE,
                    0_u64,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_SHARE_ID,
                    BSX_TKN1_AMM,
                ),
            ];

            for (_caller, deposit_idx, withdrawn_shares, _farm_entries_left, yield_farm_id, _lp_token, _amm_pool_id) in
                test_data
            {
                let expected_deposit_destroyed = true;
                assert_eq!(
                    LiquidityMining::withdraw_lp_shares(PREDEFINED_DEPOSIT_IDS[deposit_idx], yield_farm_id, 0,)
                        .unwrap(),
                    (GC_FARM, withdrawn_shares, expected_deposit_destroyed)
                );

                //check if deposit was removed.
                assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[deposit_idx]).is_none());
            }

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn withdraw_shares_from_canceled_yield_farm_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            set_block_number(10_000);

            // Stop yield farm before withdraw test.
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let pot = LiquidityMining::pot_account_id().unwrap();

            //1-th withdraw
            //_0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(BSX, &global_farm_account);
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();
            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();

            let unclaimable_rewards = 168_270 * ONE;
            let withdrawn_amount = 50 * ONE;
            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (GC_FARM, withdrawn_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    total_shares: yield_farm_0.total_shares - withdrawn_amount,
                    total_valued_shares: yield_farm_0.total_valued_shares - 2500 * ONE,
                    entries_count: 2,
                    left_to_distribute: yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..yield_farm_0
                }
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).is_none());

            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(Tokens::free_balance(BSX, &pot), pot_balance_0 - unclaimable_rewards);

            //2-nd withdraw
            //_0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(BSX, &global_farm_account);
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();
            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();

            let unclaimable_rewards = 2_055_086 * ONE;
            let shares_amount = 486 * ONE;
            let valued_shares_amount = 38_880 * ONE;

            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[6],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (GC_FARM, shares_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    total_shares: yield_farm_0.total_shares - shares_amount,
                    total_valued_shares: yield_farm_0.total_valued_shares - valued_shares_amount,
                    entries_count: 1,
                    left_to_distribute: yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..yield_farm_0
                }
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[6]).is_none());

            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(Tokens::free_balance(BSX, &pot), pot_balance_0 - unclaimable_rewards);

            //3-th withdraw
            //_0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(BSX, &global_farm_account);
            let pot_balance_0 = Tokens::free_balance(BSX, &pot);
            let global_farm_0 = LiquidityMining::global_farm(GC_FARM).unwrap();
            let yield_farm_0 = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();

            let unclaimable_rewards = 228_572 * ONE;
            let shares_amount = 80 * ONE;

            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(
                    PREDEFINED_DEPOSIT_IDS[1],
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    unclaimable_rewards
                )
                .unwrap(),
                (GC_FARM, shares_amount, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    accumulated_paid_rewards: global_farm_0.accumulated_paid_rewards - unclaimable_rewards,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    total_shares: 0,
                    total_valued_shares: 0,
                    entries_count: 0,
                    left_to_distribute: yield_farm_0.left_to_distribute - unclaimable_rewards,
                    ..yield_farm_0
                }
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[1]).is_none());

            assert_eq!(
                Tokens::free_balance(BSX, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(Tokens::free_balance(BSX, &pot), pot_balance_0 - unclaimable_rewards);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn withdraw_shares_from_removed_pool_should_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            set_block_number(10_000);

            //Stop yield farm before removing.
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GC_FARM, BSX_TKN1_AMM));

            //Terminate yield farm before test
            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GC_FARM,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            assert!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID))
                    .unwrap()
                    .state
                    .is_terminated(),
            );

            let global_farm = LiquidityMining::global_farm(GC_FARM).unwrap();

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();
            let global_farm_bsx_balance = Tokens::free_balance(BSX, &global_farm_account);
            let alice_bsx_balance = Tokens::free_balance(BSX, &ALICE);

            let yield_farm = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let shares_amount = 50 * ONE;
            //1-th withdraw
            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(PREDEFINED_DEPOSIT_IDS[0], GC_BSX_TKN1_YIELD_FARM_ID, 0).unwrap(),
                (GC_FARM, shares_amount, expected_deposit_destroyed)
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).is_none());

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    total_shares: 566 * ONE,
                    total_valued_shares: 43_040 * ONE,
                    entries_count: 2,
                    ..yield_farm
                }
            );

            assert_eq!(LiquidityMining::global_farm(GC_FARM).unwrap(), global_farm);

            //Removed yield farm don't pay rewards, only transfers amm shares.
            assert_eq!(Tokens::free_balance(BSX, &ALICE), alice_bsx_balance);
            assert_eq!(Tokens::free_balance(BSX, &global_farm_account), global_farm_bsx_balance);

            //2-nd withdraw
            let alice_bsx_balance = Tokens::free_balance(BSX, &ALICE);
            let shares_amount = 486 * ONE;

            let yield_farm = LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(PREDEFINED_DEPOSIT_IDS[6], GC_BSX_TKN1_YIELD_FARM_ID, 0,).unwrap(),
                (GC_FARM, shares_amount, expected_deposit_destroyed)
            );

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[6]).is_none());

            assert_eq!(LiquidityMining::global_farm(GC_FARM).unwrap(), global_farm);

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    total_shares: 80 * ONE,
                    total_valued_shares: 4_160 * ONE,
                    entries_count: 1,
                    ..yield_farm
                }
            );

            //removed yield farm don't pay rewards, only return LP shares
            assert_eq!(Tokens::free_balance(BSX, &ALICE), alice_bsx_balance);
            assert_eq!(Tokens::free_balance(BSX, &global_farm_account), global_farm_bsx_balance);

            //3-th withdraw
            let bob_bsx_balance = Tokens::free_balance(BSX, &BOB);
            let shares_amount = 80 * ONE;

            let expected_deposit_destroyed = true;
            assert_eq!(
                LiquidityMining::withdraw_lp_shares(PREDEFINED_DEPOSIT_IDS[1], GC_BSX_TKN1_YIELD_FARM_ID, 0).unwrap(),
                (GC_FARM, shares_amount, expected_deposit_destroyed)
            );

            //Last withdraw should flush yield farm if it's deleted
            assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).is_none());

            assert!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[1]).is_none());

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    //counts changed because last deposit removed deleted yield farm from storage
                    live_yield_farms_count: 1,
                    total_yield_farms_count: 1,
                    ..global_farm
                }
            );

            //Removed yield farm don't pay rewards, only return LP shares.
            assert_eq!(Tokens::free_balance(BSX, &BOB), bob_bsx_balance);
            assert_eq!(Tokens::free_balance(BSX, &global_farm_account), global_farm_bsx_balance);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn withdraw_shares_yield_farm_entry_not_found_should_not_work() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const DEPOSIT_ID: DepositId = 1;
            const NOT_FOUND_ENTRY_ID: YieldFarmId = 999_999;
            assert_noop!(
                LiquidityMining::withdraw_lp_shares(DEPOSIT_ID, NOT_FOUND_ENTRY_ID, 0),
                Error::<Test, Instance1>::YieldFarmEntryNotFound
            );
            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "Defensive failure has been triggered!"))]
fn withdraw_shares_should_fail_when_deposit_not_found() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            assert_noop!(
                LiquidityMining::withdraw_lp_shares(72_334_321_125_861_359_621, GC_BSX_TKN1_YIELD_FARM_ID, 0),
                Error::<Test, Instance1>::InconsistentState(InconsistentStateError::DepositNotFound)
            );

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn trait_withdraw_lp_shares_should_claim_and_withdraw_when_yield_farm_is_claimable() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const REWARD_CURRENCY: u32 = BSX;
            const GLOBAL_FARM_ID: GlobalFarmId = GC_FARM;

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();

            let pot = LiquidityMining::pot_account_id().unwrap();
            let pot_initial_balance = 1_000_000_000 * ONE;

            // This balance is used to transfer unclaimable_rewards from yield farm to global farm.
            // Claiming is not part of withdraw_shares() so some balance need to be set.
            Tokens::set_balance(Origin::root(), pot, REWARD_CURRENCY, pot_initial_balance, 0).unwrap();

            // withdraw 1A
            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);

            let withdrawn_amount = 50 * ONE;
            let expected_deposit_destroyed = true;
            let expected_claimed_amount = 23_306_074_766_355_140_u128;
            let unclaimable_rewards = 20_443_925_233_644_860_u128;
            let expected_claim_data = Some((REWARD_CURRENCY, expected_claimed_amount, unclaimable_rewards));

            assert_eq!(
                <LiquidityMining as Mutate<AccountId, AssetId, BlockNumberFor<Test>>>::withdraw_lp_shares(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_FARM,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_AMM,
                )
                .unwrap(),
                (withdrawn_amount, expected_claim_data, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    reward_currency: BSX,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares_z: 691_490 * ONE,
                    pending_rewards: 0,
                    accumulated_paid_rewards: 1_283_550 * ONE - unclaimable_rewards,
                    ..get_predefined_global_farm_ins1(2)
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(17_500_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 566 * ONE,
                    total_valued_shares: 43_040 * ONE,
                    entries_count: 2,
                    left_to_distribute: bsx_tkn1_yield_farm_0.left_to_distribute
                        - unclaimable_rewards
                        - expected_claimed_amount,
                    ..bsx_tkn1_yield_farm_0
                },
            );

            //Unclaimabe rewards was transferred from pot to global-farm's account and claimed
            //amount was transferred to user.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards - expected_claimed_amount
            );

            //Global farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]), None);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn trait_withdraw_lp_shares_should_only_withdraw_when_yield_farm_is_not_claimable() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const REWARD_CURRENCY: u32 = BSX;
            const GLOBAL_FARM_ID: GlobalFarmId = GC_FARM;

            //destroy yield-farm
            assert_ok!(LiquidityMining::stop_yield_farm(GC, GLOBAL_FARM_ID, BSX_TKN1_AMM));
            assert_ok!(LiquidityMining::terminate_yield_farm(
                GC,
                GLOBAL_FARM_ID,
                GC_BSX_TKN1_YIELD_FARM_ID,
                BSX_TKN1_AMM
            ));

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();

            let pot = LiquidityMining::pot_account_id().unwrap();
            let pot_initial_balance = 1_000_000_000 * ONE;

            // This balance is used to transfer unclaimable_rewards from yield farm to global farm.
            // Claiming is not part of withdraw_shares() so some balance need to be set.
            Tokens::set_balance(Origin::root(), pot, REWARD_CURRENCY, pot_initial_balance, 0).unwrap();

            // withdraw 1A
            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let global_farm_0 = LiquidityMining::global_farm(GLOBAL_FARM_ID).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);

            let withdrawn_amount = 50 * ONE;
            let expected_deposit_destroyed = true;
            let expected_claim_data = None; //claim from terminated farm should not happen

            assert_eq!(
                <LiquidityMining as Mutate<AccountId, AssetId, BlockNumberFor<Test>>>::withdraw_lp_shares(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_FARM,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_AMM,
                )
                .unwrap(),
                (withdrawn_amount, expected_claim_data, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    reward_currency: BSX,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    pending_rewards: 0,
                    ..global_farm_0
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    total_shares: 566 * ONE,
                    total_valued_shares: 43_040 * ONE,
                    entries_count: 2,
                    ..bsx_tkn1_yield_farm_0
                }
            );

            //Nothing was claimed from pot becasue yield-farm is terminated so balance should not
            //change
            assert_eq!(Tokens::free_balance(REWARD_CURRENCY, &pot), pot_balance_0);

            //Global farm balance should not change becasue no rewards were transferred.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0
            );

            assert_eq!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]), None);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}

#[test]
fn trait_withdraw_lp_shares_should_claim_zero_when_user_already_claimed_rewards() {
    predefined_test_ext_with_deposits().execute_with(|| {
        let _ = with_transaction(|| {
            const REWARD_CURRENCY: u32 = BSX;
            const GLOBAL_FARM_ID: GlobalFarmId = GC_FARM;

            let global_farm_account = LiquidityMining::farm_account_id(GC_FARM).unwrap();

            let pot = LiquidityMining::pot_account_id().unwrap();
            let pot_initial_balance = 1_000_000_000 * ONE;

            let fail_on_doubleclaim = true;
            assert_ok!(LiquidityMining::claim_rewards(
                ALICE,
                PREDEFINED_DEPOSIT_IDS[0],
                GC_BSX_TKN1_YIELD_FARM_ID,
                fail_on_doubleclaim
            ));

            // This balance is used to transfer unclaimable_rewards from yield farm to global farm.
            // Claiming is not part of withdraw_shares() so some balance need to be set.
            Tokens::set_balance(Origin::root(), pot, REWARD_CURRENCY, pot_initial_balance, 0).unwrap();

            // withdraw 1A
            // _0 - value before act.
            let global_farm_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
            let bsx_tkn1_yield_farm_0 =
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap();
            let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);

            let withdrawn_amount = 50 * ONE;
            let expected_deposit_destroyed = true;
            let expected_claimed_amount = 0_u128;
            let unclaimable_rewards = 20_443_925_233_644_860_u128;
            let expected_claim_data = Some((REWARD_CURRENCY, expected_claimed_amount, unclaimable_rewards));

            assert_eq!(
                <LiquidityMining as Mutate<AccountId, AssetId, BlockNumberFor<Test>>>::withdraw_lp_shares(
                    ALICE,
                    PREDEFINED_DEPOSIT_IDS[0],
                    GC_FARM,
                    GC_BSX_TKN1_YIELD_FARM_ID,
                    BSX_TKN1_AMM,
                )
                .unwrap(),
                (withdrawn_amount, expected_claim_data, expected_deposit_destroyed)
            );

            assert_eq!(
                LiquidityMining::global_farm(GC_FARM).unwrap(),
                GlobalFarmData {
                    updated_at: 25,
                    reward_currency: BSX,
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares_z: 691_490 * ONE,
                    pending_rewards: 0,
                    accumulated_paid_rewards: 1_283_550 * ONE - unclaimable_rewards,
                    ..get_predefined_global_farm_ins1(2)
                }
            );

            assert_eq!(
                LiquidityMining::yield_farm((BSX_TKN1_AMM, GC_FARM, GC_BSX_TKN1_YIELD_FARM_ID)).unwrap(),
                YieldFarmData {
                    updated_at: 25,
                    accumulated_rpvs: FixedU128::from_inner(17_500_000_000_000_000_000_u128),
                    accumulated_rpz: FixedU128::from_inner(3_500_000_000_000_000_000_u128),
                    total_shares: 566 * ONE,
                    total_valued_shares: 43_040 * ONE,
                    entries_count: 2,
                    left_to_distribute: bsx_tkn1_yield_farm_0.left_to_distribute
                        - unclaimable_rewards
                        - expected_claimed_amount,
                    ..bsx_tkn1_yield_farm_0
                },
            );

            //Unclaimabe rewards was transferred from pot to global-farm's account and claimed
            //amount was transferred to user.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &pot),
                pot_balance_0 - unclaimable_rewards - expected_claimed_amount
            );

            //Global farm balance checks.
            assert_eq!(
                Tokens::free_balance(REWARD_CURRENCY, &global_farm_account),
                global_farm_balance_0 + unclaimable_rewards
            );

            assert_eq!(LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]), None);

            TransactionOutcome::Commit(DispatchResult::Ok(()))
        });
    });
}
