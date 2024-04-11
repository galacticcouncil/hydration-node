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
fn deposit_shares_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 100 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			pretty_assertions::assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				0
			);

			set_block_number(1_800);
			let deposited_amount = 50 * ONE;

			//Act
			assert_ok!(LiquidityMining::deposit_shares(
				Origin::signed(ALICE),
				1,
				2,
				BSX_KSM_ASSET_PAIR,
				deposited_amount,
			));

			//Assert
			assert_last_event!(crate::Event::SharesDeposited {
				global_farm_id: 1,
				yield_farm_id: 2,
				who: ALICE,
				lp_token: BSX_KSM_SHARE_ID,
				amount: deposited_amount,
				deposit_id: 1
			}
			.into());

			pretty_assertions::assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				deposited_amount
			);

			let nft_owner: AccountId = DummyNFT::owner(&LM_NFT_COLLECTION, &1).unwrap();
			pretty_assertions::assert_eq!(nft_owner, ALICE);
		});
}

#[test]
fn deposit_shares_should_fail_when_account_balance_is_insufficient() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 40 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			assert_noop!(
				LiquidityMining::deposit_shares(Origin::signed(ALICE), 1, 2, BSX_KSM_ASSET_PAIR, 50 * ONE),
				Error::<Test>::InsufficientXykSharesBalance
			);
		});
}

#[test]
fn deposit_shares_should_fail_when_origin_is_not_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 50 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			assert_noop!(
				LiquidityMining::deposit_shares(Origin::none(), 1, 2, BSX_KSM_ASSET_PAIR, 50 * ONE),
				BadOrigin
			);
		});
}

#[test]
fn deposit_shares_should_fail_when_amm_pool_does_not_exist() {
	let assets_without_amm: AssetPair = AssetPair {
		asset_in: BSX,
		asset_out: DOT,
	};

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 50 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			assert_noop!(
				LiquidityMining::deposit_shares(Origin::signed(ALICE), 1, 2, assets_without_amm, 50 * ONE),
				Error::<Test>::XykPoolDoesntExist
			);
		});
}
