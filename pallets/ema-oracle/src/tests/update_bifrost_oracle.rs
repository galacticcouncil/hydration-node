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
pub use mock::{EmaOracle, RuntimeOrigin, Test};

use frame_support::{assert_noop, assert_ok};

use crate::tests::mock::BOB;
use pretty_assertions::assert_eq;

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

use crate::tests::mock::ALICE;
use polkadot_xcm::v5::prelude::*;

fn setup_bifrost_auth() {
	assert_ok!(EmaOracle::register_external_source(
		RuntimeOrigin::root(),
		BIFROST_SOURCE
	));
	assert_ok!(EmaOracle::add_authorized_account(
		RuntimeOrigin::root(),
		BIFROST_SOURCE,
		ALICE
	));
}

#[test]
fn add_oracle_should_add_entry_to_storage() {
	new_test_ext().execute_with(|| {
		//Arrange
		setup_bifrost_auth();

		let hdx = polkadot_xcm::v5::Location::new(0, polkadot_xcm::v5::Junctions::X1([GeneralIndex(0)].into()))
			.into_versioned();

		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		//Act
		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			(100, 99)
		));

		update_aggregated_oracles();

		//Assert
		let entry = Oracles::<Test>::get((BIFROST_SOURCE, ordered_pair(0, 5), OraclePeriod::Day)).map(|(e, _)| e);
		assert!(entry.is_some());
		let entry = entry.unwrap();
		assert_eq!(entry.price, EmaPrice::new(100, 99));
		assert_eq!(entry.volume, Volume::default());
		assert_eq!(entry.liquidity, Liquidity::default());
		assert_eq!(entry.updated_at, 3);
	});
}

#[test]
fn successful_oracle_update_shouldnt_pay_fee() {
	new_test_ext().execute_with(|| {
		//Arrange
		setup_bifrost_auth();

		let hdx = polkadot_xcm::v5::Location::new(0, polkadot_xcm::v5::Junctions::X1([GeneralIndex(0)].into()))
			.into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		//Act
		let res =
			EmaOracle::update_bifrost_oracle(RuntimeOrigin::signed(ALICE), Box::new(hdx), Box::new(dot), (100, 99));

		//Assert
		assert_eq!(res, Ok(Pays::No.into()));
	});
}

#[test]
fn add_oracle_should_add_entry_to_storage_with_inversed_pair() {
	new_test_ext().execute_with(|| {
		//Arrange
		setup_bifrost_auth();

		let hdx = polkadot_xcm::v5::Location::new(0, [polkadot_xcm::v5::Junction::GeneralIndex(0)]).into_versioned();

		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		//Act
		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE),
			asset_b,
			asset_a,
			(100, 99)
		));

		update_aggregated_oracles();

		//Assert
		let entry = Oracles::<Test>::get((BIFROST_SOURCE, ordered_pair(0, 5), OraclePeriod::Day)).map(|(e, _)| e);
		assert!(entry.is_some());
		let entry = entry.unwrap();
		assert_eq!(entry.price, EmaPrice::new(99, 100));
		assert_eq!(entry.volume, Volume::default());
		assert_eq!(entry.liquidity, Liquidity::default());
		assert_eq!(entry.updated_at, 3);
	});
}

#[test]
fn bifrost_oracle_should_not_be_updated_by_nonprivileged_account() {
	new_test_ext().execute_with(|| {
		//Arrange
		setup_bifrost_auth();

		let hdx = polkadot_xcm::v5::Location::new(0, [polkadot_xcm::v5::Junction::GeneralIndex(0)]).into_versioned();

		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		System::set_block_number(3);

		//Act & Assert
		assert_noop!(
			EmaOracle::update_bifrost_oracle(RuntimeOrigin::signed(BOB), asset_a, asset_b, (100, 99)),
			Error::<Test>::NotAuthorized
		);
	});
}

#[test]
fn should_fail_when_price_is_zero() {
	new_test_ext().execute_with(|| {
		//Arrange
		setup_bifrost_auth();

		let hdx = polkadot_xcm::v5::Location::new(0, [polkadot_xcm::v5::Junction::GeneralIndex(0)]).into_versioned();
		let dot = polkadot_xcm::v5::Location::parent().into_versioned();

		System::set_block_number(3);

		//Act & Assert
		assert_noop!(
			EmaOracle::update_bifrost_oracle(
				RuntimeOrigin::signed(ALICE),
				Box::new(hdx.clone()),
				Box::new(dot.clone()),
				(0, 100)
			),
			Error::<Test>::PriceIsZero
		);

		assert_noop!(
			EmaOracle::update_bifrost_oracle(RuntimeOrigin::signed(ALICE), Box::new(hdx), Box::new(dot), (100, 0)),
			Error::<Test>::PriceIsZero
		);
	});
}

pub fn update_aggregated_oracles() {
	EmaOracle::on_finalize(6);
	System::set_block_number(7);
	EmaOracle::on_initialize(7);
}
