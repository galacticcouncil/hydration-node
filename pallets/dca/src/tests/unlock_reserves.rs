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
use crate::tests::*;
use crate::{
	assert_balance,
	Error,
	Event,
};
use frame_support::{
	assert_noop,
	assert_ok,
	traits::Hooks,
};
use orml_traits::{
	MultiCurrency,
	NamedMultiReservableCurrency,
};
use pretty_assertions::assert_eq;

#[test]
fn unlock_should_not_work_when_user_has_active_schedule() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);
			let schedule = ScheduleBuilder::new().build();
			assert_ok!(Currencies::reserve_named(&NamedReserveId::get(), HDX, &ALICE, 10 * ONE));

			assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE), schedule, Option::Some(600)));

			//Act
			assert_noop!(
				DCA::unlock_reserves(RuntimeOrigin::signed(ALICE), ALICE, HDX),
				Error::<Test>::HasActiveSchedules
			);
		});
}

#[test]
fn unlock_should_unreserve_when_user_has_leftover() {
	let init_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, init_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			let leftover = 10 * ONE;
			assert_ok!(Currencies::reserve_named(&NamedReserveId::get(), HDX, &ALICE, 10 * ONE));

			assert_balance!(ALICE, HDX, init_balance - leftover);

			//Act
			assert_ok!(DCA::unlock_reserves(RuntimeOrigin::signed(ALICE), ALICE, HDX));

			//Assert
			assert_balance!(ALICE, HDX, init_balance);
			expect_events(vec![Event::ReserveUnlocked {
				who: ALICE,
				asset_id: HDX,
			}
			.into()]);
		});
}

#[test]
fn unlock_should_work_when_called_by_root() {
	let init_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, init_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			let leftover = 10 * ONE;
			assert_ok!(Currencies::reserve_named(&NamedReserveId::get(), HDX, &ALICE, 10 * ONE));

			assert_balance!(ALICE, HDX, init_balance - leftover);

			//Act
			assert_ok!(DCA::unlock_reserves(RuntimeOrigin::root(), ALICE, HDX));

			//Assert
			assert_balance!(ALICE, HDX, init_balance);
		});
}

#[test]
fn unlock_should_work_when_called_by_other_user() {
	let init_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, init_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			let leftover = 10 * ONE;
			assert_ok!(Currencies::reserve_named(&NamedReserveId::get(), HDX, &ALICE, 10 * ONE));

			assert_balance!(ALICE, HDX, init_balance - leftover);

			//Act
			assert_ok!(DCA::unlock_reserves(RuntimeOrigin::signed(BOB), ALICE, HDX));

			//Assert
			assert_balance!(ALICE, HDX, init_balance);
		});
}

#[test]
fn unlock_should_not_work_when_nothing_is_reserved() {
	let init_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, init_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			//Act
			assert_noop!(
				DCA::unlock_reserves(RuntimeOrigin::signed(ALICE), ALICE, HDX),
				Error::<Test>::NoReservesLocked
			);
		});
}

#[test]
fn unlock_should_fail_when_asset_doesnt_exist() {
	let init_balance = 10000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, init_balance)])
		.build()
		.execute_with(|| {
			//Arrange
			set_block_number(500);

			//Act
			assert_noop!(
				DCA::unlock_reserves(RuntimeOrigin::signed(ALICE), ALICE, 9999),
				Error::<Test>::NoReservesLocked
			);
		});
}

pub fn set_block_number(to: u64) {
	System::set_block_number(to);
	DCA::on_initialize(to);
}
