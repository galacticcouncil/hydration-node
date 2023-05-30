// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

use crate::tests::mock::*;
use proptest::prelude::*;
use sp_runtime::{FixedPointNumber, FixedU128};
use std::cmp::min;
use test_utils::assert_eq_approx;

const MIN_ORDER_SIZE: Balance = 5;
const DEVIATION_TOLERANCE: f64 = 0.000_1;

fn decimals() -> impl Strategy<Value = u32> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}

fn asset_amount(max: Balance, precision: u32) -> impl Strategy<Value = Balance> {
	let min_order = 5 * 10u128.pow(precision) + 10u128.pow(precision);
	let max_amount = max * 10u128.pow(precision);
	min_order..max_amount
}

fn amount_fill(
	amount_in: Balance,
	amount_out: Balance,
	precision_in: u32,
	precision_out: u32,
) -> impl Strategy<Value = Balance> {
	let price = FixedU128::from_rational(amount_in, amount_out);
	let m = price
		.checked_mul_int(MIN_ORDER_SIZE * 10u128.pow(precision_out))
		.unwrap();
	let max_remaining_amount_out = amount_in - m;
	let max_remaining_amount_in = amount_in - MIN_ORDER_SIZE * 10u128.pow(precision_in);

	0..min(max_remaining_amount_out, max_remaining_amount_in)
}

prop_compose! {
	fn get_asset_amounts_with_precision(precision_in: u32, precision_out: u32)
	(
		amount_in in asset_amount(100, precision_in),
		amount_out in asset_amount(100, precision_out),
	)
	(
		amount_in in Just(amount_in),
		amount_out in Just(amount_out),
		amount_fill in amount_fill(amount_in, amount_out, precision_in, precision_out),
	)
	-> (Balance, Balance, Balance) {
		(amount_in, amount_out, amount_fill)
	}
}

prop_compose! {
	fn get_asset_amounts()
	(
		precision_in in decimals(),
		precision_out in decimals(),
	)
	(
		precision_in in Just(precision_in),
		precision_out in Just(precision_out),
		(amount_in, amount_out, amount_fill) in get_asset_amounts_with_precision(precision_in, precision_out),
	)
	-> (Balance, Balance, Balance, u32, u32) {
		(amount_in, amount_out, amount_fill, precision_in, precision_out)
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1_000))]
	#[test]
	fn otc_price_invariant(
		(initial_amount_in, initial_amount_out, amount_fill, precision_in, precision_out) in get_asset_amounts()
	) {
		ExtBuilder::default()
		.with_existential_deposit(DAI, precision_in)
		.with_existential_deposit(HDX, precision_out)
		.build()
		.execute_with(|| {
			OTC::place_order(
				RuntimeOrigin::signed(ALICE),
				DAI,
				HDX,
				initial_amount_in,
				initial_amount_out,
				true
			).unwrap();

			let initial_price = FixedU128::from_rational(initial_amount_out, initial_amount_in);

			OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount_fill).unwrap();

			let order = OTC::orders(0).unwrap();
			let new_price = FixedU128::from_rational(order.amount_out, order.amount_in);

			assert_eq_approx!(
				initial_price,
				new_price,
				FixedU128::from_float(DEVIATION_TOLERANCE),
				"initial_amount_in / initial_amount_out = amount_in / amount_out"
			);
		});
	}
}
