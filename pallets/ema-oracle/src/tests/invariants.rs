// This file is part of pallet-ema-oracle.

// Copyright (C) 2022-2023  Intergalactic, Limited (GIB).
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

use super::mock::{DOT, HDX};
use super::*;

use pretty_assertions::assert_eq;
use proptest::prelude::*;

use frame_support::assert_ok;

// Strategies
fn valid_asset_ids() -> impl Strategy<Value = (AssetId, AssetId)> {
	(any::<AssetId>(), any::<AssetId>()).prop_filter("asset ids should not be equal", |(a, b)| a != b)
}

fn non_zero_amount() -> impl Strategy<Value = Balance> {
	1..Balance::MAX
}

fn any_volume() -> impl Strategy<Value = Volume<Balance>> {
	(any::<Balance>(), any::<Balance>(), any::<Balance>(), any::<Balance>()).prop_map(|(a_in, b_out, a_out, b_in)| {
		Volume {
			a_in,
			b_out,
			a_out,
			b_in,
		}
	})
}

fn any_price() -> impl Strategy<Value = Price> {
	(any::<Balance>(), non_zero_amount()).prop_map(|(a, b)| Price::new(a, b))
}

fn any_liquidity() -> impl Strategy<Value = Liquidity<Balance>> {
	(any::<Balance>(), any::<Balance>()).prop_map(|l| l.into())
}

fn oracle_entry_with_updated_at(updated_at: BlockNumber) -> impl Strategy<Value = OracleEntry<BlockNumber>> {
	(any_price(), any_volume(), any_liquidity(), Just(updated_at)).prop_map(|(price, volume, liquidity, updated_at)| {
		OracleEntry {
			price,
			volume,
			liquidity,
			updated_at,
		}
	})
}

fn oracle_entry_within_updated_at_range(
	(updated_at_min, updated_at_max): (BlockNumber, BlockNumber),
) -> impl Strategy<Value = OracleEntry<BlockNumber>> {
	(
		any_price(),
		any_volume(),
		any_liquidity(),
		updated_at_min..updated_at_max,
	)
		.prop_map(|(price, volume, liquidity, updated_at)| OracleEntry {
			price,
			volume,
			liquidity,
			updated_at,
		})
}

// Tests
proptest! {
	#[test]
	fn price_normalization_should_be_independent_of_asset_order(
		(asset_a, asset_b) in valid_asset_ids(),
		(amount_a, amount_b) in (non_zero_amount(), non_zero_amount())
	) {
		let a_then_b = determine_normalized_price(asset_a, asset_b, Price::new(amount_a, amount_b));
		let b_then_a = determine_normalized_price(asset_b, asset_a, Price::new(amount_b, amount_a));
		prop_assert_eq!(a_then_b, b_then_a);
	}
}

proptest! {
	#[test]
	fn on_liquidity_changed_should_not_change_volume(
		(asset_a, asset_b) in valid_asset_ids(),
		(amount_a, amount_b) in (non_zero_amount(), non_zero_amount()),
		(liquidity_a, liquidity_b) in (non_zero_amount(), non_zero_amount()),
		(second_amount_a, second_amount_b) in (non_zero_amount(), non_zero_amount()),
		(second_liquidity_a, second_liquidity_b) in (non_zero_amount(), non_zero_amount()),
	) {
		new_test_ext().execute_with(|| {
			let updated_at = 5;
			System::set_block_number(updated_at);
			assert_ok!(OnActivityHandler::<Test>::on_trade(SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_a, liquidity_b, Price::new(liquidity_a, liquidity_b)));
			let volume_before = get_accumulator_entry(SOURCE, (asset_a, asset_b)).unwrap().volume;
			assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(SOURCE, asset_a, asset_b, second_amount_a, second_amount_b, second_liquidity_a, second_liquidity_b, Price::new(second_liquidity_a, second_liquidity_b)));
			let volume_after = get_accumulator_entry(SOURCE, (asset_a, asset_b)).unwrap().volume;
			assert_eq!(volume_before, volume_after);
		});
	}
}

proptest! {
	#[test]
	fn update_outdated_to_current_equals_calculate_current_from_outdated(
		start_oracle in oracle_entry_within_updated_at_range((0, 1_000)),
		incoming_value in oracle_entry_within_updated_at_range((1_001, 100_000)),
	) {
		let next_oracle = start_oracle.calculate_current_from_outdated(TenMinutes, &incoming_value);

		let mut start_oracle = start_oracle;
		start_oracle.update_outdated_to_current(TenMinutes, &incoming_value);
		prop_assert_eq!(next_oracle, Some(start_oracle));
	}
}

proptest! {
	#[test]
	fn update_to_new_by_integrating_incoming_equals_calculate_new_by_integrating_incoming(
		start_oracle in oracle_entry_with_updated_at(10_000),
		incoming_value in oracle_entry_with_updated_at(10_001),
	) {
		let next_oracle = start_oracle.calculate_new_by_integrating_incoming(TenMinutes, &incoming_value);

		let mut start_oracle = start_oracle;
		start_oracle.update_to_new_by_integrating_incoming(TenMinutes, &incoming_value);
		prop_assert_eq!(next_oracle, Some(start_oracle));
	}
}

use hydra_dx_math::ema::{iterated_balance_ema, iterated_price_ema, iterated_volume_ema};

proptest! {
	#[test]
	fn get_entry_equals_iterated_ema(
		(amount_hdx, amount_dot) in (non_zero_amount(), non_zero_amount()),
		(liquidity_hdx, liquidity_dot) in (non_zero_amount(), non_zero_amount()),
	) {
		new_test_ext().execute_with(|| -> Result<(), TestCaseError> {
			System::set_block_number(1);
			assert_ok!(OnActivityHandler::<Test>::on_trade(SOURCE, HDX, DOT, amount_hdx, amount_dot, liquidity_hdx, liquidity_dot, Price::new(liquidity_hdx, liquidity_dot)));
			EmaOracle::on_finalize(1);
			let oracle_age: u32 = 98;
			System::set_block_number(u64::from(oracle_age) + 2);
			let smoothing = into_smoothing(LastBlock);
			let price = Price::new(liquidity_hdx, liquidity_dot);
			let volume = (amount_hdx, amount_dot, 0, 0);
			let expected = AggregatedEntry {
				price: iterated_price_ema(oracle_age, price, price, smoothing),
				volume: iterated_volume_ema(oracle_age, volume, smoothing).into(),
				liquidity: (iterated_balance_ema(oracle_age, liquidity_hdx, liquidity_hdx, smoothing),
				iterated_balance_ema(oracle_age, liquidity_dot, liquidity_dot, smoothing)).into(),
				oracle_age: 98,
			};
			prop_assert_eq!(EmaOracle::get_entry(HDX, DOT, LastBlock, SOURCE), Ok(expected));

			let smoothing = into_smoothing(TenMinutes);
			let expected_ten_min = AggregatedEntry {
				price: iterated_price_ema(oracle_age, price, price, smoothing),
				volume: iterated_volume_ema(oracle_age, volume, smoothing).into(),
				liquidity: (iterated_balance_ema(oracle_age, liquidity_hdx, liquidity_hdx, smoothing),
				iterated_balance_ema(oracle_age, liquidity_dot, liquidity_dot, smoothing)).into(),
				oracle_age: 98,
			};
			prop_assert_eq!(EmaOracle::get_entry(HDX, DOT, TenMinutes, SOURCE), Ok(expected_ten_min));
			Ok(())
		})?;
	}
}
