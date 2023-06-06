// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

#[test]
fn create_global_farm_should_work_when_origin_is_allowed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			let total_rewards = 80_000_000 * ONE;
			let planned_yielding_periods = 2_628_000; //.5 year, 6s blocks, 1 block pre period
			let blocks_per_period = 1;
			let reward_currency = HDX;
			let owner = GC;
			let yield_per_period = Perquintill::from_float(0.000_000_15_f64); //APR ~= 80%
			let min_deposit = 1_000;
			let lrna_price_adjustment = FixedU128::from_float(0.65_f64);

			assert_ok!(OmnipoolMining::create_global_farm(
				RuntimeOrigin::root(),
				total_rewards,
				planned_yielding_periods,
				blocks_per_period,
				reward_currency,
				owner,
				yield_per_period,
				min_deposit,
				lrna_price_adjustment,
			));

			assert_last_event!(crate::Event::GlobalFarmCreated {
				id: 1,
				owner: GC,
				total_rewards,
				reward_currency,
				yield_per_period,
				planned_yielding_periods,
				blocks_per_period,
				max_reward_per_period: 30_441_400_304_414_u128,
				min_deposit,
				lrna_price_adjustment,
			}
			.into());
		});
}

#[test]
fn create_global_farm_should_fail_when_origin_is_not_allowed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			let total_rewards = 80_000_000 * ONE;
			let planned_yielding_periods = 2_628_000; //.5 year, 6s blocks, 1 block pre period
			let blocks_per_period = 1;
			let reward_currency = HDX;
			let owner = GC;
			let yield_per_period = Perquintill::from_float(0.000_000_15_f64); //APR ~= 80%
			let min_deposit = 1_000;

			assert_noop!(
				OmnipoolMining::create_global_farm(
					RuntimeOrigin::signed(ALICE),
					total_rewards,
					planned_yielding_periods,
					blocks_per_period,
					reward_currency,
					owner,
					yield_per_period,
					min_deposit,
					FixedU128::one(),
				),
				BadOrigin
			);
		});
}

#[test]
fn create_global_farm_should_fail_when_origin_is_none() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		//.with_token(DOT, FixedU128::from_float(0.5), LP2, 2000 * ONE)
		//NOTE: add_token created 2 positons(nfts)
		//.with_liquidity(ALICE, KSM, 5_000 * ONE)
		//.with_liquidity(BOB, DOT, 5_000 * ONE)
		.build()
		.execute_with(|| {
			let total_rewards = 80_000_000 * ONE;
			let planned_yielding_periods = 2_628_000; //.5 year, 6s blocks, 1 block pre period
			let blocks_per_period = 1;
			let reward_currency = HDX;
			let owner = GC;
			let yield_per_period = Perquintill::from_float(0.000_000_15_f64); //APR ~= 80%
			let min_deposit = 1_000;

			assert_noop!(
				OmnipoolMining::create_global_farm(
					RuntimeOrigin::none(),
					total_rewards,
					planned_yielding_periods,
					blocks_per_period,
					reward_currency,
					owner,
					yield_per_period,
					min_deposit,
					FixedU128::one(),
				),
				BadOrigin
			);
		});
}
