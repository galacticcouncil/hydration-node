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
use crate::tests::test_ext::new_test_ext;
use pretty_assertions::assert_eq;
use proptest::{
	prelude::*,
	test_runner::{Config, TestRunner},
};
use sp_arithmetic::traits::{CheckedAdd, CheckedMul};
use std::{cell::RefCell, collections::HashMap};

const ONE: Balance = 1_000_000_000_000;
const TOLERANCE: Balance = 1_000;
const REWARD_CURRENCY: AssetId = BSX;

//6s blocks
const BLOCK_PER_YEAR: u64 = 5_256_000;

fn total_shares_z() -> impl Strategy<Value = Balance> {
	0..1_000_000_000 * ONE
}

fn left_to_distribute() -> impl Strategy<Value = Balance> {
	190 * ONE..100_000 * ONE
}

fn reward_per_period() -> impl Strategy<Value = Balance> {
	190 * ONE..1_000_000 * ONE //190BSX -> distribute 3B in 3 years(6s blocks) with 1 block per period
}

fn global_farm_accumulated_rewards() -> impl Strategy<Value = (Balance, Balance)> {
	(0..10_000_000_000 * ONE, 0..10_000_000_000 * ONE)
}

fn accumulated_rpz(total_shares_z: Balance, pending_rewards: Balance) -> impl Strategy<Value = Balance> {
	0..pending_rewards.checked_div(total_shares_z).unwrap().max(1)
}

prop_compose! {
	fn get_global_farm()
		(
			total_shares_z in total_shares_z(),
			(pending_rewards, accumulated_paid_rewards) in global_farm_accumulated_rewards(),
			reward_per_period in reward_per_period(),
		)(
			accumulated_rpz in accumulated_rpz(total_shares_z, pending_rewards),
			pending_rewards in Just(pending_rewards),
			accumulated_paid_rewards in Just(accumulated_paid_rewards),
			reward_per_period in Just(reward_per_period),
			total_shares_z in Just(total_shares_z),
			updated_at in 1_000_000..(BLOCK_PER_YEAR + 1_000_000),
		)
	-> GlobalFarmData<Test, Instance1> {
		GlobalFarmData::<Test, Instance1> {
			id: 1,
			owner: ALICE,
			updated_at,
			total_shares_z,
			accumulated_rpz: FixedU128::from(accumulated_rpz),
			reward_currency: REWARD_CURRENCY,
			pending_rewards,
			accumulated_paid_rewards,
			yield_per_period: Perquintill::from_float(0.002),
			planned_yielding_periods: 1_000,
			blocks_per_period: 1_000,
			incentivized_asset: REWARD_CURRENCY,
			max_reward_per_period: reward_per_period,
			min_deposit: 1,
			live_yield_farms_count: Zero::zero(),
			total_yield_farms_count: Zero::zero(),
			price_adjustment: FixedU128::one(),
			state: FarmState::Active,
		}
	}
}

prop_compose! {
	fn get_farms()
		(
			global_farm in get_global_farm(),
		)(
			yield_farm_accumulated_rpz in 0..global_farm.accumulated_rpz.checked_div_int(1_u128).unwrap().max(1),
			tmp_reward in 100_000 * ONE..5_256_000_000 * ONE, //max: 10K for 1 year, every block
			yield_farm_updated_at in global_farm.updated_at - 1_000..global_farm.updated_at,
			global_farm in Just(global_farm),
		)
	-> (GlobalFarmData<Test, Instance1>, YieldFarmData<Test,Instance1>) {
		//multiplier == 1 => valued_shares== Z
		let rpvs = tmp_reward.checked_div(global_farm.total_shares_z).unwrap();

		let yield_farm = YieldFarmData::<Test, Instance1> {
			id: 2,
			updated_at: yield_farm_updated_at,
			total_shares: Default::default(),
			total_valued_shares: global_farm.total_shares_z,
			accumulated_rpvs: FixedU128::from(rpvs),
			accumulated_rpz: FixedU128::from(yield_farm_accumulated_rpz),
			loyalty_curve: Default::default(),
			multiplier: One::one(),
			state: FarmState::Active,
			entries_count: Default::default(),
			left_to_distribute: Default::default(),
			total_stopped: Default::default(),
			_phantom: Default::default(),
		};

		(global_farm, yield_farm)
	}
}

