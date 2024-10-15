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
use pallet_omnipool::types::AssetReserveState;
use pallet_omnipool::types::Tradability;
use pretty_assertions::assert_eq;
#[macro_export]
macro_rules! assert_asset_state {
	( $x:expr, $y:expr) => {{
		let reserve = Tokens::free_balance($x, &Omnipool::protocol_account());
		assert_eq!(reserve, $y.reserve);

		let actual = pallet_omnipool::Pallet::<Test>::load_asset_state($x).unwrap();
		assert_eq!(actual, $y.into());
	}};
}

#[test]
fn add_liquidity_and_join_farms_should_work_with_single_yield_farm() {
	let token_amount = 2000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(BOB, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, token_amount)
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
		.with_global_farm(
			//id: 3
			60_000_000 * ONE,
			2_428_000,
			1,
			HDX,
			BOB,
			Perquintill::from_float(0.000_000_14_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 4
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 4;
			let charlie_g_farm_id = 2;
			let charlie_y_farm_id = 5;
			let bob_g_farm_id = 3;
			let bob_y_farm_id = 6;
			let omnipool_position_id = 3;
			let deposit_id = 1;
			let asset_in_position = KSM;
			let amount = 20 * ONE;
			let yield_farms = vec![(gc_g_farm_id, gc_y_farm_id)];

			assert_ok!(OmnipoolMining::add_liquidity_and_join_farms(
				RuntimeOrigin::signed(LP1),
				yield_farms.try_into().unwrap(),
				asset_in_position,
				amount,
			));

			//Assert that liquidity is added
			assert_asset_state!(
				asset_in_position,
				AssetReserveState {
					reserve: token_amount + amount,
					hub_reserve: 1313 * ONE,
					shares: token_amount + amount,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			//Assert join farms functionality
			expect_events(vec![crate::Event::SharesDeposited {
				global_farm_id: gc_g_farm_id,
				yield_farm_id: gc_y_farm_id,
				deposit_id,
				asset_id: KSM,
				who: LP1,
				shares_amount: amount,
				position_id: omnipool_position_id,
			}
			.into()]);

			assert_eq!(
				crate::OmniPositionId::<Test>::get(deposit_id).unwrap(),
				omnipool_position_id
			);

			let deposit =
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id).unwrap();
			let mut expected_deposit = DepositData::new(amount, KSM);
			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					gc_g_farm_id,
					gc_y_farm_id,
					1_300_000_000_000_0,
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
fn join_farms_should_work_with_multiple_yield_farm() {
	let token_amount = 2000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(BOB, HDX, 100_000_000 * ONE),
			(ALICE, KSM, 10_000 * ONE),
			(BOB, DOT, 10_000 * ONE),
		])
		.with_registered_asset(KSM)
		.with_registered_asset(DOT)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(KSM, FixedU128::from_float(0.65), LP1, token_amount)
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
		.with_global_farm(
			//id: 3
			60_000_000 * ONE,
			2_428_000,
			1,
			HDX,
			BOB,
			Perquintill::from_float(0.000_000_14_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 4
		.with_yield_farm(CHARLIE, 2, KSM, FixedU128::one(), None) //id: 5
		.with_yield_farm(BOB, 3, KSM, FixedU128::one(), None) //id: 6
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 4;
			let charlie_g_farm_id = 2;
			let charlie_y_farm_id = 5;
			let bob_g_farm_id = 3;
			let bob_y_farm_id = 6;
			let omnipool_position_id = 3;
			let deposit_id = 1;
			let asset_in_position = KSM;
			let amount = 10 * ONE;
			let yield_farms = vec![
				(gc_g_farm_id, gc_y_farm_id),
				(charlie_g_farm_id, charlie_y_farm_id),
				(bob_g_farm_id, bob_y_farm_id),
			];

			assert_ok!(OmnipoolMining::add_liquidity_and_join_farms(
				RuntimeOrigin::signed(LP1),
				yield_farms.try_into().unwrap(),
				KSM,
				amount,
			));

			//Assert that liquidity is added
			assert_asset_state!(
				asset_in_position,
				AssetReserveState {
					reserve: token_amount + amount,
					hub_reserve: 1_306_500_000_000_000,
					shares: token_amount + amount,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			//Assert
			expect_events(vec![
				crate::Event::SharesDeposited {
					global_farm_id: gc_g_farm_id,
					yield_farm_id: gc_y_farm_id,
					deposit_id,
					asset_id: KSM,
					who: LP1,
					shares_amount: amount,
					position_id: omnipool_position_id,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: charlie_g_farm_id,
					yield_farm_id: charlie_y_farm_id,
					deposit_id,
					asset_id: asset_in_position,
					who: LP1,
					shares_amount: amount,
					position_id: omnipool_position_id,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: bob_g_farm_id,
					yield_farm_id: bob_y_farm_id,
					deposit_id,
					asset_id: asset_in_position,
					who: LP1,
					shares_amount: amount,
					position_id: omnipool_position_id,
				}
				.into(),
			]);

			assert_eq!(
				crate::OmniPositionId::<Test>::get(deposit_id).unwrap(),
				omnipool_position_id
			);

			let deposit =
				pallet_liquidity_mining::Deposit::<Test, pallet_liquidity_mining::Instance1>::get(deposit_id).unwrap();
			let mut expected_deposit = DepositData::new(amount, KSM);
			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					gc_g_farm_id,
					gc_y_farm_id,
					6_500_000_000_000,
					FixedU128::zero(),
					1,
					0,
				))
				.unwrap();

			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					charlie_g_farm_id,
					charlie_y_farm_id,
					6_500_000_000_000,
					FixedU128::zero(),
					1,
					0,
				))
				.unwrap();

			expected_deposit
				.add_yield_farm_entry(YieldFarmEntry::new(
					bob_g_farm_id,
					bob_y_farm_id,
					6_500_000_000_000,
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
fn add_liquidity_and_join_farms_should_fail_when_origin_is_none() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(BOB, HDX, 100_000_000 * ONE),
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
		.with_global_farm(
			//id: 2
			60_000_000 * ONE,
			2_428_000,
			1,
			HDX,
			BOB,
			Perquintill::from_float(0.000_000_14_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 4
		.with_yield_farm(CHARLIE, 2, KSM, FixedU128::one(), None) //id: 5
		.with_yield_farm(BOB, 3, KSM, FixedU128::one(), None) //id: 6
		.build()
		.execute_with(|| {
			let gc_g_farm_id = 1;
			let gc_y_farm_id = 4;
			let charlie_g_farm_id = 2;
			let charlie_y_farm_id = 5;
			let bob_g_farm_id = 3;
			let bob_y_farm_id = 6;
			let yield_farms = vec![
				(gc_g_farm_id, gc_y_farm_id),
				(charlie_g_farm_id, charlie_y_farm_id),
				(bob_g_farm_id, bob_y_farm_id),
			];

			assert_noop!(
				OmnipoolMining::add_liquidity_and_join_farms(
					RuntimeOrigin::none(),
					yield_farms.try_into().unwrap(),
					KSM,
					10 * ONE,
				),
				BadOrigin
			);
		});
}

#[test]
fn join_farms_should_fail_when_no_farms_specified() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, KSM, 5000 * ONE),
			(LP2, DOT, 2000 * ONE),
			(GC, HDX, 100_000_000 * ONE),
			(CHARLIE, HDX, 100_000_000 * ONE),
			(BOB, HDX, 100_000_000 * ONE),
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
		.with_global_farm(
			//id: 2
			60_000_000 * ONE,
			2_428_000,
			1,
			HDX,
			BOB,
			Perquintill::from_float(0.000_000_14_f64),
			1_000,
			FixedU128::one(),
		)
		.with_yield_farm(GC, 1, KSM, FixedU128::one(), None) //id: 4
		.build()
		.execute_with(|| {
			let shares_amount = 10 * ONE;
			let farms = vec![];

			assert_noop!(
				OmnipoolMining::add_liquidity_and_join_farms(
					RuntimeOrigin::signed(LP1),
					farms.try_into().unwrap(),
					KSM,
					10 * ONE,
				),
				Error::<Test>::NoFarmEntriesSpecified
			);
		});
}
