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
pub type Bonds = Pallet<Test>;
use frame_support::{assert_noop, assert_ok};
pub use pretty_assertions::{assert_eq, assert_ne};

#[test]
fn partially_redeem_bonds_should_work_when_fee_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let redeem_amount = ONE.checked_div(4).unwrap();

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		// Act
		assert_ok!(Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, redeem_amount));

		// Assert
		expect_events(vec![Event::Redeemed {
			who: ALICE,
			bond_id,
			amount: redeem_amount,
		}
		.into()]);

		assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
		assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

		assert_eq!(
			Tokens::free_balance(HDX, &ALICE),
			INITIAL_BALANCE - amount + redeem_amount
		);
		assert_eq!(Tokens::free_balance(bond_id, &ALICE), amount - redeem_amount);

		assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), 0);

		assert_eq!(
			Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
			amount - redeem_amount
		);
	});
}

#[test]
fn partially_redeem_bonds_should_work_when_fee_is_non_zero() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount = ONE;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();
			let redeem_amount = amount_without_fee.checked_div(4).unwrap();

			let bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			Timestamp::set_timestamp(NOW + 2 * MONTH);

			// Act
			assert_ok!(Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, redeem_amount));

			// Assert
			expect_events(vec![Event::Redeemed {
				who: ALICE,
				bond_id,
				amount: redeem_amount,
			}
			.into()]);

			assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
			assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

			assert_eq!(
				Tokens::free_balance(HDX, &ALICE),
				INITIAL_BALANCE - amount + redeem_amount
			);
			assert_eq!(
				Tokens::free_balance(bond_id, &ALICE),
				amount_without_fee - redeem_amount
			);

			assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), fee);

			assert_eq!(
				Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
				amount_without_fee - redeem_amount
			);
		});
}

#[test]
fn fully_redeem_bonds_should_work_when_fee_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		// Act
		assert_ok!(Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, amount));

		// Assert
		expect_events(vec![Event::Redeemed {
			who: ALICE,
			bond_id,
			amount,
		}
		.into()]);

		assert!(crate::Bonds::<Test>::contains_key(bond_id));
		assert!(crate::BondIds::<Test>::contains_key((HDX, maturity)));

		assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE);
		assert_eq!(Tokens::free_balance(bond_id, &ALICE), 0);

		assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), 0);

		assert_eq!(Tokens::free_balance(HDX, &Bonds::pallet_account_id()), 0);
	});
}

#[test]
fn fully_redeem_bonds_should_work_when_fee_is_non_zero() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount = ONE;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			let bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			Timestamp::set_timestamp(NOW + 2 * MONTH);

			// Act
			assert_ok!(Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, amount_without_fee));

			// Assert
			expect_events(vec![Event::Redeemed {
				who: ALICE,
				bond_id,
				amount: amount_without_fee,
			}
			.into()]);

			assert!(crate::Bonds::<Test>::contains_key(bond_id));
			assert!(crate::BondIds::<Test>::contains_key((HDX, maturity)));

			assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - fee);
			assert_eq!(Tokens::free_balance(bond_id, &ALICE), 0);

			assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), fee);

			assert_eq!(Tokens::free_balance(HDX, &Bonds::pallet_account_id()), 0);
		});
}

#[test]
fn redeem_bonds_should_work_when_redeemed_from_non_issuer_account() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let redeem_amount = ONE.checked_div(4).unwrap();

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		assert_ok!(Tokens::transfer(
			RuntimeOrigin::signed(ALICE),
			BOB,
			bond_id,
			redeem_amount
		));

		Timestamp::set_timestamp(NOW + (2 * MONTH));

		// Act

		assert_ok!(Bonds::redeem(RuntimeOrigin::signed(BOB), bond_id, redeem_amount));

		// Assert
		expect_events(vec![Event::Redeemed {
			who: BOB,
			bond_id,
			amount: redeem_amount,
		}
		.into()]);

		assert_eq!(Tokens::free_balance(HDX, &BOB), redeem_amount);
		assert_eq!(Tokens::free_balance(bond_id, &BOB), 0);

		assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), 0);

		assert_eq!(
			Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
			amount - redeem_amount
		);
	});
}

#[test]
fn redeem_bonds_should_work_when_maturity_equals_now() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let redeem_amount = ONE.checked_div(4).unwrap();

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + MONTH);

		// Act & Assert
		assert_ok!(Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, redeem_amount));
	});
}

#[test]
fn redeem_bonds_should_fail_when_bond_not_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let bond_id = next_asset_id();

		// Act & Assert

		// asset not registered
		assert_noop!(
			Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, ONE),
			Error::<Test>::NotRegistered
		);

		// asset registered, but not as a bond token
		assert_noop!(
			Bonds::redeem(RuntimeOrigin::signed(ALICE), DAI, ONE),
			Error::<Test>::NotRegistered
		);
	});
}

#[test]
fn redeem_bonds_should_fail_when_not_mature() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;
		let redeem_amount = ONE.checked_div(4).unwrap();

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + WEEK);

		// Act & Assert
		assert_noop!(
			Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, redeem_amount),
			Error::<Test>::NotMature
		);
	});
}

#[test]
fn redeem_bonds_should_fail_when_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		// Act & Assert
		assert_noop!(
			Bonds::redeem(RuntimeOrigin::signed(BOB), bond_id, amount),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
// this case should normally never happen
fn redeem_bonds_should_fail_when_the_amount_is_greater_then_total_issued() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		// bypass the pallet and increase the issuance of the bonds
		assert_ok!(Tokens::deposit(bond_id, &ALICE, amount));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		// Act & Assert
		assert_noop!(
			Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, 2 * amount),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}
