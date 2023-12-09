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
use mock::{
	asset_pair_to_map_key, set_block_number, with_transaction, AccountId, AssetId, AssetPair, Balance, BlockNumber,
	ExtBuilder, LiquidityMining, RuntimeOrigin as Origin, Test, Tokens, TransactionOutcome, Whitelist, ACA, ACA_FARM,
	ACA_KSM_AMM, ACA_KSM_SHARE_ID, ACCOUNT_WITH_1M, ALICE, AMM_POOLS, BOB, BSX, BSX_ACA_AMM, BSX_ACA_SHARE_ID,
	BSX_ACA_YIELD_FARM_ID, BSX_DOT_AMM, BSX_DOT_SHARE_ID, BSX_DOT_YIELD_FARM_ID, BSX_ETH_AMM, BSX_ETH_SHARE_ID,
	BSX_FARM, BSX_HDX_AMM, BSX_HDX_SHARE_ID, BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_YIELD_FARM_ID, BSX_TKN1_AMM,
	BSX_TKN1_SHARE_ID, BSX_TKN2_AMM, BSX_TKN2_SHARE_ID, CHARLIE, DAVE, DOT, ETH, EVE, GC, GC_FARM, HDX,
	INITIAL_BALANCE, KSM, KSM_DOT_AMM, KSM_DOT_SHARE_ID, KSM_FARM, ONE, TKN1, TKN2, TREASURY, UNKNOWN_ASSET,
};

use frame_support::{assert_noop, assert_ok, traits::Contains};

use sp_arithmetic::{traits::CheckedSub, FixedPointNumber};
use std::cmp::Ordering;

const ALICE_FARM: u32 = BSX_FARM;
const BOB_FARM: u32 = KSM_FARM;
const CHARLIE_FARM: u32 = ACA_FARM;
const DAVE_FARM: u32 = 5;
const EVE_FARM: u32 = 6;

thread_local! {
static PREDEFINED_GLOBAL_FARMS_INS1: [GlobalFarmData<Test, Instance1>; 6] = [
	GlobalFarmData {
		id: ALICE_FARM,
		updated_at: 0,
		reward_currency: BSX,
		yield_per_period: Perquintill::from_percent(20),
		planned_yielding_periods: 300_u64,
		blocks_per_period: 1_000_u64,
		owner: ALICE,
		incentivized_asset: BSX,
		max_reward_per_period: 333_333_333_333_333_333_333,
		accumulated_rpz: Zero::zero(),
		live_yield_farms_count: Zero::zero(),
		total_yield_farms_count: Zero::zero(),
		accumulated_paid_rewards: 0,
		total_shares_z: 0,
		pending_rewards: 0,
		state: FarmState::Active,
		min_deposit: 1_000,
		price_adjustment: One::one(),
	},
	GlobalFarmData {
		id: BOB_FARM,
		updated_at: 0,
		reward_currency: KSM,
		yield_per_period: Perquintill::from_percent(38),
		planned_yielding_periods: 5_000_u64,
		blocks_per_period: 10_000_u64,
		owner: BOB,
		incentivized_asset: BSX,
		max_reward_per_period: 200_000 * ONE,
		accumulated_rpz: Zero::zero(),
		live_yield_farms_count: Zero::zero(),
		total_yield_farms_count: Zero::zero(),
		accumulated_paid_rewards: 0,
		total_shares_z: 0,
		pending_rewards: 0,
		state: FarmState::Active,
		min_deposit: 1_000,
		price_adjustment: One::one(),
	},
	GlobalFarmData {
		id: GC_FARM,
		updated_at: 0,
		reward_currency: BSX,
		yield_per_period: Perquintill::from_percent(50),
		planned_yielding_periods: 500_u64,
		blocks_per_period: 100_u64,
		owner: GC,
		incentivized_asset: BSX,
		max_reward_per_period: 60_000_000 * ONE,
		accumulated_rpz: Zero::zero(),
		live_yield_farms_count: 2,
		total_yield_farms_count: 2,
		accumulated_paid_rewards: 0,
		total_shares_z: 0,
		pending_rewards: 0,
		state: FarmState::Active,
		min_deposit: 1_000,
		price_adjustment: One::one(),
	},
	GlobalFarmData {
		id: CHARLIE_FARM,
		updated_at: 0,
		reward_currency: ACA,
		yield_per_period: Perquintill::from_percent(50),
		planned_yielding_periods: 500_u64,
		blocks_per_period: 100_u64,
		owner: CHARLIE,
		incentivized_asset: KSM,
		max_reward_per_period: 60_000_000 * ONE,
		accumulated_rpz: Zero::zero(),
		live_yield_farms_count: 1,
		total_yield_farms_count: 1,
		accumulated_paid_rewards: 0,
		total_shares_z: 0,
		pending_rewards: 0,
		state: FarmState::Active,
		min_deposit: 1_000,
		price_adjustment: FixedU128::from_float(0.5),
	},
	GlobalFarmData {
		id: DAVE_FARM,
		updated_at: 0,
		reward_currency: ACA,
		yield_per_period: Perquintill::from_percent(20),
		planned_yielding_periods: 300_u64,
		blocks_per_period: 1_000_u64,
		owner: DAVE,
		incentivized_asset: TKN1,
		max_reward_per_period: 333_333_333_333,
		accumulated_rpz: Zero::zero(),
		live_yield_farms_count: Zero::zero(),
		total_yield_farms_count: Zero::zero(),
		accumulated_paid_rewards: 0,
		total_shares_z: 0,
		pending_rewards: 0,
		state: FarmState::Active,
		min_deposit: 1_000,
		price_adjustment: One::one(),
	},
	GlobalFarmData {
		id: EVE_FARM,
		updated_at: 0,
		reward_currency: KSM,
		yield_per_period: Perquintill::from_percent(20),
		planned_yielding_periods: 300_u64,
		blocks_per_period: 1_000_u64,
		owner: EVE,
		incentivized_asset: BSX,
		max_reward_per_period: 333_333_333_333,
		accumulated_rpz: Zero::zero(),
		live_yield_farms_count: Zero::zero(),
		total_yield_farms_count: Zero::zero(),
		accumulated_paid_rewards: 0,
		total_shares_z: 0,
		pending_rewards: 0,
		state: FarmState::Active,
		min_deposit: 1_000,
		price_adjustment: One::one(),
	},
]
}

