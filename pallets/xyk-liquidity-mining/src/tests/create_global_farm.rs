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
fn create_global_farm_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, BSX, 500_000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let id = 1;
			let total_rewards: Balance = 400_000 * ONE;
			let reward_currency = BSX;
			let planned_yielding_periods: BlockNumber = 1_000_000_000_u64;
			let blocks_per_period = 20_000;
			let incentivized_asset = BSX;
			let owner = ALICE;
			let yield_per_period = Perquintill::from_percent(20);
			let max_reward_per_period: Balance = total_rewards.checked_div(planned_yielding_periods.into()).unwrap();
			let min_deposit = 3;
			let price_adjustment = One::one();

			let created_at_block = 15_896;

			set_block_number(created_at_block);

			//Act
			assert_ok!(LiquidityMining::create_global_farm(
				Origin::root(),
				total_rewards,
				planned_yielding_periods,
				blocks_per_period,
				incentivized_asset,
				reward_currency,
				owner,
				yield_per_period,
				min_deposit,
				price_adjustment
			));

			assert_last_event!(crate::Event::GlobalFarmCreated {
				id,
				owner,
				total_rewards,
				reward_currency,
				yield_per_period,
				planned_yielding_periods,
				blocks_per_period,
				incentivized_asset,
				max_reward_per_period,
				min_deposit,
				price_adjustment,
			}
			.into());
		});
}

#[test]
fn create_global_farm_should_fail_when_not_allowed_origin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, BSX, 500_000 * ONE)])
		.build()
		.execute_with(|| {
			let created_at_block = 15_896;

			set_block_number(created_at_block);

			assert_noop!(
				LiquidityMining::create_global_farm(
					Origin::signed(ALICE),
					1_000_000,
					1_000,
					300,
					BSX,
					BSX,
					ALICE,
					Perquintill::from_percent(20),
					3,
					One::one()
				),
				BadOrigin
			);
		});
}
