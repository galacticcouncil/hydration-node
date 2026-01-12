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

use frame_support::BoundedVec;
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

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			// Remove liquidity to single asset
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				position_id,
				Balance::MIN,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
				Some(deposit_id),
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

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			// Remove liquidity proportionally to multiple assets
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				position_id,
				Balance::MIN,
				vec![AssetAmount::new(USDT, 1), AssetAmount::new(USDC, 1)]
					.try_into()
					.unwrap(),
				Some(deposit_id),
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

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				position_id,
				Balance::MIN,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
				Some(deposit_id),
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

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP2),
					position_id,
					Balance::MIN,
					vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
					Some(deposit_id),
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

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP1),
					position_id,
					Balance::MIN,
					vec![].try_into().unwrap(),
					Some(deposit_id),
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

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			// Wait enough blocks to accumulate rewards
			set_block_number(1_000);

			let hdx_balance_before = Tokens::free_balance(HDX, &LP1);

			// Remove liquidity
			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				position_id,
				Balance::MIN,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
				Some(deposit_id),
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
			let expected_claimed_rewards = 243_506_250_u128;

			// Verify user received HDX rewards
			assert_eq!(hdx_balance_after - hdx_balance_before, expected_claimed_rewards);

			assert!(has_event(
				crate::Event::RewardClaimed {
					global_farm_id: gc_g_farm_id,
					yield_farm_id: gc_y_farm_id,
					who: LP1,
					claimed: expected_claimed_rewards,
					reward_currency: HDX,
					deposit_id,
				}
				.into()
			));

			// Verify all storage is cleaned up
			assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);
			assert_eq!(
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id),
				None
			);
		});
}

#[test]
fn remove_liquidity_without_farm_exit_should_work() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.build()
		.execute_with(|| {
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				None, // No farms
				None,
			));

			let position_id = 4;

			assert!(pallet_omnipool::Pallet::<Test>::load_position(position_id, LP1).is_ok());

			assert_ok!(OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
				RuntimeOrigin::signed(LP1),
				position_id,
				Balance::MIN,
				vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
				None, // No deposit_id since we never joined farms
			));

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

			// Verify NO farm events were emitted
			assert!(!has_event(
				crate::Event::DepositDestroyed {
					who: LP1,
					deposit_id: 1
				}
				.into()
			));
		});
}

#[test]
fn should_fail_with_mismatched_deposit_and_position() {
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
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			// Create first position with farms (deposit_id = 1)
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.clone().try_into().unwrap()),
				None,
			));

			let deposit_1 = 1;
			let position_1 = crate::OmniPositionId::<Test>::get(deposit_1).expect("Position 1 should exist");

			// Create second position with farms (deposit_id = 2)
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			let deposit_2 = 2;
			let position_2 = crate::OmniPositionId::<Test>::get(deposit_2).expect("Position 2 should exist");

			assert_ne!(position_1, position_2);

			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP1),
					position_1,
					Balance::MIN,
					vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
					Some(deposit_2), // WRONG deposit for position_1
				),
				Error::<Test>::PositionIdMismatch
			);
		});
}

#[test]
fn should_fail_with_nonexistent_deposit() {
	let token_amount = 2000 * ONE;
	let amount = 20 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, STABLESWAP_POOL_ID, 500000 * ONE),
			(LP1, USDT, 5000 * ONE),
		])
		.with_registered_asset(USDT)
		.with_registered_asset(STABLESWAP_POOL_ID)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(USDT, FixedU128::from_float(0.65), LP1, token_amount)
		.with_token(STABLESWAP_POOL_ID, FixedU128::from_float(0.65), LP1, token_amount)
		.build()
		.execute_with(|| {
			// Add liquidity WITHOUT farms
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				None,
				None,
			));

			let position_id = 0;

			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP1),
					position_id,
					Balance::MIN,
					vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
					Some(999), // Non-existent deposit_id
				),
				crate::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn should_fail_when_deposit_owner_differs() {
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

			// LP1 creates position with farms
			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP2),
					position_id,
					Balance::MIN,
					vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
					Some(deposit_id),
				),
				crate::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_slippage_limit_exceeded_on_omnipool_step() {
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
			//Arrange
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 2;
			let deposit_id = 1;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			assert_ok!(OmnipoolMining::add_liquidity_stableswap_omnipool_and_join_farms(
				RuntimeOrigin::signed(LP1),
				STABLESWAP_POOL_ID,
				vec![AssetAmount::new(USDT, amount)].try_into().unwrap(),
				Some(yield_farms.try_into().unwrap()),
				None,
			));

			let position_id =
				crate::OmniPositionId::<Test>::get(deposit_id).expect("Position should be mapped to deposit");

			//Act and assert
			assert_noop!(
				OmnipoolMining::remove_liquidity_stableswap_omnipool_and_exit_farms(
					RuntimeOrigin::signed(LP1),
					position_id,
					SHARES_FROM_STABLESWAP + 1,
					vec![AssetAmount::new(USDT, 1)].try_into().unwrap(),
					Some(deposit_id),
				),
				pallet_omnipool::Error::<Test>::SlippageLimit
			);
		});
}