const GC_BSX_TKN1_YIELD_FARM_ID: u32 = 7;
const GC_BSX_TKN2_YIELD_FARM_ID: u32 = 8;
const CHARLIE_ACA_KSM_YIELD_FARM_ID: u32 = 9;
const DAVE_BSX_TKN1_YIELD_FARM_ID: u32 = 10;
const EVE_BSX_TKN1_YIELD_FARM_ID: u32 = 11;
const EVE_BSX_TKN2_YIELD_FARM_ID: u32 = 12;

thread_local! {
	static PREDEFINED_YIELD_FARMS_INS1: [YieldFarmData<Test, Instance1>; 6] = [
		YieldFarmData::new(
			GC_BSX_TKN1_YIELD_FARM_ID,
			0,
			Some(LoyaltyCurve::default()),
			FixedU128::from(5),
		),
		YieldFarmData::new(
			 GC_BSX_TKN2_YIELD_FARM_ID,
			 0,
			 Some(LoyaltyCurve::default()),
			 FixedU128::from(10),
		),
		YieldFarmData::new(
			 CHARLIE_ACA_KSM_YIELD_FARM_ID,
			 0,
			 Some(LoyaltyCurve::default()),
			 FixedU128::from(10),
		),
		YieldFarmData::new(
			DAVE_BSX_TKN1_YIELD_FARM_ID,
			 0,
			Some(LoyaltyCurve::default()),
			FixedU128::from(10),
			),
		YieldFarmData::new(
			EVE_BSX_TKN1_YIELD_FARM_ID,
			0,
			Some(LoyaltyCurve::default()),
			FixedU128::from(10),
		),
		YieldFarmData::new(
			EVE_BSX_TKN2_YIELD_FARM_ID,
			0,
			Some(LoyaltyCurve::default()),
			FixedU128::from(10),
		),
	]
}

const PREDEFINED_DEPOSIT_IDS: [u128; 7] = [1, 2, 3, 4, 5, 6, 7];

//NOTE: look at approx pallet - https://github.com/brendanzab/approx
fn is_approx_eq_fixedu128(num_1: FixedU128, num_2: FixedU128, delta: FixedU128) -> bool {
	let diff = match num_1.cmp(&num_2) {
		Ordering::Less => num_2.checked_sub(&num_1).unwrap(),
		Ordering::Greater => num_1.checked_sub(&num_2).unwrap(),
		Ordering::Equal => return true,
	};

	if diff.cmp(&delta) == Ordering::Greater {
		println!("diff: {diff:?}; delta: {delta:?}; n1: {num_1:?}; n2: {num_2:?}");

		false
	} else {
		true
	}
}

fn get_predefined_global_farm_ins1(idx: usize) -> GlobalFarmData<Test, Instance1> {
	PREDEFINED_GLOBAL_FARMS_INS1.with(|v| v[idx].clone())
}

fn get_predefined_yield_farm_ins1(idx: usize) -> YieldFarmData<Test, Instance1> {
	PREDEFINED_YIELD_FARMS_INS1.with(|v| v[idx].clone())
}

#[macro_export]
macro_rules! assert_eq_approx {
	( $x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y { $x - $y } else { $y - $x };
		if diff > $z {
			panic!("\n{} not equal\n left: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}

pub mod claim_rewards;
pub mod create_global_farm;
pub mod create_yield_farm;
pub mod deposit_lp_shares;
pub mod full_run;
pub mod invariants;
pub mod mock;
pub mod redeposit_lp_shares;
pub mod resume_yield_farm;
pub mod stop_yield_farm;
pub mod terminate_global_farm;
pub mod terminate_yield_farm;
pub mod test_ext;

pub mod lm_with_oracle;
#[allow(clippy::module_inception)]
pub mod tests;
pub mod update_global_farm;
pub mod update_yield_farm;
pub mod withdraw_lp_shares;
