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

use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok, assert_storage_noop};
pub use pretty_assertions::{assert_eq, assert_ne};
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn unlock_should_work_when_bonds_are_not_mature() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let bond_id = next_asset_id();

		// Act
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * WEEK);

		assert_ok!(Bonds::unlock(RuntimeOrigin::root(), bond_id));

		// Assert
		expect_events(vec![Event::BondsUnlocked { bond_id }.into()]);

		assert_eq!(
			Bonds::bonds(bond_id).unwrap(),
			Bond {
				maturity: NOW + 2 * WEEK,
				asset_id: HDX,
				amount,
			}
		);
	});
}

#[test]
fn unlock_should_be_storage_noop_if_bonds_are_already_mature() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let bond_id = next_asset_id();

		// Act
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		// Assert
		assert_storage_noop!(Bonds::unlock(RuntimeOrigin::root(), bond_id).unwrap());
	});
}

#[test]
fn unlock_should_fail_when_called_from_wrong_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let bond_id = next_asset_id();

		// Act
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		assert_noop!(Bonds::unlock(RuntimeOrigin::signed(ALICE), bond_id), BadOrigin);
	});
}
