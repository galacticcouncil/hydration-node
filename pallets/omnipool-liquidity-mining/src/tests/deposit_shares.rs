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

use pallet_liquidity_mining::{DepositData, YieldFarmEntry};
use pretty_assertions::assert_eq;

#[test]
fn deposit_shares_should_work() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 2;
			let deposit_id = 1;

			//Act
			assert_ok!(OmnipoolMining::deposit_shares(
				RuntimeOrigin::signed(LP1),
				global_farm_id,
				yield_farm_id,
				omnipool_position_id
			));

			//Assert
			assert_last_event!(crate::Event::SharesDeposited {
				global_farm_id,
				yield_farm_id,
				deposit_id,
				asset_id: KSM,
				who: LP1,
				shares_amount: 2_000_000_000_000_000,
				position_id: omnipool_position_id
			}
			.into());

			//Storage check
			assert_eq!(
				crate::OmniPositionId::<Test>::get(deposit_id).unwrap(),
				omnipool_position_id
			);

			let deposit =
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id).unwrap();
			let mut expected_deposit = DepositData::new(2_000_000_000_000_000, KSM);
			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					global_farm_id,
					yield_farm_id,
					1_300_000_000_000_000,
					FixedU128::zero(),
					1,
					0,
				))
				.unwrap();

			assert_eq!(deposit, expected_deposit);

			//NFT check: lm account should be owner of the omnipool position.
			let lm_account = OmnipoolMining::account_id();
			let owner: AccountId = DummyNFT::owner(&OMNIPOOL_COLLECTION_ID, &omnipool_position_id).unwrap();
			assert_eq!(owner, lm_account);

			//NFT check: lm deposit should be minted for user.
			let owner: AccountId = DummyNFT::owner(&LM_COLLECTION_ID, &deposit_id).unwrap();
			assert_eq!(owner, LP1);
		});
}

#[test]
fn deposit_shares_should_fail_with_forbidden_when_account_is_not_omnipool_position_owner() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 4;
			let not_position_owner = BOB;

			//Act
			assert_noop!(
				OmnipoolMining::deposit_shares(
					RuntimeOrigin::signed(not_position_owner),
					global_farm_id,
					yield_farm_id,
					omnipool_position_id
				),
				pallet_omnipool::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn deposit_shares_should_fail_with_forbidden_when_omnipool_posotion_doesnt_exists() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let non_existing_position_id = 1_000_000;

			//Act
			assert_noop!(
				OmnipoolMining::deposit_shares(
					RuntimeOrigin::signed(BOB),
					global_farm_id,
					yield_farm_id,
					non_existing_position_id,
				),
				pallet_omnipool::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn deposit_shares_should_fail_when_origin_is_none() {
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
		.with_liquidity(ALICE, KSM, 5_000 * ONE)
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 3;

			//Act & assert
			assert_noop!(
				OmnipoolMining::deposit_shares(
					RuntimeOrigin::none(),
					global_farm_id,
					yield_farm_id,
					omnipool_position_id
				),
				BadOrigin
			);
		});
}

#[test]
fn deposit_shares_should_fail_with_asset_not_found_when_omnipool_doesnt_exists() {
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let non_existing_position_id = 1_000_000;

			//Act
			assert_noop!(
				OmnipoolMining::deposit_shares(
					RuntimeOrigin::signed(BOB),
					global_farm_id,
					yield_farm_id,
					non_existing_position_id,
				),
				pallet_omnipool::Error::<Test>::Forbidden
			);
		});
}

#[test]
fn deposit_shares_should_fail_with_asset_not_found_when_omnipool_deosnt_exists() {
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
		.with_liquidity(ALICE, KSM, 5_000 * ONE)
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
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None)
		.build()
		.execute_with(|| {
			let global_farm_id = 1;
			let yield_farm_id = 2;
			let omnipool_position_id = 3;

			//Arrange: remove asset from omnipool
			assert_ok!(Omnipool::remove_asset(KSM));

			//Act & assert
			assert_noop!(
				OmnipoolMining::deposit_shares(
					RuntimeOrigin::signed(ALICE),
					global_farm_id,
					yield_farm_id,
					omnipool_position_id
				),
				crate::Error::<Test>::AssetNotFound
			);
		});
}
