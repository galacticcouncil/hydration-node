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
use sp_runtime::{DispatchError, Perbill, Permill};

#[test]
fn add_collateral_asset_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE), (ALICE, USDC, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (USDC, 6), (HOLLAR, 18), (100, 18)])
		// Create a stablepool for HOLLAR and DAI
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
		.build()
		.execute_with(|| {
			// Add DAI as collateral
			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				100, // pool id
				Permill::from_percent(1),
				(100, 100),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			// Check that collateral was added correctly
			let collateral = Collaterals::<Test>::get(DAI).unwrap();
			assert_eq!(
				collateral,
				CollateralInfo {
					pool_id: 100,
					purchase_fee: Permill::from_percent(1),
					max_buy_price_coefficient: (100, 100),
					buy_back_fee: Permill::from_percent(1),
					b: Perbill::from_percent(10),
					max_in_holding: Some(1_000_000 * ONE),
				}
			);
		});
}

#[test]
fn add_collateral_asset_requires_sudo() {
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
		.build()
		.execute_with(|| {
			// Attempt to add collateral as ALICE (not sudo)
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::signed(ALICE),
					DAI,
					100,
					Permill::from_percent(1),
					(100, 100),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				BadOrigin
			);
		});
}

#[test]
fn add_collateral_asset_fails_when_asset_already_approved() {
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
			// Try to add DAI as collateral again
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					DAI,
					100,
					Permill::from_percent(1),
					(100, 100),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::AssetAlreadyApproved
			);
		});
}

#[test]
fn add_collateral_asset_fails_when_pool_already_has_collateral() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE), (ALICE, USDC, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (USDC, 18), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![HOLLAR, DAI, USDC],
			2,
			Permill::from_percent(1),
			vec![
				PegSource::Value((1, 1)),
				PegSource::Value((1, 1)),
				PegSource::Value((1, 1)),
			],
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
				AssetAmount {
					asset_id: USDC,
					amount: 1000 * ONE,
				},
			],
		)
		.with_collateral(DAI, 100, Permill::from_percent(1), (100, 100), Permill::from_percent(1))
		.build()
		.execute_with(|| {
			// Try to add USDC as collateral from the same pool
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					USDC,
					100,
					Permill::from_percent(1),
					(100, 100),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::PoolAlreadyHasCollateral
			);
		});
}

#[test]
fn add_collateral_asset_fails_when_asset_not_in_pool() {
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
		.build()
		.execute_with(|| {
			// Try to add USDC as collateral but it's not in the pool
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					USDC,
					100,
					Permill::from_percent(1),
					(100, 100),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::AssetNotInPool
			);
		});
}

#[test]
fn add_collateral_asset_fails_when_hollar_not_in_pool() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, ONE), (ALICE, USDC, ONE)])
		.with_registered_assets(vec![(HDX, 12), (DAI, 18), (USDC, 18), (HOLLAR, 18), (100, 18)])
		.with_pool(
			100,
			vec![DAI, USDC],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: DAI,
					amount: 1000 * ONE,
				},
				AssetAmount {
					asset_id: USDC,
					amount: 1000 * ONE,
				},
			],
		)
		.build()
		.execute_with(|| {
			// Try to add DAI as collateral but HOLLAR is not in the pool
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					DAI,
					100,
					Permill::from_percent(1),
					(100, 100),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::HollarNotInPool
			);
		});
}
