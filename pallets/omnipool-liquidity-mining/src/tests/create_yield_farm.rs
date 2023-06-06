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
fn create_yield_farm_should_work() {
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
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let asset = KSM;
			let multiplier = One::one();
			let loyalty_curve = Some(LoyaltyCurve::default());

			assert_ok!(OmnipoolMining::create_yield_farm(
				RuntimeOrigin::signed(GC),
				global_farm_id,
				asset,
				multiplier,
				loyalty_curve.clone()
			));

			assert_last_event!(crate::Event::YieldFarmCreated {
				global_farm_id,
				yield_farm_id: 2,
				asset_id: asset,
				multiplier,
				loyalty_curve
			}
			.into());
		});
}

#[test]
fn create_yield_farm_should_fail_with_asset_not_found_when_omnipool_doesnt_exists() {
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
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let not_in_omnipool_asset = ACA;
			let multiplier = One::one();
			let loyalty_curve = Some(LoyaltyCurve::default());

			assert_noop!(
				OmnipoolMining::create_yield_farm(
					RuntimeOrigin::signed(GC),
					global_farm_id,
					not_in_omnipool_asset,
					multiplier,
					loyalty_curve
				),
				Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn create_yield_farm_should_fail_when_origin_is_none() {
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
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let not_in_omnipool_asset = ACA;
			let multiplier = One::one();
			let loyalty_curve = Some(LoyaltyCurve::default());

			assert_noop!(
				OmnipoolMining::create_yield_farm(
					RuntimeOrigin::none(),
					global_farm_id,
					not_in_omnipool_asset,
					multiplier,
					loyalty_curve
				),
				BadOrigin
			);
		});
}
