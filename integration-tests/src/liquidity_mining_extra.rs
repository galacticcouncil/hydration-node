// This file is part of HydraDX-node.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

//! Integration coverage for the yield-farm lifecycle admin extrinsics of both liquidity-mining
//! pallets: `stop_yield_farm`, `resume_yield_farm`, `update_yield_farm`, `terminate_yield_farm`,
//! `terminate_global_farm` (and `update_global_farm` for xyk).

#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::{
	Balances, OmnipoolLiquidityMining, OmnipoolWarehouseLM, Runtime, RuntimeOrigin, XYKLiquidityMining, XYKWarehouseLM,
	XYK,
};
use hydradx_traits::AMM;
use pallet_xyk::types::AssetPair;
use sp_runtime::{traits::One, FixedU128, Permill, Perquintill};
use warehouse_liquidity_mining::{GlobalFarmId, Instance1, Instance2, LoyaltyCurve, YieldFarmId};
use xcm_emulator::TestExt;

// ---------------------------------------------------------------------------
// Omnipool liquidity mining
// ---------------------------------------------------------------------------

mod omnipool {
	use super::*;

	/// Creates a running omnipool yield farm and returns `(global_farm_id, yield_farm_id)`.
	fn setup_running_farm() -> (GlobalFarmId, YieldFarmId) {
		init_omnipool();

		// The farm asset must be an omnipool token; `init_omnipool` only seeds HDX and DAI.
		assert_ok!(hydradx_runtime::Omnipool::add_token(
			RuntimeOrigin::root(),
			DOT,
			FixedU128::from_inner(25_650_000_000_000_000_000),
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		go_to_block(100);
		let owner = Treasury::account_id();
		let total_rewards = 1_000_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			owner.clone(),
			total_rewards,
		));
		assert_ok!(OmnipoolLiquidityMining::create_global_farm(
			RuntimeOrigin::root(),
			total_rewards,
			1_000_000,
			10,
			HDX,
			owner.clone(),
			Perquintill::from_parts(570_776_255_707),
			1_000,
			FixedU128::one(),
		));

		go_to_block(200);
		assert_ok!(OmnipoolLiquidityMining::create_yield_farm(
			RuntimeOrigin::signed(owner),
			1,
			DOT,
			FixedU128::one(),
			Some(LoyaltyCurve::default()),
		));

		(1, 2)
	}

	#[test]
	fn stop_yield_farm_should_deactivate_yield_farm_when_called_by_owner() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id) = setup_running_farm();
			assert_eq!(
				OmnipoolWarehouseLM::active_yield_farm(DOT, global_farm_id),
				Some(yield_farm_id)
			);

			go_to_block(300);
			assert_ok!(OmnipoolLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				DOT,
			));

			assert_eq!(OmnipoolWarehouseLM::active_yield_farm(DOT, global_farm_id), None);
			expect_hydra_events(vec![pallet_omnipool_liquidity_mining::Event::YieldFarmStopped {
				global_farm_id,
				yield_farm_id,
				asset_id: DOT,
				who: Treasury::account_id(),
			}
			.into()]);
		});
	}

	#[test]
	fn resume_yield_farm_should_reactivate_yield_farm_when_previously_stopped() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id) = setup_running_farm();

			go_to_block(300);
			assert_ok!(OmnipoolLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				DOT,
			));
			assert_eq!(OmnipoolWarehouseLM::active_yield_farm(DOT, global_farm_id), None);

			go_to_block(400);
			let multiplier = FixedU128::from(2);
			assert_ok!(OmnipoolLiquidityMining::resume_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				yield_farm_id,
				DOT,
				multiplier,
			));

			assert_eq!(
				OmnipoolWarehouseLM::active_yield_farm(DOT, global_farm_id),
				Some(yield_farm_id)
			);
			expect_hydra_events(vec![pallet_omnipool_liquidity_mining::Event::YieldFarmResumed {
				global_farm_id,
				yield_farm_id,
				asset_id: DOT,
				who: Treasury::account_id(),
				multiplier,
			}
			.into()]);
		});
	}

	#[test]
	fn update_yield_farm_should_change_multiplier_when_called_by_owner() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id) = setup_running_farm();

			go_to_block(300);
			let new_multiplier = FixedU128::from(5);
			assert_ok!(OmnipoolLiquidityMining::update_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				DOT,
				new_multiplier,
			));

			expect_hydra_events(vec![pallet_omnipool_liquidity_mining::Event::YieldFarmUpdated {
				global_farm_id,
				yield_farm_id,
				asset_id: DOT,
				who: Treasury::account_id(),
				multiplier: new_multiplier,
			}
			.into()]);
		});
	}

	#[test]
	fn terminate_yield_farm_should_remove_yield_farm_when_stopped_and_empty() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id) = setup_running_farm();

			go_to_block(300);
			assert_ok!(OmnipoolLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				DOT,
			));

			go_to_block(400);
			assert_ok!(OmnipoolLiquidityMining::terminate_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				yield_farm_id,
				DOT,
			));

			// Empty yield farm is removed from storage immediately.
			assert!(warehouse_liquidity_mining::YieldFarm::<Runtime, Instance1>::get((
				DOT,
				global_farm_id,
				yield_farm_id
			))
			.is_none());
			expect_hydra_events(vec![pallet_omnipool_liquidity_mining::Event::YieldFarmTerminated {
				global_farm_id,
				yield_farm_id,
				asset_id: DOT,
				who: Treasury::account_id(),
			}
			.into()]);
		});
	}

	#[test]
	fn terminate_global_farm_should_remove_global_farm_when_all_yield_farms_terminated() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id) = setup_running_farm();

			go_to_block(300);
			assert_ok!(OmnipoolLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				DOT,
			));
			assert_ok!(OmnipoolLiquidityMining::terminate_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				yield_farm_id,
				DOT,
			));

			go_to_block(400);
			assert_ok!(OmnipoolLiquidityMining::terminate_global_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
			));

			assert!(OmnipoolWarehouseLM::global_farm(global_farm_id).is_none());
			expect_hydra_events(vec![pallet_omnipool_liquidity_mining::Event::GlobalFarmTerminated {
				global_farm_id,
				who: Treasury::account_id(),
				reward_currency: HDX,
				undistributed_rewards: 1_000_000 * UNITS,
			}
			.into()]);
		});
	}

	#[test]
	fn terminate_global_farm_should_fail_when_yield_farm_still_exists() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, _) = setup_running_farm();

			go_to_block(300);
			assert_noop!(
				OmnipoolLiquidityMining::terminate_global_farm(
					RuntimeOrigin::signed(Treasury::account_id()),
					global_farm_id,
				),
				warehouse_liquidity_mining::Error::<Runtime, Instance1>::GlobalFarmIsNotEmpty
			);
		});
	}

	#[test]
	fn terminate_yield_farm_should_fail_when_yield_farm_is_still_active() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id) = setup_running_farm();

			go_to_block(300);
			assert_noop!(
				OmnipoolLiquidityMining::terminate_yield_farm(
					RuntimeOrigin::signed(Treasury::account_id()),
					global_farm_id,
					yield_farm_id,
					DOT,
				),
				warehouse_liquidity_mining::Error::<Runtime, Instance1>::LiquidityMiningIsActive
			);
		});
	}
}

