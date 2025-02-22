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
use polkadot_xcm::v3::Junction::GeneralIndex;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn add_oracle_should_add_entry_to_storage() {
	new_test_ext().execute_with(|| {
		//Arrange
		let hdx =
			polkadot_xcm::v3::MultiLocation::new(0, polkadot_xcm::v3::Junctions::X1(GeneralIndex(0))).into_versioned();

		let dot = polkadot_xcm::v3::MultiLocation::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		//Act
		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE.into()),
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
fn add_oracle_should_add_entry_to_storage_with_inversed_pair() {
	new_test_ext().execute_with(|| {
		//Arrange
		let hdx =
			polkadot_xcm::v3::MultiLocation::new(0, polkadot_xcm::v3::Junctions::X1(GeneralIndex(0))).into_versioned();

		let dot = polkadot_xcm::v3::MultiLocation::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		//Act
		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE.into()),
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
fn bitfrost_oracle_should_not_be_updated_by_nonpriviliged_account() {
	new_test_ext().execute_with(|| {
		//Arrange
		let hdx =
			polkadot_xcm::v3::MultiLocation::new(0, polkadot_xcm::v3::Junctions::X1(GeneralIndex(0))).into_versioned();

		let dot = polkadot_xcm::v3::MultiLocation::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		//Act
		System::set_block_number(3);

		assert_noop!(
			EmaOracle::update_bifrost_oracle(RuntimeOrigin::signed(BOB.into()), asset_a, asset_b, (100, 99)),
			BadOrigin
		);
	});
}

#[test]
fn should_fail_when_new_price_is_bigger_than_allowed() {
	new_test_ext().execute_with(|| {
		//Arrange
		let hdx =
			polkadot_xcm::v3::MultiLocation::new(0, polkadot_xcm::v3::Junctions::X1(GeneralIndex(0))).into_versioned();

		let dot = polkadot_xcm::v3::MultiLocation::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a.clone(),
			asset_b.clone(),
			(100, 100)
		));

		update_aggregated_oracles();

		//Act
		assert_noop!(
			EmaOracle::update_bifrost_oracle(
				RuntimeOrigin::signed(ALICE.into()),
				asset_a.clone(),
				asset_b.clone(),
				(111, 100)
			),
			Error::<Test>::PriceOutsideAllowedRange
		);

		assert_noop!(
			EmaOracle::update_bifrost_oracle(RuntimeOrigin::signed(ALICE.into()), asset_a, asset_b, (89, 100)),
			Error::<Test>::PriceOutsideAllowedRange
		);
	});
}

#[test]
fn should_pass_when_new_price_is_still_within_range() {
	new_test_ext().execute_with(|| {
		//Arrange
		let hdx =
			polkadot_xcm::v3::MultiLocation::new(0, polkadot_xcm::v3::Junctions::X1(GeneralIndex(0))).into_versioned();

		let dot = polkadot_xcm::v3::MultiLocation::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a.clone(),
			asset_b.clone(),
			(100, 100)
		));

		update_aggregated_oracles();

		//Act
		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a.clone(),
			asset_b.clone(),
			(110, 100)
		),);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			asset_b,
			(90, 100)
		),);
	});
}

pub fn update_aggregated_oracles() {
	EmaOracle::on_finalize(6);
	System::set_block_number(7);
	EmaOracle::on_initialize(7);
}

//TODO: add negative test when it is not called by bitfrost origni
