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

use pretty_assertions::assert_eq;

#[test]
fn redeposit_shares_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_global_farm(
			//id: 1
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
			//id: 2
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			CHARLIE,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 3
		.with_yield_farm(CHARLIE, 2, KSM, FixedU128::one(), None) //id: 4
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 3;
			let charlie_g_farm_id = 2;
			let charlie_y_farm_id = 4;
			let omnipool_position_id = 2;
			let deposit_id = 1;
			let asset_in_position = KSM;

			//Arrange
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			//Act
			assert_ok!(OmnipoolMining::redeposit_shares(
				RuntimeOrigin::signed(LP1),
				charlie_g_farm_id,
				charlie_y_farm_id,
				deposit_id
			));

			//Assert
			assert_last_event!(crate::Event::SharesRedeposited {
				global_farm_id: charlie_g_farm_id,
				yield_farm_id: charlie_y_farm_id,
				deposit_id,
				asset_id: asset_in_position,
				who: LP1,
				shares_amount: 2_000_000_000_000_000,
				position_id: omnipool_position_id
			}
			.into());
		});
}

#[test]
fn redeposit_shares_should_fail_with_asset_not_found_when_omnipool_doesnt_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE) //pos_id: 0
		.with_liquidity(ALICE, KSM, 5_000 * ONE) //pos_id: 1
		.with_global_farm(
			//id: 1
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
			//id: 2
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			CHARLIE,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 3
		.with_yield_farm(CHARLIE, 2, KSM, FixedU128::one(), None) //id: 4
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 3;
			let charlie_g_farm_id = 2;
			let charlie_y_farm_id = 4;
			let omnipool_position_id = 3;
			let deposit_id = 1;

			//Arrange: deposit position and remove asset from omnipool
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(ALICE),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			assert_ok!(Omnipool::remove_asset(KSM));

			//Act & assert
			assert_noop!(
				OmnipoolMining::redeposit_shares(
					RuntimeOrigin::signed(ALICE),
					charlie_g_farm_id,
					charlie_y_farm_id,
					deposit_id
				),
				crate::Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn redeposit_shares_should_fail_with_not_deposit_owner_when_account_is_not_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE) //pos_id: 0
		.with_global_farm(
			//id: 1
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
			//id: 2
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			CHARLIE,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 3
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 3;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit position and remove asset from omnipool
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			//Act & assert
			assert_noop!(
				OmnipoolMining::redeposit_shares(RuntimeOrigin::signed(ALICE), gc_g_farm_id, gc_y_farm_id, deposit_id),
				crate::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn redeposit_shares_should_fail_when_origin_is_none() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE) //pos_id: 0
		.with_global_farm(
			//id: 1
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
			//id: 2
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			CHARLIE,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 3
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 3;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit position
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			//Act & assert
			assert_noop!(
				OmnipoolMining::redeposit_shares(RuntimeOrigin::none(), gc_g_farm_id, gc_y_farm_id, deposit_id),
				BadOrigin
			);
		});
}

#[test]
fn redeposit_shares_should_fail_with_cant_find_deposit_owner_when_nft_is_missing() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, 2000 * ONE) //pos_id: 0
		.with_global_farm(
			//id: 1
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
			//id: 2
			80_000_000 * ONE,
			2_628_000,
			1,
			HDX,
			CHARLIE,
			Perquintill::from_float(0.000_000_15_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 3
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 3;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit position and burn lm deposit's nft
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			DummyNFT::burn(&LM_COLLECTION_ID, &deposit_id, None::<&AccountId>).unwrap();

			//Act & assert
			assert_noop!(
				OmnipoolMining::redeposit_shares(RuntimeOrigin::signed(LP1), gc_g_farm_id, gc_y_farm_id, deposit_id),
				crate::Error::<Test>::Forbidden
			);
		});
}
