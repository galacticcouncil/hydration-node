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
pub use mock::{expect_events, EmaOracle, RuntimeOrigin, Test, DOT, HDX, ORACLE_ENTRY_1};

use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

#[test]
fn add_oracle_should_add_entry_to_storage() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)));

		expect_events(vec![Event::AddedToWhitelist {
			source: SOURCE,
			assets: (HDX, DOT),
		}
		.into()]);
	});
}

#[test]
fn add_oracle_should_store_assets_in_correct_order() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (DOT, HDX)));

		expect_events(vec![Event::AddedToWhitelist {
			source: SOURCE,
			assets: (HDX, DOT),
		}
		.into()]);

		assert!(WhitelistedAssets::<Test>::get().contains(&(SOURCE, (HDX, DOT))));
	});
}

#[test]
fn add_oracle_should_not_fail_when_storage_entry_already_added() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)));
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)));
	});
}

#[test]
fn add_oracle_should_fail_when_storage_is_full() {
	new_test_ext().execute_with(|| {
		let max_entries = <<Test as Config>::MaxUniqueEntries as Get<u32>>::get();

		for i in 0..max_entries {
			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, i)));
		}

		assert_eq!(WhitelistedAssets::<Test>::get().len(), max_entries as usize);

		assert_noop!(
			EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)),
			Error::<Test>::TooManyUniqueEntries
		);
	});
}

#[test]
fn remove_oracle_should_remove_entry_from_storage() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)));
		assert!(WhitelistedAssets::<Test>::get().contains(&(SOURCE, (HDX, DOT))));

		System::set_block_number(5);
		EmaOracle::on_initialize(5);

		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));

		EmaOracle::on_finalize(5);
		System::set_block_number(6);
		EmaOracle::on_initialize(6);

		for period in <Test as crate::Config>::SupportedPeriods::get() {
			assert_eq!(get_oracle_entry(HDX, DOT, period), Some(ORACLE_ENTRY_1),);
		}

		assert_ok!(EmaOracle::remove_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)));
		assert!(!WhitelistedAssets::<Test>::get().contains(&(SOURCE, (HDX, DOT))));

		for period in <Test as crate::Config>::SupportedPeriods::get() {
			assert!(get_oracle_entry(HDX, DOT, period).is_none());
		}

		EmaOracle::on_finalize(6);
		System::set_block_number(7);
		EmaOracle::on_initialize(7);

		expect_events(vec![Event::RemovedFromWhitelist {
			source: SOURCE,
			assets: (HDX, DOT),
		}
		.into()]);
	});
}

#[test]
fn remove_oracle_should_fail_when_oracle_not_tracked() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::remove_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)),
			Error::<Test>::OracleNotFound
		);
	});
}

#[test]
fn on_trade_should_include_whitelisted_oracle_for_correct_source() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::add_oracle(
			RuntimeOrigin::root(),
			SOURCE,
			(HDX, INSUFFICIENT_ASSET)
		));

		assert!(get_accumulator_entry(SOURCE, (HDX, INSUFFICIENT_ASSET)).is_none());

		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			INSUFFICIENT_ASSET,
			1_000,
			500,
			2_000,
			1_000,
			Price::new(2_000, 1_000)
		));

		assert!(get_accumulator_entry(SOURCE, (HDX, INSUFFICIENT_ASSET)).is_some());
		assert!(get_accumulator_entry([0; 8], (HDX, INSUFFICIENT_ASSET)).is_none());
	});
}

#[test]
fn on_liquidity_changed_should_include_whitelisted_oracle_for_correct_source() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::add_oracle(
			RuntimeOrigin::root(),
			SOURCE,
			(HDX, INSUFFICIENT_ASSET)
		));

		assert!(get_accumulator_entry(SOURCE, (HDX, INSUFFICIENT_ASSET)).is_none());

		assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(
			SOURCE,
			HDX,
			INSUFFICIENT_ASSET,
			1_000,
			500,
			2_000,
			1_000,
			Price::new(2_000, 1_000),
		));

		assert!(get_accumulator_entry(SOURCE, (HDX, INSUFFICIENT_ASSET)).is_some());
		assert!(get_accumulator_entry([0; 8], (HDX, INSUFFICIENT_ASSET)).is_none());
	});
}
