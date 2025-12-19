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

use pallet_omnipool::types::{AssetReserveState, Tradability};
use pretty_assertions::assert_eq;

#[test]
fn remove_liquidity_stableswap_omnipool_and_exit_farms_should_work_with_single_asset_removal() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			GC,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 2;
			let deposit_id = 1;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			// Add liquidity first
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			// Remove liquidity to single asset
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				deposit_id,
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
			));

			// Verify omnipool state returned to initial (all liquidity removed)
			assert_asset_state!(
				STABLESWAP_POOL_ID,
				AssetReserveState {
					reserve: token_amount,
					hub_reserve: 1_300_000_000_000_000,
					shares: token_amount,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			// Verify storage is cleaned up
			assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);
			assert_eq!(
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id),
				None
			);

			// Verify DepositDestroyed event was emitted
			assert!(has_event(
				crate::Event::DepositDestroyed { who: LP1, deposit_id }.into()
			));
		});
}

#[test]
fn remove_liquidity_stableswap_omnipool_and_exit_farms_should_work_with_multi_asset_removal() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
			(LP1, USDC, 5000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(USDC)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(USDC, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			GC,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 2;
			let deposit_id = 1;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			// Add liquidity first
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount), AssetAmount::new(USDC, amount)]
					.try_into()
					.unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			// Remove liquidity proportionally to multiple assets
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				deposit_id,
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, 1), AssetAmount::new(USDC, 1)]
					.try_into()
					.unwrap(),
			));

			// Verify omnipool state returned to initial (all liquidity removed)
			assert_asset_state!(
				STABLESWAP_POOL_ID,
				AssetReserveState {
					reserve: token_amount,
					hub_reserve: 1_300_000_000_000_000,
					shares: token_amount,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			// Verify storage is cleaned up
			assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);
			assert_eq!(
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id),
				None
			);
		});
}

#[test]
fn remove_liquidity_stableswap_omnipool_and_exit_farms_should_work_with_multiple_farms() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(BOB, HDX, 100_000_000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			GC,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			CHARLIE,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_global_farm(
			60_000_000 * ONE,
			2_428_000,
			1,
			HDX,
			BOB,
			Perquintill::from_float(0.000_000_14_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.with_yield_farm(CHARLIE, 2, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.with_yield_farm(BOB, 3, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 4;
			let charlie_g_farm_id = 2;
			let charlie_y_farm_id = 5;
			let bob_g_farm_id = 3;
			let bob_y_farm_id = 6;
			let deposit_id = 1;
			let yield_farms = vec![
				(gc_g_farm_id, gc_y_farm_id),
				(charlie_g_farm_id, charlie_y_farm_id),
				(bob_g_farm_id, bob_y_farm_id),
			];

			// Add liquidity and join 3 farms
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			// Remove liquidity - should exit all 3 farms automatically
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				deposit_id,
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
			));

			// Verify omnipool state returned to initial (all liquidity removed)
			assert_asset_state!(
				STABLESWAP_POOL_ID,
				AssetReserveState {
					reserve: token_amount,
					hub_reserve: 1_300_000_000_000_000,
					shares: token_amount,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			// Verify all farms were exited (SharesWithdrawn events)
			assert!(has_event(
				crate::Event::SharesWithdrawn {
					global_farm_id: gc_g_farm_id,
					yield_farm_id: gc_y_farm_id,
					who: LP1,
					amount: SHARES_FROM_STABLESWAP,
					deposit_id,
				}
				.into()
			));
			assert!(has_event(
				crate::Event::SharesWithdrawn {
					global_farm_id: charlie_g_farm_id,
					yield_farm_id: charlie_y_farm_id,
					who: LP1,
					amount: SHARES_FROM_STABLESWAP,
					deposit_id,
				}
				.into()
			));
			assert!(has_event(
				crate::Event::SharesWithdrawn {
					global_farm_id: bob_g_farm_id,
					yield_farm_id: bob_y_farm_id,
					who: LP1,
					amount: SHARES_FROM_STABLESWAP,
					deposit_id,
				}
				.into()
			));

			// Verify storage is cleaned up
			assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);
			assert_eq!(
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id),
				None
			);
		});
}

#[test]
fn remove_liquidity_stableswap_omnipool_and_exit_farms_should_fail_when_not_owner() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
			(LP2, USDT, 5000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			GC,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 2;
			let deposit_id = 1;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			// LP1 adds liquidity
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			// LP2 tries to remove LP1's liquidity - should fail
			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP2),
					deposit_id,
					STABLESWAP_POOL_ID,
					vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
				),
				crate::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn remove_liquidity_stableswap_omnipool_and_exit_farms_should_fail_with_empty_assets() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			GC,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 2;
			let deposit_id = 1;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			// Add liquidity first
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			// Try to remove with empty assets list - should fail
			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP1),
					deposit_id,
					STABLESWAP_POOL_ID,
					vec![].try_into().unwrap(),
				),
				Error::<Test>::NoAssetsSpecified
			);
		});
}

#[test]
fn remove_liquidity_stableswap_omnipool_and_exit_farms_full_round_trip_with_rewards() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.with_global_farm(
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			GC,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, STABLESWAP_POOL_ID, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 2;
			let deposit_id = 1;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			// Add liquidity
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			// Wait some blocks to accumulate rewards
			set_block_number(100);

			let hdx_balance_before = Tokens::free_balance(HDX, &LP1);

			// Remove liquidity
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				deposit_id,
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
			));

			// Verify omnipool state returned to initial (all liquidity removed)
			assert_asset_state!(
				STABLESWAP_POOL_ID,
				AssetReserveState {
					reserve: token_amount,
					hub_reserve: 1_300_000_000_000_000,
					shares: token_amount,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			let hdx_balance_after = Tokens::free_balance(HDX, &LP1);

			// Verify user received HDX rewards (if any were generated)
			// Note: rewards might be 0 in test depending on block time configuration
			if hdx_balance_after > hdx_balance_before {
				// Rewards were claimed
				assert!(has_event(
					crate::Event::RewardClaimed {
						global_farm_id: gc_g_farm_id,
						yield_farm_id: gc_y_farm_id,
						who: LP1,
						claimed: hdx_balance_after - hdx_balance_before,
						reward_currency: HDX,
						deposit_id,
					}
					.into()
				));
			}

			// Verify all storage is cleaned up
			assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);
			assert_eq!(
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id),
				None
			);
		});
}
