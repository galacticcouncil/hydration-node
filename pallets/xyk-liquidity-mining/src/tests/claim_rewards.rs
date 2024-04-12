// This file is part of Basilisk-node.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

#[test]
fn claim_rewards_should_work_when_deposit_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(CHARLIE, 1, 2, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(10_000);

			//Act
			assert_ok!(LiquidityMining::claim_rewards(Origin::signed(CHARLIE), 1, 2));

			//Assert
			assert_last_event!(crate::Event::RewardClaimed {
				global_farm_id: 1,
				yield_farm_id: 2,
				who: CHARLIE,
				claimed: 20_000_000 * ONE,
				reward_currency: BSX,
				deposit_id: 1,
			}
			.into());
		});
}

#[test]
fn claim_rewards_should_propagate_error_when_claims_rewards_fails_due_to_double_claim() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(CHARLIE, 1, 2, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			set_block_number(1_000);
			//Arrange
			assert_ok!(LiquidityMining::claim_rewards(Origin::signed(CHARLIE), 1, 2));

			//Act and assert
			assert_noop!(
				LiquidityMining::claim_rewards(Origin::signed(CHARLIE), 1, 2),
				"Dummy Double Claim"
			);
		});
}

#[test]
fn claim_rewards_should_fail_when_origin_is_not_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(CHARLIE, 1, 2, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(LiquidityMining::claim_rewards(Origin::none(), 1, 2), BadOrigin);
		});
}

#[test]
fn claim_rewards_should_fail_when_claimed_by_non_deposit_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(CHARLIE, 1, 2, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			const NOT_OWNER: u128 = BOB;

			assert_noop!(
				LiquidityMining::claim_rewards(Origin::signed(NOT_OWNER), 1, 2),
				Error::<Test>::NotDepositOwner
			);
		});
}

#[test]
fn claim_rewards_should_fail_when_claimed_reward_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(ZERO_REWARDS_USER, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(ZERO_REWARDS_USER, 1, 2, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			set_block_number(1_000);

			assert_noop!(
				LiquidityMining::claim_rewards(Origin::signed(ZERO_REWARDS_USER), 1, 2),
				Error::<Test>::ZeroClaimedRewards
			);
		});
}
