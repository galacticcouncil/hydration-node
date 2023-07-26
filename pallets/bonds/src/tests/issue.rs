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
fn issue_bonds_should_work_when_fee_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;

		// Act
		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		// Assert
		expect_events(vec![Event::BondTokenCreated {
			issuer: ALICE,
			asset_id: HDX,
			bond_asset_id: bond_id,
			amount,
			fee: 0,
		}
		.into()]);

		assert_eq!(
			Bonds::bonds(bond_id).unwrap(),
			Bond {
				maturity,
				asset_id: HDX,
				amount,
			}
		);

		assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - amount);
		assert_eq!(Tokens::free_balance(bond_id, &ALICE), amount);

		assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), 0);

		assert_eq!(Tokens::free_balance(HDX, &Bonds::pallet_account_id()), amount);
	});
}

#[test]
fn issue_bonds_should_work_when_fee_is_non_zero() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount: Balance = 1_000_000;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			// Act
			let bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			// Assert
			expect_events(vec![Event::BondTokenCreated {
				issuer: ALICE,
				asset_id: HDX,
				bond_asset_id: bond_id,
				amount: amount_without_fee,
				fee,
			}
			.into()]);

			assert_eq!(
				Bonds::bonds(bond_id).unwrap(),
				Bond {
					maturity,
					asset_id: HDX,
					amount: amount_without_fee,
				}
			);

			assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - amount);
			assert_eq!(Tokens::free_balance(bond_id, &ALICE), amount_without_fee);

			assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), fee);

			assert_eq!(
				Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
				amount_without_fee
			);
		});
}

#[test]
fn issue_bonds_should_work_when_issuing_multiple_bonds() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.with_registered_asset(DAI, NATIVE_EXISTENTIAL_DEPOSIT)
		.add_endowed_accounts(vec![(BOB, DAI, INITIAL_BALANCE)])
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount: Balance = 1_000_000;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			// Act
			let first_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			let second_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			let third_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(BOB), DAI, amount, maturity));

			// Assert
			expect_events(vec![
				Event::BondTokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_asset_id: first_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
				Event::BondTokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_asset_id: second_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
				Event::BondTokenCreated {
					issuer: BOB,
					asset_id: DAI,
					bond_asset_id: third_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
			]);

			assert_eq!(
				Bonds::bonds(first_bond_id).unwrap(),
				Bond {
					maturity,
					asset_id: HDX,
					amount: amount_without_fee,
				}
			);
			assert_eq!(
				Bonds::bonds(second_bond_id).unwrap(),
				Bond {
					maturity,
					asset_id: HDX,
					amount: amount_without_fee,
				}
			);
			assert_eq!(
				Bonds::bonds(third_bond_id).unwrap(),
				Bond {
					maturity,
					asset_id: DAI,
					amount: amount_without_fee,
				}
			);

			assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - 2 * amount);
			assert_eq!(Tokens::free_balance(DAI, &BOB), INITIAL_BALANCE - amount);

			assert_eq!(Tokens::free_balance(first_bond_id, &ALICE), amount_without_fee);
			assert_eq!(Tokens::free_balance(second_bond_id, &ALICE), amount_without_fee);
			assert_eq!(Tokens::free_balance(third_bond_id, &BOB), amount_without_fee);

			assert_eq!(
				Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()),
				2 * fee
			);
			assert_eq!(Tokens::free_balance(DAI, &<Test as Config>::FeeReceiver::get()), fee);

			assert_eq!(
				Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
				2 * amount_without_fee
			);
			assert_eq!(
				Tokens::free_balance(DAI, &Bonds::pallet_account_id()),
				amount_without_fee
			);
		});
}

#[test]
fn issue_bonds_should_fail_when_maturity_is_in_the_past() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, ONE, NOW - DAY),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn issue_bonds_should_fail_when_maturity_is_too_soon() {
	ExtBuilder::default().build().execute_with(|| {
		// T::MinMaturity set to one week
		assert_noop!(
			Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, ONE, NOW + DAY),
			Error::<Test>::InvalidMaturity
		);
	});
}

#[test]
fn issue_bonds_should_fail_when_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Bonds::issue(RuntimeOrigin::signed(BOB), HDX, ONE, NOW + MONTH),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
fn issue_bonds_should_fail_when_underlying_asset_not_registered() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = next_asset_id();
		assert_noop!(
			Bonds::issue(RuntimeOrigin::signed(ALICE), asset_id, ONE, NOW + MONTH),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}
