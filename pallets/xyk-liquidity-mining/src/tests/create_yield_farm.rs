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
use pretty_assertions::assert_eq;

const BSX_ACA_ASSET_PAIR: AssetPair = AssetPair {
	asset_in: BSX,
	asset_out: ACA,
};

#[test]
fn create_yield_farm_should_work_when_global_farm_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, BSX, 1_000_000 * ONE)])
		.with_amm_pool(BSX_ACA_AMM, BSX_ACA_SHARE_ID, BSX_ACA_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			10_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(20),
			ONE,
			One::one(),
		)
		.build()
		.execute_with(|| {
			let multiplier = One::one();
			let loyalty_curve = Some(LoyaltyCurve {
				initial_reward_percentage: FixedU128::from_float(0.558),
				scale_coef: 20,
			});

			set_block_number(17_850);

			//Act
			assert_ok!(LiquidityMining::create_yield_farm(
				Origin::signed(ALICE),
				ALICE_FARM,
				BSX_ACA_ASSET_PAIR,
				multiplier,
				loyalty_curve.clone()
			));

			//Assert
			assert_last_event!(crate::Event::YieldFarmCreated {
				global_farm_id: ALICE_FARM,
				yield_farm_id: 2,
				multiplier,
				loyalty_curve,
				asset_pair: BSX_ACA_ASSET_PAIR,
			}
			.into());
		})
}

#[test]
fn create_yield_farm_should_fail_when_amm_pool_doesnt_exists() {
	let assets_without_pool = AssetPair {
		asset_in: BSX,
		asset_out: KSM,
	};

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, BSX, 1_000_000 * ONE)])
		.with_amm_pool(BSX_ACA_AMM, BSX_ACA_SHARE_ID, BSX_ACA_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			10_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(20),
			ONE,
			One::one(),
		)
		.build()
		.execute_with(|| {
			assert_noop!(
				LiquidityMining::create_yield_farm(
					Origin::signed(ALICE),
					ALICE_FARM,
					assets_without_pool,
					One::one(),
					Some(LoyaltyCurve::default())
				),
				Error::<Test>::XykPoolDoesntExist
			);
		});
}
