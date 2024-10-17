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
fn withdraw_shares_should_unlock_omnipool_position_when_last_entry_in_deposit() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 2
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit shares
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				global_farm_id,
				yield_farm_id,
				omnipool_position_id
			));

			set_block_number(1_000);

			//Act
			assert_ok!(OmnipoolMining::withdraw_shares(
				RuntimeOrigin::signed(LP1),
				deposit_id,
				yield_farm_id,
			));

			//Assert
			assert!(has_event(
				crate::Event::RewardClaimed {
					global_farm_id,
					yield_farm_id,
					who: LP1,
					claimed: 97_402_500_000_u128,
					reward_currency: HDX,
					deposit_id
				}
				.into()
			));

			assert!(has_event(
				crate::Event::SharesWithdrawn {
					global_farm_id,
					yield_farm_id,
					who: LP1,
					amount: 2_000_000_000_000_000,
					deposit_id
				}
				.into()
			));

			assert_last_event!(crate::Event::DepositDestroyed { who: LP1, deposit_id }.into());

			//Storage check
			assert_eq!(crate::OmniPositionId::<Test>::get(deposit_id), None);

			//Omnipool's NFT should return to the owner
			let owner: AccountId = DummyNFT::owner(&OMNIPOOL_COLLECTION_ID, &omnipool_position_id).unwrap();
			assert_eq!(owner, LP1);

			//Deposit's NFT should be burned.
			let owner: Option<AccountId> = DummyNFT::owner(&LM_COLLECTION_ID, &deposit_id);
			assert_eq!(owner, None);
		});
}

#[test]
fn withdraw_shares_should_not_unlock_omnipool_position_when_deposit_is_not_burned() {
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

			//Arrange: deposit shares & redeposit
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

			//Act
			assert_ok!(OmnipoolMining::withdraw_shares(
				RuntimeOrigin::signed(LP1),
				deposit_id,
				gc_y_farm_id,
			));

			//Assert
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

			//Storage check: storage should not change as deposit was not destroyed.
			assert_eq!(
				crate::OmniPositionId::<Test>::get(deposit_id).unwrap(),
				omnipool_position_id
			);

			//Omnipool's NFT should not changed as deposit was not destroyed.
			let lm_account = OmnipoolMining::account_id();
			let owner: AccountId = DummyNFT::owner(&OMNIPOOL_COLLECTION_ID, &omnipool_position_id).unwrap();
			assert_eq!(owner, lm_account);

			//Deposit's NFT should not change as deposit was not destroyed.
			let owner: AccountId = DummyNFT::owner(&LM_COLLECTION_ID, &deposit_id).unwrap();
			assert_eq!(owner, LP1);
		});
}

#[test]
fn withdraw_shares_should_fail_when_origin_is_none() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 2
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit shares
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				global_farm_id,
				yield_farm_id,
				omnipool_position_id
			));

			//Act
			assert_noop!(
				OmnipoolMining::withdraw_shares(RuntimeOrigin::none(), deposit_id, yield_farm_id,),
				BadOrigin
			);
		});
}

#[test]
fn withdraw_shares_should_fail_with_not_deposit_owner_when_account_is_not_owner() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 2
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit shares
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				global_farm_id,
				yield_farm_id,
				omnipool_position_id
			));

			//Act
			assert_noop!(
				OmnipoolMining::withdraw_shares(RuntimeOrigin::signed(ALICE), deposit_id, yield_farm_id,),
				crate::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn withdraw_shares_should_fail_with_not_deposit_owner_when_nft_is_missing() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 2
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Arrange: deposit shares
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				global_farm_id,
				yield_farm_id,
				omnipool_position_id
			));

			DummyNFT::burn(&LM_COLLECTION_ID, &deposit_id, None::<&AccountId>).unwrap();

			//Act
			assert_noop!(
				OmnipoolMining::withdraw_shares(RuntimeOrigin::signed(ALICE), deposit_id, yield_farm_id,),
				crate::Error::<Test>::Forbidden
			);
		});
}
