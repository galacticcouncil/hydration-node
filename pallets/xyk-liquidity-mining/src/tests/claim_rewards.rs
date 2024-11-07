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
fn claim_rewards_should_be_disabled() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX_KSM_SHARE_ID, 200 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			ALICE,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_yield_farm(ALICE, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_deposit(CHARLIE, 1, 2, BSX_KSM_ASSET_PAIR, 100 * ONE)
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(10_000);

			//Act
			assert_noop!(
				LiquidityMining::claim_rewards(Origin::signed(CHARLIE), 1, 2),
				Error::<Test>::Disabled
			);
		});
}
