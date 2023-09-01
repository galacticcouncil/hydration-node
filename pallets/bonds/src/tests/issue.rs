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
use frame_support::sp_runtime::traits::Zero;
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
		expect_events(vec![
			Event::TokenCreated {
				issuer: ALICE,
				asset_id: HDX,
				bond_id,
				maturity,
			}
			.into(),
			Event::Issued {
				issuer: ALICE,
				bond_id,
				amount,
				fee: 0,
			}
			.into(),
		]);

		assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
		assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

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
			let amount: Balance = ONE;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			// Act
			let bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			// Assert
			expect_events(vec![
				Event::TokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_id,
					maturity,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
			]);

			assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
			assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

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
fn issue_bonds_should_issue_new_bonds_when_bonds_are_already_registered() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount: Balance = ONE;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			let bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			// Act
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			// Assert
			expect_events(vec![
				Event::TokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_id,
					maturity,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
			]);

			assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
			assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

			assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - 2 * amount);

			assert_eq!(Tokens::free_balance(bond_id, &ALICE), 2 * amount_without_fee);

			assert_eq!(
				Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()),
				2 * fee
			);

			assert_eq!(
				Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
				2 * amount_without_fee
			);
		});
}

#[test]
fn issue_bonds_should_register_new_bonds_when_underlying_asset_is_different() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.with_registered_asset(DAI, NATIVE_EXISTENTIAL_DEPOSIT, AssetKind::Token)
		.add_endowed_accounts(vec![(ALICE, DAI, INITIAL_BALANCE)])
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount: Balance = ONE;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			let first_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

			// Act
			let second_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), DAI, amount, maturity));

			// Assert
			expect_events(vec![
				Event::TokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_id: first_bond_id,
					maturity,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id: first_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
				Event::TokenCreated {
					issuer: ALICE,
					asset_id: DAI,
					bond_id: second_bond_id,
					maturity,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id: second_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
			]);

			assert_eq!(Bonds::bond(first_bond_id), Some((HDX, maturity)));
			assert_eq!(Bonds::bond_id((HDX, maturity)), Some(first_bond_id));

			assert_eq!(Bonds::bond(second_bond_id), Some((DAI, maturity)));
			assert_eq!(Bonds::bond_id((DAI, maturity)), Some(second_bond_id));

			assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - amount);
			assert_eq!(Tokens::free_balance(DAI, &ALICE), INITIAL_BALANCE - amount);

			assert_eq!(Tokens::free_balance(first_bond_id, &ALICE), amount_without_fee);
			assert_eq!(Tokens::free_balance(second_bond_id, &ALICE), amount_without_fee);

			assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), fee);
			assert_eq!(Tokens::free_balance(DAI, &<Test as Config>::FeeReceiver::get()), fee);

			assert_eq!(
				Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
				amount_without_fee
			);
			assert_eq!(
				Tokens::free_balance(DAI, &Bonds::pallet_account_id()),
				amount_without_fee
			);
		});
}

#[test]
fn issue_bonds_should_register_new_bonds_when_maturity_is_different() {
	ExtBuilder::default()
		.with_protocol_fee(Permill::from_percent(10))
		.with_registered_asset(DAI, NATIVE_EXISTENTIAL_DEPOSIT, AssetKind::Token)
		.add_endowed_accounts(vec![(ALICE, DAI, INITIAL_BALANCE)])
		.build()
		.execute_with(|| {
			// Arrange
			let next_month = NOW + MONTH;
			let next_week = NOW + WEEK;
			let amount: Balance = ONE;
			let fee = <Test as Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			let first_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, next_month));

			// Act
			let second_bond_id = next_asset_id();
			assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, next_week));

			// Assert
			expect_events(vec![
				Event::TokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_id: first_bond_id,
					maturity: next_month,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id: first_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
				Event::TokenCreated {
					issuer: ALICE,
					asset_id: HDX,
					bond_id: second_bond_id,
					maturity: next_week,
				}
				.into(),
				Event::Issued {
					issuer: ALICE,
					bond_id: second_bond_id,
					amount: amount_without_fee,
					fee,
				}
				.into(),
			]);

			assert_eq!(Bonds::bond(first_bond_id), Some((HDX, next_month)));
			assert_eq!(Bonds::bond_id((HDX, next_month)), Some(first_bond_id));

			assert_eq!(Bonds::bond(second_bond_id), Some((HDX, next_week)));
			assert_eq!(Bonds::bond_id((HDX, next_week)), Some(second_bond_id));

			assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - 2 * amount);

			assert_eq!(Tokens::free_balance(first_bond_id, &ALICE), amount_without_fee);
			assert_eq!(Tokens::free_balance(second_bond_id, &ALICE), amount_without_fee);

			assert_eq!(
				Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()),
				2 * fee
			);

			assert_eq!(
				Tokens::free_balance(HDX, &Bonds::pallet_account_id()),
				2 * amount_without_fee
			);
		});
}

