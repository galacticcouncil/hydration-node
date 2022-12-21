// This file is part of HydraDX.

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

use frame_support::traits::OnInitialize;

use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_dca::types::ScheduleId;
use pallet_dca::{Order, PoolType, Recurrence, Schedule, Trade};
use pretty_assertions::assert_eq;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
const ALICE: AccountId = 1000;

#[test]
fn dca_btc() {
	ExtBuilder::default()
		.with_token(BTC, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(ALICE, DAI, 10000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = schedule_fake((BTC, DAI), ONE, Recurrence::Fixed(5));
			assert_ok!(DCA::schedule(Origin::signed(ALICE), schedule, Option::None));

			//Act
			Omnipool::on_initialize(501);

			//Assert
			//assert_balance!(ALICE, BTC, ONE);
		});
}

//TODO: add negative case for validating block numbers

fn create_bounded_vec(trades: Vec<Trade>) -> BoundedVec<Trade, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}

type AssetPair = (AssetId, AssetId);

fn schedule_fake(asset_pair: AssetPair, amount: crate::types::Balance, recurrence: Recurrence) -> Schedule {
	let trades = create_bounded_vec(vec![]);

	let schedule = Schedule {
		period: 10,
		order: Order {
			asset_in: 3,
			asset_out: 4,
			amount_in: 1000,
			amount_out: 2000,
			limit: 0,
			route: trades,
		},
		recurrence: recurrence,
	};
	schedule
}
