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

use super::mock::{self, EmaOracle, ExtBuilder, RuntimeOrigin, System, Test, ALICE, BOB};
use super::SOURCE;
use crate::pallet::{AuthorizedAccounts, ExternalSources};
use crate::*;

use frame_support::pallet_prelude::*;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

const EXTERNAL_SOURCE: Source = *b"external";
const ANOTHER_SOURCE: Source = *b"another_";

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

#[test]
fn register_external_source_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert!(ExternalSources::<Test>::contains_key(EXTERNAL_SOURCE));
	});
}

#[test]
fn register_duplicate_source_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_noop!(
			EmaOracle::register_external_source(RuntimeOrigin::root(), EXTERNAL_SOURCE),
			Error::<Test>::SourceAlreadyRegistered
		);
	});
}

#[test]
fn register_external_source_requires_authority() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::register_external_source(RuntimeOrigin::signed(ALICE), EXTERNAL_SOURCE),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_external_source_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));
		assert_ok!(EmaOracle::remove_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert!(!ExternalSources::<Test>::contains_key(EXTERNAL_SOURCE));
		assert!(!AuthorizedAccounts::<Test>::contains_key(EXTERNAL_SOURCE, ALICE));
	});
}

#[test]
fn remove_nonexistent_source_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::remove_external_source(RuntimeOrigin::root(), EXTERNAL_SOURCE),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn add_authorized_account_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));
		assert!(AuthorizedAccounts::<Test>::contains_key(EXTERNAL_SOURCE, ALICE));
	});
}

#[test]
fn add_account_for_nonexistent_source_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::add_authorized_account(RuntimeOrigin::root(), EXTERNAL_SOURCE, ALICE),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn remove_authorized_account_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));
		assert_ok!(EmaOracle::remove_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));
		assert!(!AuthorizedAccounts::<Test>::contains_key(EXTERNAL_SOURCE, ALICE));
	});
}

#[test]
fn set_external_oracle_happy_path() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));

		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		System::set_block_number(3);

		let res = EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx),
			Box::new(dot),
			(100, 99),
		);
		assert_eq!(res, Ok(Pays::No.into()));

		// Verify the entry is in the accumulator
		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn set_external_oracle_unauthorized_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx),
				Box::new(dot),
				(100, 99),
			),
			Error::<Test>::NotAuthorized
		);
	});
}

#[test]
fn set_external_oracle_zero_price_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));

		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx.clone()),
				Box::new(dot.clone()),
				(0, 100),
			),
			Error::<Test>::PriceIsZero
		);

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx),
				Box::new(dot),
				(100, 0),
			),
			Error::<Test>::PriceIsZero
		);
	});
}

#[test]
fn set_external_oracle_unregistered_source_rejected() {
	new_test_ext().execute_with(|| {
		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx),
				Box::new(dot),
				(100, 99),
			),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn external_sources_bypass_whitelist() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));

		// Use INSUFFICIENT_ASSET which is normally excluded by the whitelist
		let asset_a_loc = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let asset_b_loc = polkadot_xcm::v5::Location::parent().into_versioned();

		System::set_block_number(3);

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(asset_a_loc),
			Box::new(asset_b_loc),
			(100, 99),
		));

		// Verify the entry is in the accumulator (bypasses whitelist)
		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn multiple_sources_in_same_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE,
			BOB
		));

		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		System::set_block_number(3);

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx.clone()),
			Box::new(dot.clone()),
			(100, 99),
		));

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(BOB),
			ANOTHER_SOURCE,
			Box::new(hdx),
			Box::new(dot),
			(200, 99),
		));

		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
		assert!(acc.contains_key(&(ANOTHER_SOURCE, ordered_pair(0, 5))));
	});
}

// --- soft limit ---

#[test]
fn amm_trades_are_limited_to_max_unique_entries() {
	new_test_ext().execute_with(|| {
		//Arrange
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();

		//Act - fill the accumulator to max
		for i in 0..max_entries {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			));
		}

		//Assert - accumulator is full, next AMM trade fails
		assert_eq!(Accumulator::<Test>::get().len(), max_entries as usize);
		assert_noop!(
			OnActivityHandler::<Test>::on_trade(
				SOURCE,
				2 * max_entries,
				2 * max_entries + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			)
			.map_err(|(_w, e)| e),
			Error::<Test>::TooManyUniqueEntries
		);
	});
}

#[test]
fn external_sources_can_add_entries_beyond_max_unique_entries() {
	new_test_ext().execute_with(|| {
		//Arrange - fill accumulator to max with AMM trades
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();
		for i in 0..max_entries {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			));
		}

		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));

		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		//Act - external source adds entry beyond the soft limit
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx),
			Box::new(dot),
			(100, 99),
		));

		//Assert - accumulator has more entries than MaxUniqueEntries
		assert_eq!(Accumulator::<Test>::get().len(), (max_entries + 1) as usize);
	});
}

#[test]
fn soft_limit_only_for_non_external_sources() {
	new_test_ext().execute_with(|| {
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();

		// Fill the accumulator with non-external entries
		for i in 0..max_entries {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			));
		}

		// Non-external source should fail
		assert_noop!(
			OnActivityHandler::<Test>::on_trade(
				SOURCE,
				2 * max_entries,
				2 * max_entries + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			)
			.map_err(|(_w, e)| e),
			Error::<Test>::TooManyUniqueEntries
		);

		// But external sources should still be able to insert
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			ALICE
		));

		let hdx = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		// External source can still insert past the soft limit
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx),
			Box::new(dot),
			(100, 99),
		));
	});
}