#[test]
fn issue_bonds_should_work_when_bonds_are_mature() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		// Act
		Timestamp::set_timestamp(NOW + 2 * MONTH);

		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		// Assert
		expect_events(vec![
			Event::TokenCreated {
				issuer: ALICE,
				asset_id: HDX,
				bond_id,
				maturity,
			}
			.into(),
			Event::Issued {
				issuer: ALICE,
				bond_id,
				amount,
				fee: 0,
			}
			.into(),
			Event::Issued {
				issuer: ALICE,
				bond_id,
				amount,
				fee: 0,
			}
			.into(),
		]);

		assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
		assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

		assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - 2 * amount);
		assert_eq!(Tokens::free_balance(bond_id, &ALICE), 2 * amount);

		assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), 0);

		assert_eq!(Tokens::free_balance(HDX, &Bonds::pallet_account_id()), 2 * amount);
	});
}

#[test]
fn reissuance_of_bonds_should_work_again_when_all_bonds_were_redeemed() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let maturity = NOW + MONTH;
		let amount = ONE;

		let bond_id = next_asset_id();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		Timestamp::set_timestamp(NOW + 2 * MONTH);

		assert_ok!(Bonds::redeem(RuntimeOrigin::signed(ALICE), bond_id, amount));

		// make sure that all bonds were redeemed and the bonds removed from the storage
		assert!(Tokens::total_issuance(bond_id).is_zero());

		// Act
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, amount, maturity));

		// Assert
		expect_events(vec![
			Event::TokenCreated {
				issuer: ALICE,
				asset_id: HDX,
				bond_id,
				maturity,
			}
			.into(),
			Event::Issued {
				issuer: ALICE,
				bond_id,
				amount,
				fee: 0,
			}
			.into(),
			Event::TokenCreated {
				issuer: ALICE,
				asset_id: HDX,
				bond_id,
				maturity,
			}
			.into(),
			Event::Issued {
				issuer: ALICE,
				bond_id,
				amount,
				fee: 0,
			}
			.into(),
		]);

		assert_eq!(Bonds::bond(bond_id), Some((HDX, maturity)));
		assert_eq!(Bonds::bond_id((HDX, maturity)), Some(bond_id));

		assert_eq!(Tokens::free_balance(HDX, &ALICE), INITIAL_BALANCE - amount);
		assert_eq!(Tokens::free_balance(bond_id, &ALICE), amount);

		assert_eq!(Tokens::free_balance(HDX, &<Test as Config>::FeeReceiver::get()), 0);

		assert_eq!(Tokens::free_balance(HDX, &Bonds::pallet_account_id()), amount);
	});
}

#[test]
fn issue_bonds_should_fail_when_asset_is_blacklisted() {
	let bond_id: AssetId = 10;

	ExtBuilder::default()
		.with_registered_asset(bond_id, NATIVE_EXISTENTIAL_DEPOSIT, AssetKind::Bond)
		.build()
		.execute_with(|| {
			// Arrange
			let maturity = NOW + MONTH;
			let amount = ONE;

			assert_ok!(Tokens::deposit(bond_id, &ALICE, amount));

			// Act & Assert
			assert_noop!(
				Bonds::issue(RuntimeOrigin::signed(ALICE), bond_id, amount, maturity),
				Error::<Test>::DisallowedAsset
			);
		});
}

#[test]
fn issue_bonds_should_fail_when_maturity_is_in_the_past() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Bonds::issue(RuntimeOrigin::signed(ALICE), HDX, ONE, NOW - DAY),
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
			DispatchError::Other("AssetNotFound")
		);
	});
}

#[test]
fn issue_bonds_should_fail_when_called_from_wrong_origin() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = next_asset_id();

		assert_noop!(
			Bonds::issue(RuntimeOrigin::signed(3u64), asset_id, ONE, NOW + MONTH),
			DispatchError::BadOrigin
		);
	});
}
