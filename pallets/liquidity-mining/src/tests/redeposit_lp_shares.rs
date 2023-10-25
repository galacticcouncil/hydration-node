// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use pretty_assertions::assert_eq;
use test_ext::*;

#[test]
fn redeposit_lp_shares_should_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			//predefined_deposit[0] - GC_FARM, BSX_TKN1_AMM
			set_block_number(50_000);
			assert_eq!(
				LiquidityMining::redeposit_lp_shares(
					EVE_FARM,
					EVE_BSX_TKN1_YIELD_FARM_ID,
					PREDEFINED_DEPOSIT_IDS[0],
					|_, _, _| { Ok(500 * ONE) }
				)
				.unwrap(),
				(50 * ONE, BSX_TKN1_AMM)
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, EVE_FARM, EVE_BSX_TKN1_YIELD_FARM_ID))
					.unwrap()
					.entries_count,
				1
			);

			set_block_number(800_000);
			//Dave's farm incentivize TKN1 - some balance must be set so `valued_shares` will not be `0`.
			assert_eq!(
				LiquidityMining::redeposit_lp_shares(
					DAVE_FARM,
					DAVE_BSX_TKN1_YIELD_FARM_ID,
					PREDEFINED_DEPOSIT_IDS[0],
					|_, _, _| { Ok(5_000 * ONE) }
				)
				.unwrap(),
				(50 * ONE, BSX_TKN1_AMM)
			);

			assert_eq!(
				LiquidityMining::yield_farm((BSX_TKN1_AMM, DAVE_FARM, DAVE_BSX_TKN1_YIELD_FARM_ID))
					.unwrap()
					.entries_count,
				1
			);

			let deposit = LiquidityMining::deposit(PREDEFINED_DEPOSIT_IDS[0]).unwrap();

			assert_eq!(
				deposit.yield_farm_entries,
				vec![
					YieldFarmEntry {
						global_farm_id: GC_FARM,
						valued_shares: 2_500 * ONE,
						yield_farm_id: GC_BSX_TKN1_YIELD_FARM_ID,
						accumulated_claimed_rewards: 0,
						accumulated_rpvs: Zero::zero(),
						entered_at: 18,
						updated_at: 18,
						stopped_at_creation: 0,
						_phantom: PhantomData,
					},
					YieldFarmEntry {
						global_farm_id: EVE_FARM,
						valued_shares: 500 * ONE,
						yield_farm_id: EVE_BSX_TKN1_YIELD_FARM_ID,
						accumulated_claimed_rewards: 0,
						accumulated_rpvs: Zero::zero(),
						entered_at: 50,
						updated_at: 50,
						stopped_at_creation: 0,
						_phantom: PhantomData,
					},
					YieldFarmEntry {
						global_farm_id: DAVE_FARM,
						valued_shares: 5_000 * ONE,
						yield_farm_id: DAVE_BSX_TKN1_YIELD_FARM_ID,
						accumulated_claimed_rewards: 0,
						accumulated_rpvs: Zero::zero(),
						entered_at: 800,
						updated_at: 800,
						stopped_at_creation: 0,
						_phantom: PhantomData,
					},
				]
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
#[cfg_attr(debug_assertions, should_panic(expected = "Defensive failure has been triggered!"))]
fn redeposit_lp_shares_deposit_not_found_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let yield_farm_id = DAVE_BSX_TKN1_YIELD_FARM_ID;

			assert_noop!(
				LiquidityMining::redeposit_lp_shares(DAVE_FARM, yield_farm_id, 999_999_999, |_, _, _| { Ok(10_u128) }),
				Error::<Test, Instance1>::InconsistentState(InconsistentStateError::DepositNotFound)
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn redeposit_lp_shares_to_wrong_yield_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			// Desired yield farm is for different assert pair than original deposit.
			let yield_farm_id = EVE_BSX_TKN2_YIELD_FARM_ID; //original deposit is for bsx/tkn1 assert pair

			assert_noop!(
				LiquidityMining::redeposit_lp_shares(EVE_FARM, yield_farm_id, PREDEFINED_DEPOSIT_IDS[0], |_, _, _| {
					Ok(10_u128)
				}),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			// Same global farm different asset pair.
			let yield_farm_id = GC_BSX_TKN2_YIELD_FARM_ID;
			assert_noop!(
				LiquidityMining::redeposit_lp_shares(GC_FARM, yield_farm_id, PREDEFINED_DEPOSIT_IDS[0], |_, _, _| {
					Ok(10_u128)
				}),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			//Desired yield farm is not in the provided global farm.
			let yield_farm_id = EVE_BSX_TKN1_YIELD_FARM_ID;
			assert_noop!(
				LiquidityMining::redeposit_lp_shares(GC_FARM, yield_farm_id, PREDEFINED_DEPOSIT_IDS[0], |_, _, _| {
					Ok(10_u128)
				}),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn redeposit_lp_shares_to_not_active_yield_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			let yield_farm_id = EVE_BSX_TKN1_YIELD_FARM_ID;

			//Deposit to yield farm to prevent flushing from storage on destroy.
			assert_ok!(LiquidityMining::deposit_lp_shares(
				EVE_FARM,
				yield_farm_id,
				BSX_TKN1_AMM,
				1_000,
				|_, _, _| { Ok(1_000_u128) }
			));

			// Redeposit to stopped farm.
			assert_ok!(LiquidityMining::stop_yield_farm(EVE, EVE_FARM, BSX_TKN1_AMM));

			assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, EVE_FARM, yield_farm_id))
				.unwrap()
				.state
				.is_stopped());

			assert_noop!(
				LiquidityMining::redeposit_lp_shares(EVE_FARM, yield_farm_id, PREDEFINED_DEPOSIT_IDS[0], |_, _, _| {
					Ok(10_u128)
				}),
				Error::<Test, Instance1>::LiquidityMiningCanceled
			);

			// Redeposit to deleted farm
			assert_ok!(LiquidityMining::terminate_yield_farm(
				EVE,
				EVE_FARM,
				yield_farm_id,
				BSX_TKN1_AMM
			));

			assert!(LiquidityMining::yield_farm((BSX_TKN1_AMM, EVE_FARM, yield_farm_id))
				.unwrap()
				.state
				.is_terminated());

			assert_noop!(
				LiquidityMining::redeposit_lp_shares(EVE_FARM, yield_farm_id, PREDEFINED_DEPOSIT_IDS[0], |_, _, _| {
					Ok(10_u128)
				}),
				Error::<Test, Instance1>::LiquidityMiningCanceled
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn redeposit_lp_shares_non_existing_farm_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			const NON_EXISTING_YILED_FARM_ID: FarmId = 999_999_999;

			assert_noop!(
				LiquidityMining::redeposit_lp_shares(
					EVE_FARM,
					NON_EXISTING_YILED_FARM_ID,
					PREDEFINED_DEPOSIT_IDS[0],
					|_, _, _| { Ok(10_u128) }
				),
				Error::<Test, Instance1>::YieldFarmNotFound
			);

			const NON_EXISTING_GLOBAL_FARM_ID: FarmId = 999_999_999;
			assert_noop!(
				LiquidityMining::redeposit_lp_shares(
					NON_EXISTING_GLOBAL_FARM_ID,
					GC_BSX_TKN2_YIELD_FARM_ID,
					PREDEFINED_DEPOSIT_IDS[0],
					|_, _, _| { Ok(10_u128) }
				),
				Error::<Test, Instance1>::YieldFarmNotFound //NOTE: check for yield farm existence is first that's why this error.
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn redeposit_lp_shares_same_deposit_should_not_work() {
	predefined_test_ext_with_deposits().execute_with(|| {
		let _ = with_transaction(|| {
			assert_noop!(
				LiquidityMining::redeposit_lp_shares(
					GC_FARM,
					GC_BSX_TKN1_YIELD_FARM_ID,
					PREDEFINED_DEPOSIT_IDS[0],
					|_, _, _| { Ok(10_u128) }
				),
				Error::<Test, Instance1>::DoubleLock
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn redeposit_lp_shares_should_not_work_when_valued_shares_are_bellow_min_deposit() {
	let _ = predefined_test_ext_with_deposits().execute_with(|| {
		with_transaction(|| {
			assert_noop!(
				LiquidityMining::redeposit_lp_shares(
					EVE_FARM,
					EVE_BSX_TKN1_YIELD_FARM_ID,
					PREDEFINED_DEPOSIT_IDS[0],
					|_, _, _| { Ok(MIN_DEPOSIT - 1) }
				),
				Error::<Test, Instance1>::IncorrectValuedShares
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		})
	});
}
