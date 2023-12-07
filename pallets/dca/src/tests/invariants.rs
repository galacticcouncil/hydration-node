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

use crate::tests::*;

use frame_support::assert_ok;
use frame_support::traits::OnInitialize;
use hydradx_traits::router::PoolType;
use pretty_assertions::assert_eq;
use proptest::prelude::ProptestConfig;
use proptest::prelude::Strategy;
use proptest::proptest;
use std::ops::RangeInclusive;

pub const ONE: Balance = 1_000_000_000_000;

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 10_000_000 * ONE);

fn budget() -> impl Strategy<Value = Balance> {
	BALANCE_RANGE.0..BALANCE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1 * ONE..5000 * ONE
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(200))]
	#[test]
	fn dca_invariant_for_remaining_budget_calculation(
		budget in budget(),
		trade_amount in trade_amount(),
	) {
		ExtBuilder::default()
			.with_endowed_accounts(vec![(ALICE, HDX, budget)])
			.build()
			.execute_with(|| {
				proceed_to_blocknumber(1, 10);

				let total_amount = budget;
				let amount_to_sell = trade_amount;

				let schedule = ScheduleBuilder::new()
					.with_total_amount(total_amount)
					.with_period(1)
					.with_slippage(Some(Permill::from_percent(100)))
					.with_order(Order::Sell {
						asset_in: HDX,
						asset_out: BTC,
						amount_in: amount_to_sell,
						min_amount_out: Balance::MIN,
						route: create_bounded_vec(vec![Trade {
							pool: PoolType::Omnipool,
							asset_in: HDX,
							asset_out: BTC,
						}]),
					})
					.build();

				assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::None));

				//Act and assert
				let schedule_id = 0;

				for i in 1..=10u64 {
					set_to_blocknumber(10 + i);
					let spent =  (amount_to_sell + SELL_DCA_FEE_IN_NATIVE) * i as u128;
					let ramaining_budget = DCA::remaining_amounts(schedule_id).unwrap();
					assert_eq!(total_amount, ramaining_budget + spent);
				}
			});
	}
}

//TODO: remove duplication
pub fn proceed_to_blocknumber(from: u64, to: u64) {
	for block_number in RangeInclusive::new(from, to) {
		System::set_block_number(block_number);
		DCA::on_initialize(block_number);
	}
}

pub fn set_to_blocknumber(to: u64) {
	System::set_block_number(to);
	DCA::on_initialize(to);
}
