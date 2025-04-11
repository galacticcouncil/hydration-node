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
use crate::{CollateralHoldings, Collaterals, Error};
use frame_support::{assert_err, assert_ok, error::BadOrigin};
use hydradx_traits::stableswap::AssetAmount;
use pallet_stableswap::types::PegSource;
use sp_runtime::Permill;

#[test]
fn remove_collateral_asset_works() {
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
		.with_collateral(DAI, 100, Permill::from_percent(1), (100, 100), Permill::from_percent(1))
		.build()
		.execute_with(|| {
			// Remove DAI as collateral
			assert_ok!(HSM::remove_collateral_asset(RuntimeOrigin::root(), DAI));

			// Check that collateral was removed
			assert!(Collaterals::<Test>::get(DAI).is_none());
		});
}

#[test]
fn remove_collateral_asset_requires_sudo() {
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
		.with_collateral(DAI, 100, Permill::from_percent(1), (100, 100), Permill::from_percent(1))
		.build()
		.execute_with(|| {
			// Attempt to remove collateral as ALICE (not sudo)
			assert_err!(
				HSM::remove_collateral_asset(RuntimeOrigin::signed(ALICE), DAI),
				BadOrigin
			);
		});
}

#[test]
fn remove_collateral_asset_fails_when_asset_not_approved() {
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
		.with_collateral(DAI, 100, Permill::from_percent(1), (100, 100), Permill::from_percent(1))
		.build()
		.execute_with(|| {
			// Try to remove USDC which is not a collateral
			assert_err!(
				HSM::remove_collateral_asset(RuntimeOrigin::root(), USDC),
				Error::<Test>::AssetNotApproved
			);
		});
}

#[test]
fn remove_collateral_asset_fails_when_collateral_not_empty() {
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
		.with_collateral(DAI, 100, Permill::from_percent(1), (100, 100), Permill::from_percent(1))
		.build()
		.execute_with(|| {
			// Set some collateral holdings
			CollateralHoldings::<Test>::insert(DAI, 100 * ONE);

			// Try to remove DAI as collateral when it still has holdings
			assert_err!(
				HSM::remove_collateral_asset(RuntimeOrigin::root(), DAI),
				Error::<Test>::CollateralNotEmpty
			);
		});
}

#[test]
fn remove_collateral_asset_works_with_zero_holdings() {
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
		.with_collateral(DAI, 100, Permill::from_percent(1), (100, 100), Permill::from_percent(1))
		.build()
		.execute_with(|| {
			// Explicitly set collateral holdings to zero
			CollateralHoldings::<Test>::insert(DAI, 0);

			// Should be able to remove DAI as collateral with zero holdings
			assert_ok!(HSM::remove_collateral_asset(RuntimeOrigin::root(), DAI));

			// Check that collateral was removed
			assert!(Collaterals::<Test>::get(DAI).is_none());
		});
}
