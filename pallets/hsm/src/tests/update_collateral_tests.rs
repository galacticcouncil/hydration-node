// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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

use crate::tests::mock::*;
use crate::types::CollateralInfo;
use crate::{Collaterals, Error};
use frame_support::{assert_err, assert_ok, error::BadOrigin};
use frame_system::EnsureRoot;
use hydradx_traits::stableswap::AssetAmount;
use pallet_stableswap::types::PegSource;
use sp_runtime::{Perbill, Permill};

#[test]
fn update_collateral_asset_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1000 * ONE,
				},
			],
		)
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			Permill::from_percent(110),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Update purchase fee only
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				Some(Permill::from_percent(2)),
				None,
				None,
				None,
				None,
			));

			// Check that collateral was updated correctly
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(
				collateral,
				CollateralInfo {
					pool_id: 100,
					purchase_fee: Permill::from_percent(2), // Updated
					max_buy_price_coefficient: Permill::from_percent(110),
					buy_back_fee: Permill::from_percent(1),
					b: Perbill::from_percent(50), // Default from mock builder
					max_in_holding: None,
				}
			);

			// Update multiple parameters
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				Some(Permill::from_percent(120)),
				Some(Permill::from_percent(2)),
				Some(Perbill::from_percent(15)),
				Some(Some(2_000_000 * ONE)),
			));

			// Check that all parameters were updated correctly
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(
				collateral,
				CollateralInfo {
					pool_id: 100,
					purchase_fee: Permill::from_percent(2),
					max_buy_price_coefficient: Permill::from_percent(120), // Updated
					buy_back_fee: Permill::from_percent(2),                // Updated
					b: Perbill::from_percent(15),                          // Updated
					max_in_holding: Some(2_000_000 * ONE),                 // Updated
				}
			);
		});
}

#[test]
fn update_collateral_asset_removing_max_holding_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1000 * ONE,
				},
			],
		)
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			Permill::from_percent(110),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// First, set max_in_holding
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				None,
				None,
				None,
				Some(Some(1_000_000 * ONE)),
			));

			// Verify it was set
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(collateral.max_in_holding, Some(1_000_000 * ONE));

			// Now, remove max_in_holding by setting it to None
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				None,
				None,
				None,
				Some(None),
			));

			// Verify it was removed
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(collateral.max_in_holding, None);
		});
}

#[test]
fn update_collateral_asset_requires_sudo() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1000 * ONE,
				},
			],
		)
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			Permill::from_percent(110),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Attempt to update collateral as ALICE (not sudo)
			assert_err!(
				HSM::update_collateral_asset(
					RuntimeOrigin::signed(ALICE),
					DAI,
					Some(Permill::from_percent(2)),
					None,
					None,
					None,
					None,
				),
				BadOrigin
			);
		});
}

#[test]
fn update_collateral_asset_fails_when_asset_not_approved() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE), (ALICE, USDC, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (USDC, 6), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1000 * ONE,
				},
			],
		)
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			Permill::from_percent(110),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Try to update USDC which is not a collateral
			assert_err!(
				HSM::update_collateral_asset(
					RuntimeOrigin::root(),
					USDC,
					Some(Permill::from_percent(2)),
					None,
					None,
					None,
					None,
				),
				Error::<Test>::AssetNotApproved
			);
		});
}

#[test]
fn update_collateral_asset_with_no_changes_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1000 * ONE,
				},
			],
		)
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			Permill::from_percent(110),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Get the original collateral info
			let original_info = Collaterals::<Test>::get(DAI).unwrap();

			// Call update with no new values (all None)
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				None,
				None,
				None,
				None,
			));

			// Check that collateral info remains unchanged
			let updated_info = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(original_info, updated_info);
		});
}
