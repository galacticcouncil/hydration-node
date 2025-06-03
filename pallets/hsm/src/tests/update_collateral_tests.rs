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
use crate::{CollateralInfo, Collaterals, Error};
use frame_support::{assert_err, assert_ok};
use hydradx_traits::stableswap::AssetAmount;
use pallet_stableswap::types::PegSource;
use sp_runtime::{FixedU128, Perbill, Permill};

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
			FixedU128::from_rational(110, 100), // 110% as a ratio
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
					max_buy_price_coefficient: FixedU128::from_rational(110, 100),
					buy_back_fee: Permill::from_percent(1),
					buyback_rate: Perbill::from_percent(50), // Default from mock builder
					max_in_holding: None,
				}
			);

			// Update multiple parameters
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				Some(FixedU128::from_rational(120, 100)), // 120% as a ratio
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
					max_buy_price_coefficient: FixedU128::from_rational(120, 100), // Updated
					buy_back_fee: Permill::from_percent(2),                        // Updated
					buyback_rate: Perbill::from_percent(15),                       // Updated
					max_in_holding: Some(2_000_000 * ONE),                         // Updated
				}
			);

			// Check that the collateral update can remove max_in_holding
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				None,
				None,
				None,
				Some(None),
			));

			// Check that max_in_holding was removed
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(collateral.max_in_holding, None);

			// Update with a different ratio for max_buy_price_coefficient
			assert_ok!(HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				None,
				Some(FixedU128::from_rational(200, 100)), // 200% as a ratio (greater than 100%)
				None,
				None,
				None,
			));

			// Check the ratio was updated correctly
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(collateral.max_buy_price_coefficient, FixedU128::from_rational(200, 100));
		});
}

#[test]
fn update_collateral_asset_fails_for_non_existent_asset() {
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
			FixedU128::from_rational(110, 100), // 110% as a ratio
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Try to update a non-existent collateral
			assert_err!(
				HSM::update_collateral_asset(
					RuntimeOrigin::root(),
					HDX, // HDX is not a collateral
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
fn update_collateral_asset_fails_for_non_root() {
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
			FixedU128::from_rational(110, 100), // 110% as a ratio
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Try to update as a non-root user
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
				sp_runtime::DispatchError::BadOrigin
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
			FixedU128::from_rational(110, 100), // 110% as a ratio
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
