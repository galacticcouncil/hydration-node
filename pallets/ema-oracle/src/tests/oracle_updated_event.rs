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

use super::*;
use mock::{expect_events, ACA, ORACLE_ENTRY_1, ORACLE_ENTRY_2};

use frame_support::assert_ok;
use hydradx_traits::OraclePeriod;
use pretty_assertions::assert_eq;
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn oracle_updated_event_should_be_emitted_on_trade() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		EmaOracle::on_initialize(5);

		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));

		EmaOracle::on_finalize(5);

		let expected_price = ORACLE_ENTRY_1.price;

		expect_events(vec![Event::OracleUpdated {
			source: SOURCE,
			assets: ordered_pair(HDX, DOT),
			updates: BTreeMap::from([
				(OraclePeriod::TenMinutes, expected_price),
				(OraclePeriod::Day, expected_price),
				(OraclePeriod::Week, expected_price),
				(OraclePeriod::LastBlock, expected_price),
			]),
		}
		.into()]);
	});
}

#[test]
fn no_oracle_updated_event_when_no_accumulator_data() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		EmaOracle::on_initialize(5);

		// Don't call on_trade - no data in accumulator
		EmaOracle::on_finalize(5);

		// Verify no OracleUpdated event was emitted
		let events = get_oracle_updated_events();
		assert_eq!(events.len(), 0);
	});
}

#[test]
fn oracle_updated_event_emitted_for_each_asset_pair() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		EmaOracle::on_initialize(5);

		// Trade on two different pairs
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, ACA), ORACLE_ENTRY_2));

		EmaOracle::on_finalize(5);

		// Verify two OracleUpdated events were emitted (one per asset pair)
		let events = get_oracle_updated_events();
		assert_eq!(events.len(), 2);
	});
}

#[test]
fn oracle_updated_event_contains_updated_price_after_multiple_trades() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		EmaOracle::on_initialize(5);

		// Multiple trades on same pair - second trade's price should be reflected
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_2));

		EmaOracle::on_finalize(5);

		let events = get_oracle_updated_events();
		assert_eq!(events.len(), 1);

		let (_, _, updates) = &events[0];

		// The price should reflect the accumulated/updated oracle entry
		// For initial oracle (first block), the price should be the last trade's price
		let expected_price = ORACLE_ENTRY_2.with_added_volume_from(&ORACLE_ENTRY_1).price;

		for (_, price) in updates.iter() {
			assert_eq!(*price, expected_price);
		}
	});
}

#[allow(clippy::type_complexity)]
fn get_oracle_updated_events() -> Vec<(Source, (AssetId, AssetId), BTreeMap<OraclePeriod, Price>)> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.filter_map(|record| {
			if let mock::RuntimeEvent::EmaOracle(Event::OracleUpdated {
				source,
				assets,
				updates,
			}) = record.event
			{
				Some((source, assets, updates))
			} else {
				None
			}
		})
		.collect()
}