prop_compose! {
	fn get_global_farm_and_current_period()
		(
			global_farm in get_global_farm(),
		)(
			current_period in global_farm.updated_at..(global_farm.updated_at + BLOCK_PER_YEAR),
			global_farm in Just(global_farm),
		)
	-> (GlobalFarmData<Test, Instance1>, BlockNumber) {
		(global_farm, current_period)
	}
}

prop_compose! {
	fn get_farms_and_current_period_and_yield_farm_rewards()
		(
			(global_farm, yield_farm) in get_farms(),
		)(
			current_period in global_farm.updated_at..(global_farm.updated_at + BLOCK_PER_YEAR),
			yield_farm in Just(yield_farm),
			global_farm in Just(global_farm),
		)
	-> (GlobalFarmData<Test, Instance1>, YieldFarmData<Test, Instance1>, BlockNumber, Balance) {
		//+1 rounding
		let yield_farm_rewards = yield_farm.accumulated_rpvs.checked_mul_int(yield_farm.total_valued_shares).unwrap() + 1;

		(global_farm, yield_farm, current_period, yield_farm_rewards)
	}
}

prop_compose! {
	fn get_farms_and_current_period_and_yield_farm_rewards_and_lef_to_distribute()
		(
			(global_farm, yield_farm, current_period, yield_farm_rewards) in get_farms_and_current_period_and_yield_farm_rewards(),
		)(
			left_to_distribute in yield_farm_rewards + ONE..yield_farm_rewards + 1_000_000 * ONE,
			global_farm in Just(global_farm),
			yield_farm in Just(yield_farm),
			current_period in Just(current_period),
			yield_farm_rewards in Just(yield_farm_rewards),
		)
	-> (GlobalFarmData<Test, Instance1>, YieldFarmData<Test, Instance1>, BlockNumber, Balance, Balance) {

		(global_farm, yield_farm, current_period, yield_farm_rewards, left_to_distribute)
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn sync_global_farm(
		(mut farm, current_period) in get_global_farm_and_current_period(),
		left_to_distribute in left_to_distribute(),
	) {
		new_test_ext().execute_with(|| {
			let _ = with_transaction(|| {
				let farm_account = LiquidityMining::farm_account_id(farm.id).unwrap();
				Tokens::set_balance(Origin::root(), farm_account, REWARD_CURRENCY, left_to_distribute, 0).unwrap();

				//NOTE: _0 - before action, _1 - after action
				let pending_rewards_0 = farm.pending_rewards;
				let accumulated_rpz_0 = farm.accumulated_rpz;
				let reward = LiquidityMining::sync_global_farm(&mut farm, current_period).unwrap();

				let s_0 = accumulated_rpz_0
					.checked_mul(&FixedU128::from((farm.total_shares_z, ONE))).unwrap()
					.checked_add(&FixedU128::from((reward, ONE))).unwrap();
				let s_1 = farm.accumulated_rpz.checked_mul(&FixedU128::from((farm.total_shares_z, ONE))).unwrap();

				assert_eq_approx!(
					s_0,
					s_1,
					FixedU128::from((TOLERANCE, ONE)),
					"acc_rpz[1] x shares = acc_rpz[0] x shares + reward"
				);

				assert!(
					farm.pending_rewards == pending_rewards_0.checked_add(reward).unwrap(),
					"acc_rewards[1] = acc_rewards[0] + reward"
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn claim_rewards_should_be_inclued_in_paid_rewards(
		(mut global_farm, mut yield_farm) in get_farms()
	) {
		new_test_ext().execute_with(|| {
			let _ = with_transaction(|| {
				//NOTE: _0 - before action, _1 - after action
				let sum_accumulated_paid_rewards_0 = global_farm.pending_rewards
					.checked_add(global_farm.accumulated_paid_rewards).unwrap();

				let current_period = yield_farm.updated_at + 1;
				LiquidityMining::sync_yield_farm(&mut yield_farm, &mut global_farm,current_period).unwrap();

				let sum_accumulated_paid_rewards_1 = global_farm.pending_rewards
					.checked_add(global_farm.accumulated_paid_rewards).unwrap();

				assert_eq!(sum_accumulated_paid_rewards_0, sum_accumulated_paid_rewards_1);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn sync_yield_farm(
		(mut global_farm, mut yield_farm, current_period, _, left_to_distribute) in get_farms_and_current_period_and_yield_farm_rewards_and_lef_to_distribute(),
	) {
		new_test_ext().execute_with(|| {
			let _ = with_transaction(|| {
				const GLOBAL_FARM_ID: GlobalFarmId = 1;

				let pot = LiquidityMining::pot_account_id().unwrap();
				let global_farm_account = LiquidityMining::farm_account_id(GLOBAL_FARM_ID).unwrap();
				//rewads for yield farm are paid from global-farm's account to pot
				Tokens::set_balance(Origin::root(), global_farm_account, REWARD_CURRENCY, left_to_distribute, 0).unwrap();

				//NOTE: _0 - before action, _1 - after action
				let pot_balance_0 = Tokens::total_balance(REWARD_CURRENCY, &pot);
				let global_farm_balance_0 = Tokens::total_balance(REWARD_CURRENCY, &global_farm_account);
				let pending_rewards_0 = global_farm.pending_rewards;
				let accumulated_rpvs_0 = yield_farm.accumulated_rpvs;

				LiquidityMining::sync_yield_farm(
					&mut yield_farm, &mut global_farm, current_period).unwrap();

				let global_farm_balance_1 = Tokens::total_balance(REWARD_CURRENCY, &global_farm_account);

				//invariant 1
				//NOTE: yield-farm's rewards are left in the pot until user claims.
				let pot_balance_1 = Tokens::total_balance(REWARD_CURRENCY, &pot);
				let s_0 = global_farm_balance_0 + pot_balance_0;
				let s_1 = global_farm_balance_1 + pot_balance_1;

				assert_eq!(
					s_0,
					s_1,
					"invariant: `global_farm_balance + pot_balance` is always constant"
			   );

				//invariant 2
				let s_0 = FixedU128::from((pending_rewards_0, ONE)) + accumulated_rpvs_0 * FixedU128::from((yield_farm.total_valued_shares, ONE));
				let s_1 = FixedU128::from((global_farm.pending_rewards, ONE)) + yield_farm.accumulated_rpvs * FixedU128::from((yield_farm.total_valued_shares, ONE));

				assert_eq!(
					s_0,
					s_1,
					"invariant: `global_farm.pending_rewards + yield_farm.accumulated_rpvs * yield_farm.total_valued_shares` is always constant"
			   );

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn sync_global_farm_left_to_distribute_invariant(
		(mut global_farm, _, current_period, _, left_to_distribute) in get_farms_and_current_period_and_yield_farm_rewards_and_lef_to_distribute(),
	) {
		new_test_ext().execute_with(|| {
			let _ = with_transaction(|| {
				const GLOBAL_FARM_ID: GlobalFarmId = 1;
				let global_farm_account = LiquidityMining::farm_account_id(GLOBAL_FARM_ID).unwrap();
				let pot = LiquidityMining::pot_account_id().unwrap();
				Tokens::set_balance(Origin::root(), global_farm_account, REWARD_CURRENCY, left_to_distribute, 0).unwrap();

				let left_to_distribute_0 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);
				let pot_balance_0 = Tokens::free_balance(REWARD_CURRENCY, &pot);

				let reward =
					LiquidityMining::sync_global_farm(&mut global_farm, current_period).unwrap();

				let s_0 = (left_to_distribute_0 - reward).max(0);
				let s_1 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account);

				assert_eq!(
					s_0,
					s_1,
					"left_to_distribute[1] = max(0, left_to_distribute[0] - reward)"
				);

				let s_0 = left_to_distribute_0 + pot_balance_0;
				let s_1 = Tokens::free_balance(REWARD_CURRENCY, &global_farm_account) + Tokens::free_balance(REWARD_CURRENCY, &pot);

				assert_eq!(
					s_0,
					s_1,
					"global_farm_account[0] + pot[0] = global_farm_account[1] + pot[1]"
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

#[derive(Clone)]
pub struct InvTestGFarm {
	id: GlobalFarmId,
	total_rewards: Balance,
	reward_currency: AssetId,
	yield_farms: Vec<(YieldFarmId, AccountId, Vec<AssetId>)>,
}

//NOTE: Variables in this block are valid only if creation order in `invariants_externalities` won't change.
thread_local! {
	pub static G_FARMS: RefCell<Vec<InvTestGFarm>> = RefCell::new(vec![
		InvTestGFarm {
			id: 1,
			total_rewards: 115_000_000 * ONE,
			reward_currency: BSX,
			yield_farms: vec![
				(4, BSX_TKN1_AMM, vec![BSX, TKN1]),
				(5, BSX_TKN2_AMM, vec![BSX, TKN2])
			]
		},
		InvTestGFarm {
			id: 2,
			total_rewards: 1_000_000_000 * ONE,
			reward_currency: BSX,
			yield_farms: vec![
				(6, BSX_TKN1_AMM, vec![BSX, TKN1]),
				(7, BSX_TKN2_AMM, vec![BSX, TKN2])
			]
		},
		InvTestGFarm {
			id: 3,
			//NOTE: `total_rewards` is intentionally small. This represents asses with fewer dec. places.
			total_rewards: 100_000_000_000_000,
			reward_currency: TKN1,
			yield_farms: vec![
				(8, BSX_TKN1_AMM, vec![BSX, TKN1]),
				(9, BSX_TKN2_AMM, vec![BSX, TKN2])
			]
		}
	])
}

pub fn invariants_externalities() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();

	ext.execute_with(|| {
		let _ = with_transaction(|| {
			//global-farms:
			assert_ok!(LiquidityMining::create_global_farm(
				G_FARMS.with(|v| v.borrow()[0].total_rewards),
				302_400,
				1,
				BSX,
				G_FARMS.with(|v| v.borrow()[0].reward_currency),
				GC,
				Perquintill::from_rational(152_207_001_522, FixedU128::DIV), //apr 80%
				1_000,
				FixedU128::one()
			));

			assert_ok!(LiquidityMining::create_global_farm(
				G_FARMS.with(|v| v.borrow()[1].total_rewards),
				500_000,
				1,
				BSX,
				G_FARMS.with(|v| v.borrow()[1].reward_currency),
				GC,
				Perquintill::from_rational(951_293_759_512, FixedU128::DIV), //apr 50%
				1_000,
				FixedU128::one()
			));

			assert_ok!(LiquidityMining::create_global_farm(
				G_FARMS.with(|v| v.borrow()[2].total_rewards),
				50_000,
				1,
				BSX,
				G_FARMS.with(|v| v.borrow()[2].reward_currency),
				GC,
				Perquintill::from_rational(9_512_937_595, FixedU128::DIV), //apr 5%
				1_000,
				FixedU128::from_float(0.5_f64)
			));

			//yield-farms
			let g_farm = G_FARMS.with(|v| v.borrow()[0].clone());
			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				g_farm.id,
				FixedU128::one(),
				Some(LoyaltyCurve::default()),
				g_farm.yield_farms[0].1,
				g_farm.yield_farms[0].2.clone(),
			));

			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				g_farm.id,
				FixedU128::one(),
				None,
				g_farm.yield_farms[1].1,
				g_farm.yield_farms[1].2.clone(),
			));

			let g_farm = G_FARMS.with(|v| v.borrow()[1].clone());
			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				g_farm.id,
				FixedU128::one(),
				Some(LoyaltyCurve::default()),
				g_farm.yield_farms[0].1,
				g_farm.yield_farms[0].2.clone(),
			));

			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				g_farm.id,
				FixedU128::from_float(1.5_f64),
				None,
				g_farm.yield_farms[1].1,
				g_farm.yield_farms[1].2.clone(),
			));

			let g_farm = G_FARMS.with(|v| v.borrow()[2].clone());
			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				g_farm.id,
				FixedU128::one(),
				Some(LoyaltyCurve::default()),
				g_farm.yield_farms[0].1,
				g_farm.yield_farms[0].2.clone(),
			));

			assert_ok!(LiquidityMining::create_yield_farm(
				GC,
				g_farm.id,
				FixedU128::from_float(0.5_f64),
				Some(LoyaltyCurve::default()),
				g_farm.yield_farms[1].1,
				g_farm.yield_farms[1].2.clone(),
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	ext
}

#[derive(Debug, Clone)]
struct Deposit {
	global_farm_id: GlobalFarmId,
	yield_farm_id: YieldFarmId,
	amm_pool_id: AccountId,
	shares: Balance,
	valued_shares: Balance,
}

prop_compose! {
	fn arb_deposit()(
		shares in 1_000 * ONE..1_000_000 * ONE,
		valued_shares in 1..10_000_000 * ONE,
		g_idx in 0..3_usize,
		y_idx in 0..2_usize,
	) -> Deposit {
		let g_farm = G_FARMS.with(|v| v.borrow()[g_idx].clone());
		let y_farm = &g_farm.yield_farms[y_idx];

		Deposit {global_farm_id: g_farm.id,yield_farm_id: y_farm.0, amm_pool_id: y_farm.1, shares, valued_shares}
	}
}

#[test]
//https://www.notion.so/Liquidity-mining-spec-b30ccfe470a74173b82c3702b1e8fca1#87868f45e4d04ecb92374c5f795a493d
//
//For each g\ \in\ GlobalFarms
//total\_rewards_g = Balance_g + accumulated\_paid\_rewards_g +\ pending\_rewards_g
//where
//total\_rewards_g = max\_reward\_per\_period\ *\  planned\_yielding\_periods
fn invariant_1() {
	//Number of sucessfull test cases that must execute for the test as a whole to pass.
	let successfull_cases = 1_000;
	//Number of blocks added to current block number in each test case run. This number should be
	//reasonable smaller than total of runned test to make sure lot of claims is executed and
	//multiple claims for same deposit to happen.
	let blocks_offset_range = 1..10_u64;
	//Index of deposit in `deposit` vec. This idx is used in each test case run and execute claim
	//if deposit exits.

	let deposit_idx_range = 0..500_usize;

	invariants_externalities().execute_with(|| {
		let mut runner = TestRunner::new(Config {
			cases: successfull_cases,
			source_file: Some("liquidity-mining/src/tests/invariants.rs"),
			test_name: Some("invariant_1"),
			..Config::default()
		});
		let deposits: RefCell<Vec<Deposit>> = RefCell::new(Vec::new());

		runner
			.run(
				&(arb_deposit(), blocks_offset_range, deposit_idx_range),
				|(d, blocks_offset, deposit_idx)| {
					deposits.borrow_mut().push(d.clone());

					//Act
					let _ = with_transaction(|| {
						assert_ok!(LiquidityMining::deposit_lp_shares(
							d.global_farm_id,
							d.yield_farm_id,
							d.amm_pool_id,
							d.shares,
							|_, _, _| -> Result<Balance, DispatchError> { Ok(d.valued_shares) }
						));

						set_block_number(mock::System::block_number() + blocks_offset);

						//claim rewards only if deposit exists
						if deposit_idx < deposits.borrow().len() {
							let d = &deposits.borrow()[deposit_idx];
							let deposit_id = deposit_idx as u128 + 1;

							assert_ok!(LiquidityMining::claim_rewards(ALICE, deposit_id, d.yield_farm_id, true));
						}

						TransactionOutcome::Commit(DispatchResult::Ok(()))
					});

					//Assert:
					G_FARMS.with(|v| {
						v.borrow().clone().into_iter().for_each(|gf| {
							let g_farm_balance = Tokens::free_balance(
								gf.reward_currency,
								&LiquidityMining::farm_account_id(gf.id).unwrap(),
							);
							let g_farm_1 = LiquidityMining::global_farm(gf.id).unwrap();

							//1.1 assert
							let s_1 = g_farm_balance + g_farm_1.accumulated_paid_rewards + g_farm_1.pending_rewards;
							//NOTE: This should be precise.
							assert_eq!(gf.total_rewards, s_1);

							//1.2 assert
							let s_1: u128 = g_farm_1.max_reward_per_period * g_farm_1.planned_yielding_periods as u128;
							//NOTE: Approax becasue of div in max_reward_per_period calculation.
							assert_eq_approx!(
								gf.total_rewards,
								s_1,
								100_000,
								"total_rewards = max_reward_per_period * planned_yielding_periods"
							);
						})
					});

					Ok(())
				},
			)
			.unwrap();
	});
}

#[test]
//https://www.notion.so/Liquidity-mining-spec-b30ccfe470a74173b82c3702b1e8fca1#422dea2e23744859baeb704dbdb3caca
//
//\displaystyle \sum_{g\ \in\ globalFarm} total\_rewards_g = \sum_{g\ \in\ globalFarm} Balance_g + Balance_{pot} + \sum_{d\ \epsilon\ Deposits, y\ \in\ YieldFarm}claimed\_rewards^y_d
fn invariant_2() {
	//Number of sucessfull test cases that must execute for the test as a whole to pass.
	let successfull_cases = 1_000;
	//Number of blocks added to current block number in each test case run. This number should be
	//reasonable smaller than total of runned test to make sure lot of claims is executed and
	//multiple claims for same deposit to happen.
	let blocks_offset_range = 1..10_u64;
	//Index of deposit in `deposit` vec. This idx is used in each test case run and execute claim
	//if deposit exits.

	let deposit_idx_range = 0..500_usize;

	invariants_externalities().execute_with(|| {
		let mut runner = TestRunner::new(Config {
			cases: successfull_cases,
			source_file: Some("liquidity-mining/src/tests/invariants.rs"),
			test_name: Some("invariant_2"),
			..Config::default()
		});
		let deposits: RefCell<Vec<Deposit>> = RefCell::new(Vec::new());
		let pot = LiquidityMining::pot_account_id().unwrap();

		runner
			.run(
				&(arb_deposit(), blocks_offset_range, deposit_idx_range),
				|(d, blocks_offset, deposit_idx)| {
					deposits.borrow_mut().push(d.clone());

					//Act
					let _ = with_transaction(|| {
						assert_ok!(LiquidityMining::deposit_lp_shares(
							d.global_farm_id,
							d.yield_farm_id,
							d.amm_pool_id,
							d.shares,
							|_, _, _| -> Result<Balance, DispatchError> { Ok(d.valued_shares) }
						));

						set_block_number(mock::System::block_number() + blocks_offset);

						//claim rewards only if deposit exists
						if deposit_idx < deposits.borrow().len() {
							let d = &deposits.borrow()[deposit_idx];
							let deposit_id = deposit_idx as u128 + 1;

							assert_ok!(LiquidityMining::claim_rewards(ALICE, deposit_id, d.yield_farm_id, true));
						}

						TransactionOutcome::Commit(DispatchResult::Ok(()))
					});

					//Calculate necessary values and assert
					let (total_rewards_sum, farm_balances_sum, pot_balance_sum) = G_FARMS.with(|v| {
						let mut total_rewards_sum = 0_u128;
						let mut farm_balances_sum = 0_u128;
						let mut pot_balance_sum = 0_u128;
						let mut already_summed_balances: Vec<AssetId> = Vec::new();

						v.borrow().clone().into_iter().for_each(|gf| {
							farm_balances_sum += Tokens::free_balance(
								gf.reward_currency,
								&LiquidityMining::farm_account_id(gf.id).unwrap(),
							);

							total_rewards_sum += gf.total_rewards;

							if !already_summed_balances.contains(&gf.reward_currency) {
								pot_balance_sum += Tokens::total_balance(gf.reward_currency, &pot);
								already_summed_balances.push(gf.reward_currency);
							}
						});
						(total_rewards_sum, farm_balances_sum, pot_balance_sum)
					});

					let last_deposit_id = LiquidityMining::deposit_id();
					let mut claimed_by_users_sum = 0_u128;
					for i in 1..=last_deposit_id {
						let d = LiquidityMining::deposit(i).unwrap();

						let claimed_amount = d.yield_farm_entries[0].accumulated_claimed_rewards;
						claimed_by_users_sum += claimed_amount;
					}

					//WARN: There is no room for rounding errors in this invariant. Any discrepancy
					//in this assert means we are loosing tokens somewhere.
					assert_eq!(
						total_rewards_sum,
						farm_balances_sum + pot_balance_sum + claimed_by_users_sum,
					);

					Ok(())
				},
			)
			.unwrap();
	});
}

#[test]
//https://www.notion.so/Liquidity-mining-spec-b30ccfe470a74173b82c3702b1e8fca1#6dd41bee00384293980b7c20d8dc6ec6
//
//For each g\ \in\ GlobalFarms
//\displaystyle accumulated\_paid\_rewards_g = \sum_{y\ \in\ YieldFarms} left\_to\_distribute_y + \sum_{d\ \in\ Deposits,y\ \in\ YielFarm} claimed\_rewards^y_d
fn invariant_3() {
	//Number of sucessfull test cases that must execute for the test as a whole to pass.
	let successfull_cases = 1_000;
	//Number of blocks added to current block number in each test case run. This number should be
	//reasonable smaller than total of runned test to make sure lot of claims is executed and
	//multiple claims for same deposit to happen.
	let blocks_offset_range = 1..10_u64;
	//Index of deposit in `deposit` vec. This idx is used in each test case run and execute claim
	//if deposit exits.

	let deposit_idx_range = 0..500_usize;

	invariants_externalities().execute_with(|| {
		let mut runner = TestRunner::new(Config {
			cases: successfull_cases,
			source_file: Some("liquidity-mining/src/tests/invariants.rs"),
			test_name: Some("invariant_3"),
			..Config::default()
		});
		let deposits: RefCell<Vec<Deposit>> = RefCell::new(Vec::new());

		runner
			.run(
				&(arb_deposit(), blocks_offset_range, deposit_idx_range),
				|(d, blocks_offset, deposit_idx)| {
					deposits.borrow_mut().push(d.clone());

					//Act
					let _ = with_transaction(|| {
						assert_ok!(LiquidityMining::deposit_lp_shares(
							d.global_farm_id,
							d.yield_farm_id,
							d.amm_pool_id,
							d.shares,
							|_, _, _| -> Result<Balance, DispatchError> { Ok(d.valued_shares) }
						));

						set_block_number(mock::System::block_number() + blocks_offset);

						//claim rewards only if deposit exists
						if deposit_idx < deposits.borrow().len() {
							let d = &deposits.borrow()[deposit_idx];
							let deposit_id = deposit_idx as u128 + 1;

							assert_ok!(LiquidityMining::claim_rewards(ALICE, deposit_id, d.yield_farm_id, true));
						}

						TransactionOutcome::Commit(DispatchResult::Ok(()))
					});

					//Calculate necessary values and assert
					let last_deposit_id = LiquidityMining::deposit_id();
					let mut claimed_by_user_per_yield_farm: HashMap<(GlobalFarmId, YieldFarmId), u128> = HashMap::new();
					for i in 1..=last_deposit_id {
						let d = LiquidityMining::deposit(i).unwrap();

						let claimed_amount = d.yield_farm_entries[0].accumulated_claimed_rewards;
						let global_farm_id = d.yield_farm_entries[0].global_farm_id;
						let yield_farm_id = d.yield_farm_entries[0].yield_farm_id;
						*claimed_by_user_per_yield_farm
							.entry((global_farm_id, yield_farm_id))
							.or_insert(0) += claimed_amount;
					}

					G_FARMS.with(|v| {
						v.borrow().clone().into_iter().for_each(|gf| {
							let g_farm = LiquidityMining::global_farm(gf.id).unwrap();
							let mut y_farms_let_to_distribute_sum = 0_u128;

							let mut claimed_by_yield_farms_in_global_farm = 0_u128;
							gf.yield_farms.iter().for_each(|yf| {
								y_farms_let_to_distribute_sum += LiquidityMining::yield_farm((yf.1, gf.id, yf.0))
									.unwrap()
									.left_to_distribute;

								//NOTE this run for each iteration of test so record in HashMap may
								//not exists if deposit doesn't exists yet.
								claimed_by_yield_farms_in_global_farm +=
									claimed_by_user_per_yield_farm.get(&(gf.id, yf.0)).unwrap_or(&0_u128);
							});

							assert_eq!(
								g_farm.accumulated_paid_rewards,
								y_farms_let_to_distribute_sum + claimed_by_yield_farms_in_global_farm
							);
						})
					});

					Ok(())
				},
			)
			.unwrap();
	});
}

#[test]
//https://www.notion.so/Liquidity-mining-spec-b30ccfe470a74173b82c3702b1e8fca1#b8cb39a055054198a083c4946110b70d
//
//\displaystyle accumulated\_rewards_g + paid\_accumulated\_rewards_g = \sum_{i\ \in\
//\{\frac{b}{block\_per\_period_g}|b\ \in\ BlockNumber\}}(accumulated\_rpz^i_g -
//accumulated\_rpz^{i-1}_g) * Z^i_g
fn invariant_4() {
	//Number of sucessfull test cases that must execute for the test as a whole to pass.
	let successfull_cases = 1_000;
	//Number of blocks added to current block number in each test case run. This number should be
	//reasonable smaller than total of runned test to make sure lot of claims is executed and
	//multiple claims for same deposit to happen.
	let blocks_offset_range = 1..10_u64;
	//Index of deposit in `deposit` vec. This idx is used in each test case run and execute claim
	//if deposit exits.

	let deposit_idx_range = 0..500_usize;

	invariants_externalities().execute_with(|| {
        let mut runner = TestRunner::new(Config {
            cases: successfull_cases,
            source_file: Some("liquidity-mining/src/tests/invariants.rs"),
            test_name: Some("invariant_4"),
            ..Config::default()
        });
        let deposits: RefCell<Vec<Deposit>> = RefCell::new(Vec::new());
        let paid_until_now: RefCell<HashMap<GlobalFarmId, Balance>> = RefCell::new(HashMap::new());

        runner
            .run(
                &(arb_deposit(), blocks_offset_range, deposit_idx_range),
                |(d, blocks_offset, deposit_idx)| {
                    deposits.borrow_mut().push(d.clone());
                    let _ = with_transaction(|| {
                        let g_farm_0 = LiquidityMining::global_farm(d.global_farm_id).unwrap();

                        assert_ok!(LiquidityMining::deposit_lp_shares(
                            d.global_farm_id,
                            d.yield_farm_id,
                            d.amm_pool_id,
                            d.shares,
                            |_,_,_| -> Result<Balance, DispatchError> {
                                Ok(d.valued_shares)
                            }
                        ));

                        let g_farm_1 = LiquidityMining::global_farm(g_farm_0.id).unwrap();
                        let s_0 = g_farm_1.accumulated_paid_rewards + g_farm_1.pending_rewards;
                        *paid_until_now.borrow_mut().entry(g_farm_1.id).or_insert(0_u128) += (g_farm_1.accumulated_rpz
                            - g_farm_0.accumulated_rpz)
                            .checked_mul_int(g_farm_0.total_shares_z)
                            .unwrap();

                        //NOTE: global-farm is updated before deposit so in this case we need to
                        //use Z{now-1} instead of Z{now} which includes deposited shares.
                        assert_eq_approx!(s_0, *paid_until_now.borrow().get(&d.global_farm_id).unwrap(), 1_000_000, "accumulated_paid_rewards + pending_rewards = sum(rpz{now} - rpz{now-1} * Z{now-1}) for all periods");

                        set_block_number(mock::System::block_number() + blocks_offset);

                        //claim rewards only if deposit exists
                        if deposit_idx < deposits.borrow().len() {
                            let d = &deposits.borrow()[deposit_idx];
                            let deposit_id = deposit_idx as u128 + 1;

                            let g_farm_0 = LiquidityMining::global_farm(d.global_farm_id).unwrap();

                            assert_ok!(LiquidityMining::claim_rewards(ALICE, deposit_id, d.yield_farm_id, true));

                            let g_farm_1 = LiquidityMining::global_farm(g_farm_0.id).unwrap();
                            let s_0 = g_farm_1.accumulated_paid_rewards + g_farm_1.pending_rewards;
                            *paid_until_now.borrow_mut().entry(g_farm_1.id).or_insert(0_u128) += (g_farm_1.accumulated_rpz
                                - g_farm_0.accumulated_rpz)
                                .checked_mul_int(g_farm_1.total_shares_z)
                                .unwrap();

                            //NOTE: global-farm is updated before claim so RPZ includes all Z so Z{now}
                            //must be used in this case.
                            assert_eq_approx!(s_0, *paid_until_now.borrow().get(&d.global_farm_id).unwrap(), 1_000_000, "accumulated_paid_rewards + pending_rewards = sum(rpz{now} - rpz{now-1} * Z{now}) for all periods");
                        }

                        TransactionOutcome::Commit(DispatchResult::Ok(()))
                    });

                    Ok(())
                },
            )
            .unwrap();
    });
}
