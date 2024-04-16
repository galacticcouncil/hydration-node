// This file is part of Basilisk-node.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

use super::*;

#[test]
fn update_global_farm_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE)])
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			BOB,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.build()
		.execute_with(|| {
			let new_price_adjustment = FixedU128::from_float(234.1234_f64);

			set_block_number(100_000);

			//Act
			assert_ok!(LiquidityMining::update_global_farm(
				Origin::signed(BOB),
				1,
				new_price_adjustment
			));

			//Assert
			assert_last_event!(crate::Event::GlobalFarmUpdated {
				id: 1,
				price_adjustment: new_price_adjustment,
			}
			.into());
		});
}

#[test]
fn update_global_farm_should_fail_when_origin_is_not_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE)])
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			BOB,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.build()
		.execute_with(|| {
			assert_noop!(
				LiquidityMining::update_global_farm(Origin::none(), BOB_FARM, FixedU128::from_float(0.268_756_6)),
				BadOrigin
			);
		});
}
