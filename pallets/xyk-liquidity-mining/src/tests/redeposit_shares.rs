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
fn redeposit_shares_should_work_when_deposit_already_exists() {
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
			set_block_number(50_000);

			//Act
			assert_ok!(LiquidityMining::redeposit_shares(
				Origin::signed(CHARLIE),
				2,
				4,
				BSX_KSM_ASSET_PAIR,
				1,
			));

			assert_last_event!(crate::Event::SharesRedeposited {
				global_farm_id: 2,
				yield_farm_id: 4,
				who: CHARLIE,
				lp_token: BSX_KSM_SHARE_ID,
				amount: 100 * ONE,
				deposit_id: 1,
			}
			.into());
		})
}

#[test]
fn redeposit_shares_should_fail_when_called_by_not_the_deposit_owner() {
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
			assert_noop!(
				LiquidityMining::redeposit_shares(Origin::signed(BOB), 2, 4, BSX_KSM_ASSET_PAIR, 1,),
				Error::<Test>::NotDepositOwner
			);
		});
}

#[test]
fn redeposit_shares_deposit_should_fail_when_asset_pair_has_invalid_asset() {
	let pair_without_amm = AssetPair {
		asset_in: BSX,
		asset_out: DOT,
	};

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
			assert_noop!(
				LiquidityMining::redeposit_shares(Origin::signed(CHARLIE), 2, 4, pair_without_amm, 1,),
				Error::<Test>::XykPoolDoesntExist
			);
		});
}

#[test]
fn redeposit_shares_should_fail_when_origin_is_not_signed() {
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
			assert_noop!(
				LiquidityMining::redeposit_shares(Origin::none(), 2, 4, BSX_KSM_ASSET_PAIR, 1,),
				BadOrigin
			);
		});
}

#[test]
fn redeposit_shares_should_fail_when_nft_owner_is_not_found() {
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
			set_block_number(50_000);
			let deposit_id = 1;

			//Arrange
			//Destory NFT without destruction of depoist.
			mock::DummyNFT::burn(&LM_NFT_COLLECTION, &deposit_id, None::<&AccountId>).unwrap();

			//Act
			assert_noop!(
				LiquidityMining::redeposit_shares(Origin::signed(CHARLIE), 2, 4, BSX_KSM_ASSET_PAIR, 1,),
				Error::<Test>::CantFindDepositOwner
			);
		})
}

#[test]
fn redeposit_shares_should_fail_when_asset_pair_is_not_in_the_deposit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(BOB, ACA, 1_000_000 * ONE),
			(CHARLIE, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_amm_pool(BSX_ACA_AMM, BSX_ACA_SHARE_ID, BSX_ACA_ASSET_PAIR)
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
			set_block_number(50_000);

			//Act
			assert_noop!(
				LiquidityMining::redeposit_shares(Origin::signed(CHARLIE), 2, 4, BSX_ACA_ASSET_PAIR, 1),
				Error::<Test>::InvalidAssetPair
			);
		})
}
