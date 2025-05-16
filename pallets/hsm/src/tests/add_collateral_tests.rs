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
use frame_support::{assert_err, assert_noop, assert_ok, error::BadOrigin};
use hydradx_traits::stableswap::AssetAmount;
use num_traits::One;
use pallet_stableswap::types::PegSource;
use sp_runtime::FixedU128;
use sp_runtime::{Perbill, Permill};

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
				FixedU128::one(),
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
					max_buy_price_coefficient: FixedU128::one(),
					buy_back_fee: Permill::from_percent(1),
					buyback_rate: Perbill::from_percent(10),
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
					FixedU128::one(),
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
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			FixedU128::one(),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Try to add DAI as collateral again
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					DAI,
					100,
					Permill::from_percent(1),
					FixedU128::one(),
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
		.with_collateral(
			DAI,
			100,
			Permill::from_percent(1),
			FixedU128::one(),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Try to add USDC as collateral from the same pool
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					USDC,
					100,
					Permill::from_percent(1),
					FixedU128::one(),
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
					FixedU128::one(),
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
					FixedU128::one(),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::HollarNotInPool
			);
		});
}

#[test]
fn add_collateral_should_fail_when_max_is_reached() {
	let dai2 = 1000;
	let dai3 = 2000;
	let dai4 = 3000;
	let dai5 = 4000;
	let dai6 = 5000;
	let dai7 = 6000;
	let dai8 = 7000;
	let dai9 = 8000;
	let dai10 = 9000;
	let dai11 = 10000;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HOLLAR, ONE), (ALICE, DAI, ONE), (ALICE, USDC, ONE)])
		.with_registered_assets(vec![
			(HDX, 12),
			(DAI, 18),
			(USDC, 6),
			(HOLLAR, 18),
			(100, 18),
			(200, 18),
			(300, 18),
			(400, 18),
			(500, 18),
			(600, 18),
			(700, 18),
			(800, 18),
			(900, 18),
			(1000, 18),
			(1100, 18),
			(dai2, 18),
			(dai3, 18),
			(dai4, 18),
			(dai5, 18),
			(dai6, 18),
			(dai7, 18),
			(dai8, 18),
			(dai9, 18),
			(dai10, 18),
			(dai11, 18),
		])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			100,
			vec![HOLLAR, DAI],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			200,
			vec![HOLLAR, dai2],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			300,
			vec![HOLLAR, dai3],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			400,
			vec![HOLLAR, dai4],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			500,
			vec![HOLLAR, dai5],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			600,
			vec![HOLLAR, dai6],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			700,
			vec![HOLLAR, dai7],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			800,
			vec![HOLLAR, dai8],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			900,
			vec![HOLLAR, dai9],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_pool(
			1000,
			vec![HOLLAR, dai10],
			2,
			Permill::from_percent(1),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.build()
		.execute_with(|| {
			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				DAI,
				100, // pool id
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai2,
				200, // pool id
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai3,
				300,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai5,
				500,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai6,
				600,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai7,
				700,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai8,
				800,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai9,
				900,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai10,
				1000,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_ok!(HSM::add_collateral_asset(
				RuntimeOrigin::root(),
				dai4,
				400,
				Permill::from_percent(1),
				FixedU128::one(),
				Permill::from_percent(1),
				Perbill::from_percent(10),
				Some(1_000_000 * ONE),
			));

			assert_noop!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					dai11,
					1100,
					Permill::from_percent(1),
					FixedU128::one(),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::MaxNumberOfCollateralsReached
			);
		});
}


#[test]
fn add_collateral_asset_fails_when_asset_is_hollar() {
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
			FixedU128::one(),
			Permill::from_percent(1),
		)
		.build()
		.execute_with(|| {
			// Try to add DAI as collateral again
			assert_err!(
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					HOLLAR,
					100,
					Permill::from_percent(1),
					FixedU128::one(),
					Permill::from_percent(1),
					Perbill::from_percent(10),
					Some(1_000_000 * ONE),
				),
				Error::<Test>::AssetAlreadyApproved
			);
		});
}