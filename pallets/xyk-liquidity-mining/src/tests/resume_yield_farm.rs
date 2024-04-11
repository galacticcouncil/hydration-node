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
fn resume_yield_farm_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE)])
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
			//Arrange
			assert_ok!(LiquidityMining::stop_yield_farm(
				Origin::signed(BOB),
				1,
				BSX_KSM_ASSET_PAIR
			));

			set_block_number(13_420_000);

			//Act
			let new_multiplier = FixedU128::from(7_490_000);
			assert_ok!(LiquidityMining::resume_yield_farm(
				Origin::signed(BOB),
				1,
				2,
				BSX_KSM_ASSET_PAIR,
				new_multiplier
			));

			//Assert
			assert_last_event!(crate::Event::YieldFarmResumed {
				global_farm_id: 1,
				yield_farm_id: 2,
				who: BOB,
				asset_pair: BSX_KSM_ASSET_PAIR,
				multiplier: new_multiplier,
			}
			.into());
		});
}

#[test]
fn resume_yield_farm_should_fail_when_amm_pool_does_not_exist() {
	let pair_without_amm: AssetPair = AssetPair {
		asset_in: BSX,
		asset_out: DOT,
	};

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE)])
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
			//Arrange
			assert_ok!(LiquidityMining::stop_yield_farm(
				Origin::signed(BOB),
				1,
				BSX_KSM_ASSET_PAIR
			));
			set_block_number(13_420_000);

			//Act and assert
			assert_noop!(
				LiquidityMining::resume_yield_farm(
					Origin::signed(BOB),
					1,
					2,
					pair_without_amm,
					FixedU128::from(7_490_000)
				),
				Error::<Test>::XykPoolDoesntExist
			);
		});
}

#[test]
fn resume_yield_farm_should_fail_when_caller_is_not_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE)])
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
				LiquidityMining::resume_yield_farm(
					Origin::none(),
					1,
					2,
					BSX_KSM_ASSET_PAIR,
					FixedU128::from(7_490_000)
				),
				BadOrigin
			);
		});
}
