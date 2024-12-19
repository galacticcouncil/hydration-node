// This file is part of Basilisk-node

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
fn withdraw_shares_should_work() {
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
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			BOB,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 2, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(CHARLIE, 1, 3, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(1_000);

			//redeposit lp shares
			assert_ok!(LiquidityMining::redeposit_shares(
				Origin::signed(CHARLIE),
				2,
				4,
				BSX_KSM_ASSET_PAIR,
				1
			));

			let charlie_lp_token_balance = Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				100 * ONE
			);

			//Act
			assert_ok!(LiquidityMining::withdraw_shares(
				Origin::signed(CHARLIE),
				1,
				3,
				BSX_KSM_ASSET_PAIR
			));

			//Assert
			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::RewardClaimed {
						global_farm_id: 1,
						yield_farm_id: 3,
						who: CHARLIE,
						claimed: 20_000_000 * ONE,
						reward_currency: BSX,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::SharesWithdrawn {
						global_farm_id: 1,
						yield_farm_id: 3,
						who: CHARLIE,
						lp_token: BSX_KSM_SHARE_ID,
						amount: 100 * ONE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			//NOTE: balance should not change because deposit is not destroyed
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE),
				charlie_lp_token_balance
			);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				100 * ONE
			);

			//Second claim with desposit destruction
			set_block_number(2_000);
			//Act
			assert_ok!(LiquidityMining::withdraw_shares(
				Origin::signed(CHARLIE),
				1,
				4,
				BSX_KSM_ASSET_PAIR
			));

			//Assert
			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::RewardClaimed {
						global_farm_id: 2,
						yield_farm_id: 4,
						who: CHARLIE,
						claimed: 20_000_000 * ONE,
						reward_currency: BSX,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::SharesWithdrawn {
						global_farm_id: 2,
						yield_farm_id: 4,
						who: CHARLIE,
						lp_token: BSX_KSM_SHARE_ID,
						amount: 100 * ONE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::DepositDestroyed {
						who: CHARLIE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			//NOTE: deposit was destroyed and LP shares unlocked
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE),
				charlie_lp_token_balance + 100 * ONE
			);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				0
			);
		});
}

#[test]
fn withdraw_should_work_when_it_is_in_same_period_as_claim() {
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
			set_block_number(1_000);
			assert_ok!(LiquidityMining::claim_rewards(Origin::signed(CHARLIE), 1, 2));

			pretty_assertions::assert_eq!(Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE), 100 * ONE);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				100 * ONE
			);

			//Act
			assert_ok!(LiquidityMining::withdraw_shares(
				Origin::signed(CHARLIE),
				1,
				2,
				BSX_KSM_ASSET_PAIR
			));

			//Assert
			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::SharesWithdrawn {
						global_farm_id: 1,
						yield_farm_id: 2,
						who: CHARLIE,
						lp_token: BSX_KSM_SHARE_ID,
						amount: 100 * ONE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::DepositDestroyed {
						who: CHARLIE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			//lp shares unlocked
			pretty_assertions::assert_eq!(Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE), 200 * ONE);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				0
			);
		});
}

#[test]
fn withdraw_shares_should_not_work_when_not_owner() {
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
			const NOT_FNT_OWNER: u128 = BOB;

			assert_noop!(
				LiquidityMining::withdraw_shares(Origin::signed(NOT_FNT_OWNER), 1, 2, BSX_KSM_ASSET_PAIR),
				Error::<Test>::NotDepositOwner
			);
		});
}

#[test]
fn withdraw_shares_should_fail_when_global_farm_is_not_found() {
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
			let non_known_farm: u32 = 99999;

			assert_noop!(
				LiquidityMining::withdraw_shares(Origin::signed(CHARLIE), 1, non_known_farm, BSX_KSM_ASSET_PAIR),
				Error::<Test>::DepositDataNotFound
			);
		});
}

#[test]
fn withdraw_shares_should_fail_when_origin_is_not_signed() {
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
			assert_noop!(
				LiquidityMining::withdraw_shares(Origin::none(), 1, 2, BSX_KSM_ASSET_PAIR),
				BadOrigin
			);
		});
}

#[test]
fn withdraw_shares_should_fail_when_nft_owner_is_not_found() {
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

			const NOT_EXISTS_DEPOSIT: u128 = 2;

			assert_noop!(
				LiquidityMining::withdraw_shares(Origin::signed(CHARLIE), NOT_EXISTS_DEPOSIT, 2, BSX_KSM_ASSET_PAIR),
				Error::<Test>::CantFindDepositOwner
			);
		});
}

#[test]
fn withdraw_shares_should_work_when_yield_farm_is_not_claimable() {
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
			set_block_number(1_000);

			let charlie_lp_token_balance = Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				100 * ONE
			);

			assert_ok!(LiquidityMining::stop_yield_farm(
				Origin::signed(ALICE),
				1,
				BSX_KSM_ASSET_PAIR
			));

			//Act
			assert_ok!(LiquidityMining::withdraw_shares(
				Origin::signed(CHARLIE),
				1,
				2,
				BSX_KSM_ASSET_PAIR
			));

			//Assert
			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::SharesWithdrawn {
						global_farm_id: 1,
						yield_farm_id: 2,
						who: CHARLIE,
						lp_token: BSX_KSM_SHARE_ID,
						amount: 100 * ONE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::DepositDestroyed {
						who: CHARLIE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			//NOTE: deposit was destroyed and LP shares unlocked
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &CHARLIE),
				charlie_lp_token_balance + 100 * ONE
			);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				0
			);
		});
}
