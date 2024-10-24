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
fn exit_farm_should_work_for_multiple_farm_entries() {
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

			//Arrange
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			assert_ok!(OmnipoolMining::redeposit_shares(
				RuntimeOrigin::signed(LP1),
				charlie_g_farm_id,
				charlie_y_farm_id,
				deposit_id
			));

			set_block_number(1_000);

			//Act
			let farm_entries = vec![(deposit_id, gc_y_farm_id), (deposit_id, charlie_y_farm_id)];
			assert_ok!(OmnipoolMining::exit_farms(
				RuntimeOrigin::signed(LP1),
				farm_entries.try_into().unwrap()
			));

			//Assert
			assert!(has_event(
				crate::Event::RewardClaimed {
					global_farm_id: gc_g_farm_id,
					yield_farm_id: gc_y_farm_id,
					who: LP1,
					claimed: 97_402_500_000_u128,
					reward_currency: HDX,
					deposit_id
				}
				.into()
			));

			assert!(has_event(
				crate::Event::SharesWithdrawn {
					global_farm_id: gc_g_farm_id,
					yield_farm_id: gc_y_farm_id,
					who: LP1,
					amount: 2_000_000_000_000_000,
					deposit_id
				}
				.into()
			));

			assert!(has_event(
				crate::Event::RewardClaimed {
					global_farm_id: charlie_g_farm_id,
					yield_farm_id: charlie_y_farm_id,
					who: LP1,
					claimed: 97_402_500_000_u128,
					reward_currency: HDX,
					deposit_id
				}
				.into()
			));

			assert!(has_event(
				crate::Event::SharesWithdrawn {
					global_farm_id: charlie_g_farm_id,
					yield_farm_id: charlie_y_farm_id,
					who: LP1,
					amount: 2_000_000_000_000_000,
					deposit_id
				}
				.into()
			));

			assert_last_event!(crate::Event::DepositDestroyed { who: LP1, deposit_id }.into());

			//Storage check
			std::assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);

			//Omnipool's NFT should return to the owner
			let owner: AccountId = DummyNFT::owner(&OMNIPOOL_COLLECTION_ID, &omnipool_position_id).unwrap();
			std::assert_eq!(owner, LP1);

			//Deposit's NFT should be burned.
			let owner: Option<AccountId> = DummyNFT::owner(&LM_COLLECTION_ID, &deposit_id);
			std::assert_eq!(owner, None);
		});
}

#[test]
fn exit_farm_should_fail_with_no_origin() {
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

			//Arrange
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			assert_ok!(OmnipoolMining::redeposit_shares(
				RuntimeOrigin::signed(LP1),
				charlie_g_farm_id,
				charlie_y_farm_id,
				deposit_id
			));

			set_block_number(1_000);

			//Act and assert
			let farm_entries = vec![(deposit_id, gc_y_farm_id), (deposit_id, charlie_y_farm_id)];
			assert_noop!(
				OmnipoolMining::exit_farms(RuntimeOrigin::none(), farm_entries.try_into().unwrap()),
				BadOrigin
			);
		});
}

#[test]
fn exit_farm_should_fail_with_non_nft_owner() {
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

			//Arrange
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				gc_g_farm_id,
				gc_y_farm_id,
				omnipool_position_id
			));

			assert_ok!(OmnipoolMining::redeposit_shares(
				RuntimeOrigin::signed(LP1),
				charlie_g_farm_id,
				charlie_y_farm_id,
				deposit_id
			));

			set_block_number(1_000);

			//Act and assert
			let farm_entries = vec![(deposit_id, gc_y_farm_id), (deposit_id, charlie_y_farm_id)];
			assert_noop!(
				OmnipoolMining::exit_farms(RuntimeOrigin::signed(LP2), farm_entries.try_into().unwrap()),
				Error::<Test>::Forbidden
			);
		});
}