// ---------------------------------------------------------------------------
// XYK liquidity mining
// ---------------------------------------------------------------------------

mod xyk {
	use super::*;

	fn create_xyk_pool(asset_a: AssetId, amount_a: Balance, asset_b: AssetId, amount_b: Balance) {
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			DAVE.into(),
			asset_a,
			(amount_a + UNITS) as i128,
		));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			DAVE.into(),
			asset_b,
			(amount_b + UNITS) as i128,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(DAVE.into()),
			asset_a,
			amount_a,
			asset_b,
			amount_b,
		));
	}

	/// Creates a running xyk yield farm and returns `(global_farm_id, yield_farm_id, asset_pair, amm_pool_id)`.
	fn setup_running_farm() -> (GlobalFarmId, YieldFarmId, AssetPair, AccountId) {
		let asset_pair = AssetPair {
			asset_in: PEPE,
			asset_out: ACA,
		};
		let amm_pool_id = <Runtime as pallet_xyk_liquidity_mining::Config>::AMM::get_pair_id(asset_pair);

		go_to_block(100);
		let owner = Treasury::account_id();
		let total_rewards = 1_000_000 * UNITS;
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			owner.clone(),
			HDX,
			total_rewards as i128 + 1_000,
		));
		assert_ok!(XYKLiquidityMining::create_global_farm(
			RuntimeOrigin::root(),
			total_rewards,
			1_000_000,
			10,
			PEPE,
			HDX,
			owner.clone(),
			Perquintill::from_parts(570_776_255_707),
			1_000,
			FixedU128::one(),
		));

		create_xyk_pool(
			asset_pair.asset_in,
			1_000_000 * UNITS,
			asset_pair.asset_out,
			10_000_000 * UNITS,
		);

		go_to_block(200);
		assert_ok!(XYKLiquidityMining::create_yield_farm(
			RuntimeOrigin::signed(owner),
			1,
			asset_pair,
			FixedU128::one(),
			Some(LoyaltyCurve::default()),
		));

		(1, 2, asset_pair, amm_pool_id)
	}

	#[test]
	fn stop_yield_farm_should_deactivate_yield_farm_when_called_by_owner() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id, asset_pair, amm_pool_id) = setup_running_farm();
			assert_eq!(
				XYKWarehouseLM::active_yield_farm(amm_pool_id.clone(), global_farm_id),
				Some(yield_farm_id)
			);

			go_to_block(300);
			assert_ok!(XYKLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				asset_pair,
			));

			assert_eq!(XYKWarehouseLM::active_yield_farm(amm_pool_id, global_farm_id), None);
			expect_hydra_events(vec![pallet_xyk_liquidity_mining::Event::YieldFarmStopped {
				global_farm_id,
				yield_farm_id,
				who: Treasury::account_id(),
				asset_pair,
			}
			.into()]);
		});
	}

	#[test]
	fn resume_yield_farm_should_reactivate_yield_farm_when_previously_stopped() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id, asset_pair, amm_pool_id) = setup_running_farm();

			go_to_block(300);
			assert_ok!(XYKLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				asset_pair,
			));
			assert_eq!(
				XYKWarehouseLM::active_yield_farm(amm_pool_id.clone(), global_farm_id),
				None
			);

			go_to_block(400);
			let multiplier = FixedU128::from(2);
			assert_ok!(XYKLiquidityMining::resume_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				yield_farm_id,
				asset_pair,
				multiplier,
			));

			assert_eq!(
				XYKWarehouseLM::active_yield_farm(amm_pool_id, global_farm_id),
				Some(yield_farm_id)
			);
			expect_hydra_events(vec![pallet_xyk_liquidity_mining::Event::YieldFarmResumed {
				global_farm_id,
				yield_farm_id,
				who: Treasury::account_id(),
				asset_pair,
				multiplier,
			}
			.into()]);
		});
	}

	#[test]
	fn update_yield_farm_should_change_multiplier_when_called_by_owner() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id, asset_pair, _) = setup_running_farm();

			go_to_block(300);
			let new_multiplier = FixedU128::from(5);
			assert_ok!(XYKLiquidityMining::update_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				asset_pair,
				new_multiplier,
			));

			expect_hydra_events(vec![pallet_xyk_liquidity_mining::Event::YieldFarmUpdated {
				global_farm_id,
				yield_farm_id,
				who: Treasury::account_id(),
				asset_pair,
				multiplier: new_multiplier,
			}
			.into()]);
		});
	}

	#[test]
	fn update_global_farm_should_change_price_adjustment_when_called_by_owner() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, _, _, _) = setup_running_farm();

			go_to_block(300);
			let price_adjustment = FixedU128::from(3);
			assert_ok!(XYKLiquidityMining::update_global_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				price_adjustment,
			));

			expect_hydra_events(vec![pallet_xyk_liquidity_mining::Event::GlobalFarmUpdated {
				id: global_farm_id,
				price_adjustment,
			}
			.into()]);
		});
	}

	#[test]
	fn terminate_yield_farm_should_remove_yield_farm_when_stopped_and_empty() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id, asset_pair, amm_pool_id) = setup_running_farm();

			go_to_block(300);
			assert_ok!(XYKLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				asset_pair,
			));

			go_to_block(400);
			assert_ok!(XYKLiquidityMining::terminate_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				yield_farm_id,
				asset_pair,
			));

			// Empty yield farm is removed from storage immediately.
			assert!(warehouse_liquidity_mining::YieldFarm::<Runtime, Instance2>::get((
				amm_pool_id,
				global_farm_id,
				yield_farm_id
			))
			.is_none());
			expect_hydra_events(vec![pallet_xyk_liquidity_mining::Event::YieldFarmTerminated {
				global_farm_id,
				yield_farm_id,
				who: Treasury::account_id(),
				asset_pair,
			}
			.into()]);
		});
	}

	#[test]
	fn terminate_global_farm_should_remove_global_farm_when_all_yield_farms_terminated() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id, asset_pair, _) = setup_running_farm();

			go_to_block(300);
			assert_ok!(XYKLiquidityMining::stop_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				asset_pair,
			));
			assert_ok!(XYKLiquidityMining::terminate_yield_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
				yield_farm_id,
				asset_pair,
			));

			go_to_block(400);
			assert_ok!(XYKLiquidityMining::terminate_global_farm(
				RuntimeOrigin::signed(Treasury::account_id()),
				global_farm_id,
			));

			assert!(XYKWarehouseLM::global_farm(global_farm_id).is_none());
			expect_hydra_events(vec![pallet_xyk_liquidity_mining::Event::GlobalFarmTerminated {
				global_farm_id,
				who: Treasury::account_id(),
				reward_currency: HDX,
				undistributed_rewards: 1_000_000 * UNITS,
			}
			.into()]);
		});
	}

	#[test]
	fn terminate_global_farm_should_fail_when_yield_farm_still_exists() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, _, _, _) = setup_running_farm();

			go_to_block(300);
			assert_noop!(
				XYKLiquidityMining::terminate_global_farm(
					RuntimeOrigin::signed(Treasury::account_id()),
					global_farm_id,
				),
				warehouse_liquidity_mining::Error::<Runtime, Instance2>::GlobalFarmIsNotEmpty
			);
		});
	}

	#[test]
	fn terminate_yield_farm_should_fail_when_yield_farm_is_still_active() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let (global_farm_id, yield_farm_id, asset_pair, _) = setup_running_farm();

			go_to_block(300);
			assert_noop!(
				XYKLiquidityMining::terminate_yield_farm(
					RuntimeOrigin::signed(Treasury::account_id()),
					global_farm_id,
					yield_farm_id,
					asset_pair,
				),
				warehouse_liquidity_mining::Error::<Runtime, Instance2>::LiquidityMiningIsActive
			);
		});
	}
}
