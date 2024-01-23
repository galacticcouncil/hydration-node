// This file is part of Basilisk-node.

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

pub use crate::{mock::*, Config, Error};
use crate::{AcceptedCurrencies, AcceptedCurrencyPrice, Event, PaymentInfo, Price};

use frame_support::traits::tokens::Precision;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::{DispatchInfo, PostDispatchInfo},
	sp_runtime::traits::{BadOrigin, SignedExtension},
	traits::Hooks,
	weights::Weight,
};
use orml_traits::MultiCurrency;
use pallet_balances::Call as BalancesCall;
use pallet_transaction_payment::ChargeTransactionPayment;

const CALL: &<Test as frame_system::Config>::RuntimeCall =
	&RuntimeCall::Balances(BalancesCall::transfer { dest: 2, value: 69 });

#[test]
fn on_initialize_should_fill_storage_with_prices() {
	// Arrange
	ExtBuilder::default().build().execute_with(|| {
		// Act
		let current = System::block_number();
		PaymentPallet::on_finalize(current);
		// the block number is not important here and can stay the same
		PaymentPallet::on_initialize(current);

		// Assert
		// verify that all accepted currencies have the price set
		let iter = <AcceptedCurrencies<Test>>::iter();
		for (asset_id, _) in iter {
			assert!(<AcceptedCurrencyPrice<Test>>::contains_key(asset_id));
		}

		// fallback price
		assert_eq!(
			PaymentPallet::currency_price(SUPPORTED_CURRENCY),
			Some(Price::from_float(1.5))
		);
		// price from the spot price provider
		assert_eq!(
			PaymentPallet::currency_price(SUPPORTED_CURRENCY_WITH_PRICE),
			Some(Price::from_float(0.1))
		);
		// not supported
		assert_eq!(PaymentPallet::currency_price(UNSUPPORTED_CURRENCY), None);
	});
}

#[test]
fn on_finalize_should_remove_prices_from_storage() {
	// Arrange
	ExtBuilder::default().build().execute_with(|| {
		let current = System::block_number();

		// verify that the storage is not empty
		assert_eq!(
			PaymentPallet::currency_price(SUPPORTED_CURRENCY),
			Some(Price::from_float(1.5))
		);

		// Act
		PaymentPallet::on_finalize(current);

		// Assert
		let mut iter = <AcceptedCurrencyPrice<Test>>::iter_values();
		assert_eq!(iter.next(), None);
	});
}

#[test]
fn set_unsupported_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			PaymentPallet::set_currency(RuntimeOrigin::signed(BOB), UNSUPPORTED_CURRENCY),
			Error::<Test>::UnsupportedCurrency
		);

		assert_eq!(PaymentPallet::get_currency(BOB), None);
	});
}

#[test]
fn set_supported_currency_without_spot_price_should_charge_fee_in_correct_currency() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		let call = &RuntimeCall::PaymentPallet(crate::Call::set_currency {
			currency: SUPPORTED_CURRENCY,
		});

		let len = 10;
		let info = info_from_weight(Weight::from_parts(5, 0));

		let pre = ChargeTransactionPayment::<Test>::from(0).pre_dispatch(&ALICE, call, &info, len);
		assert!(pre.is_ok());

		assert_eq!(
			Currencies::free_balance(SUPPORTED_CURRENCY, &ALICE),
			999_999_999_999_970
		);

		assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&default_post_info(),
			len,
			&Ok(())
		));
		assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FEE_RECEIVER), 30);
	});
}

#[test]
fn set_supported_currency_with_spot_price_should_charge_fee_in_correct_currency() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		let call = &RuntimeCall::PaymentPallet(crate::Call::set_currency {
			currency: SUPPORTED_CURRENCY_WITH_PRICE,
		});

		let len = 10;
		let info = info_from_weight(Weight::from_parts(5, 0));

		let pre = ChargeTransactionPayment::<Test>::from(0).pre_dispatch(&ALICE, call, &info, len);
		assert!(pre.is_ok());

		assert_eq!(
			Currencies::free_balance(SUPPORTED_CURRENCY_WITH_PRICE, &ALICE),
			999_999_999_999_998
		);

		assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
			Some(pre.unwrap()),
			&info,
			&default_post_info(),
			len,
			&Ok(())
		));
		assert_eq!(
			Currencies::free_balance(SUPPORTED_CURRENCY_WITH_PRICE, &FEE_RECEIVER),
			2
		);
	});
}

#[test]
fn set_native_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(PaymentPallet::set_currency(RuntimeOrigin::signed(ALICE), HDX),);

		assert_eq!(PaymentPallet::get_currency(ALICE), Some(HDX));
	});
}

#[test]
fn fee_payment_in_native_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 100)
		.build()
		.execute_with(|| {
			let len = 10;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_ok());

			assert_eq!(Balances::free_balance(CHARLIE), 100 - 5 - 5 - 10);
		});
}

#[test]
fn fee_payment_in_non_native_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY_WITH_PRICE, 10_000)
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY_WITH_PRICE)])
		.build()
		.execute_with(|| {
			// Make sure Charlie ain't got a penny!
			assert_eq!(Balances::free_balance(CHARLIE), 0);

			let len = 1000;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY_WITH_PRICE, &CHARLIE), 10_000);

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_ok());

			//Native balance check - Charlie should be still broke!
			assert_eq!(Balances::free_balance(CHARLIE), 0);

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY_WITH_PRICE, &CHARLIE), 9899);
		});
}

#[test]
fn fee_payment_in_expensive_non_native_currency_should_be_non_zero() {
	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(BOB, HIGH_VALUE_CURRENCY, 10_000)
		.with_currencies(vec![(BOB, HIGH_VALUE_CURRENCY)])
		.build()
		.execute_with(|| {
			let len = 100;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert_eq!(Tokens::free_balance(HIGH_VALUE_CURRENCY, &BOB), 10_000);

			let pre = ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&BOB, CALL, &info, len)
				.unwrap();

			// Bob should be charged at least 1 token
			assert_eq!(Tokens::free_balance(HIGH_VALUE_CURRENCY, &BOB), 9999);

			let post_info = post_info_from_weight(Weight::from_parts(3, 0));
			assert!(
				ChargeTransactionPayment::<Test>::post_dispatch(Some(pre), &info, &post_info, len, &Ok(())).is_ok()
			);
			// BOB should not be refunded in case he payed only 1 token
			assert_eq!(Tokens::free_balance(HIGH_VALUE_CURRENCY, &BOB), 9999);
		});
}

#[test]
fn fee_payment_non_native_insufficient_balance() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 100)
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.build()
		.execute_with(|| {
			let len = 1000;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 100);
		});
}

#[test]
fn add_new_accepted_currency() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		assert_ok!(PaymentPallet::add_currency(
			RuntimeOrigin::root(),
			100,
			Price::from_float(1.1)
		));
		expect_events(vec![Event::CurrencyAdded { asset_id: 100 }.into()]);

		assert_eq!(PaymentPallet::currencies(100), Some(Price::from_float(1.1)));
		assert_noop!(
			PaymentPallet::add_currency(RuntimeOrigin::signed(ALICE), 1000, Price::from_float(1.2)),
			BadOrigin
		);
		assert_noop!(
			PaymentPallet::add_currency(RuntimeOrigin::root(), 100, Price::from(10)),
			Error::<Test>::AlreadyAccepted
		);
		assert_eq!(PaymentPallet::currencies(100), Some(Price::from_float(1.1)));
	});
}

#[test]
fn removed_accepted_currency() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		assert_ok!(PaymentPallet::add_currency(RuntimeOrigin::root(), 100, Price::from(3)));
		assert_eq!(PaymentPallet::currencies(100), Some(Price::from(3)));

		assert_noop!(
			PaymentPallet::remove_currency(RuntimeOrigin::signed(ALICE), 100),
			BadOrigin
		);

		assert_noop!(
			PaymentPallet::remove_currency(RuntimeOrigin::root(), 1000),
			Error::<Test>::UnsupportedCurrency
		);

		assert_ok!(PaymentPallet::remove_currency(RuntimeOrigin::root(), 100));
		expect_events(vec![Event::CurrencyRemoved { asset_id: 100 }.into()]);

		assert_eq!(PaymentPallet::currencies(100), None);

		assert_noop!(
			PaymentPallet::remove_currency(RuntimeOrigin::root(), 100),
			Error::<Test>::UnsupportedCurrency
		);
	});
}

#[test]
fn account_currency_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PaymentPallet::account_currency(&ALICE), HDX);

		assert_ok!(PaymentPallet::set_currency(
			RuntimeOrigin::signed(ALICE),
			SUPPORTED_CURRENCY
		));
		expect_events(vec![Event::CurrencySet {
			account_id: ALICE,
			asset_id: SUPPORTED_CURRENCY,
		}
		.into()]);

		assert_eq!(PaymentPallet::account_currency(&ALICE), SUPPORTED_CURRENCY);

		assert_ok!(PaymentPallet::set_currency(RuntimeOrigin::signed(ALICE), HDX));
		assert_eq!(PaymentPallet::account_currency(&ALICE), HDX);
	});
}

/// create a transaction info struct from weight. Handy to avoid building the whole struct.
pub fn info_from_weight(w: Weight) -> DispatchInfo {
	// pays_fee: Pays::Yes -- class: DispatchClass::Normal
	DispatchInfo {
		weight: w,
		..Default::default()
	}
}

fn post_info_from_weight(w: Weight) -> PostDispatchInfo {
	PostDispatchInfo {
		actual_weight: Some(w),
		pays_fee: Default::default(),
	}
}

fn default_post_info() -> PostDispatchInfo {
	PostDispatchInfo {
		actual_weight: None,
		pays_fee: Default::default(),
	}
}

#[test]
fn fee_should_be_transferred_when_paid_in_native_currency() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 100)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 0;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();
			// Assert
			assert_eq!(pre, (tip, CHARLIE, Some(PaymentInfo::Native(5 + 15 + 10))));

			assert_eq!(Balances::free_balance(CHARLIE), 100 - 30);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&default_post_info(),
				len,
				&Ok(())
			));
			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 100 - 30);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 30);
		});
}

#[test]
fn fee_should_be_withdrawn_when_paid_in_native_currency() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 100)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 0;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));
			let previous_total_issuance = Balances::total_issuance();

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 100 - 30);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&default_post_info(),
				len,
				&Ok(())
			));
			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 100 - 30);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 30);
			assert_eq!(Balances::total_issuance(), previous_total_issuance);
		});
}

#[test]
fn fee_should_be_transferred_when_paid_in_native_currency_work_with_tip() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 100)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 5;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));
			let post_dispatch_info = post_info_from_weight(Weight::from_parts(10, 0));

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();
			// Assert
			assert_eq!(pre, (tip, CHARLIE, Some(PaymentInfo::Native(5 + 15 + 10 + tip))));

			assert_eq!(Balances::free_balance(CHARLIE), 100 - 5 - 10 - 15 - tip);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&post_dispatch_info,
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 100 - 5 - 10 - 10 - tip);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 30);
		});
}

#[test]
fn fee_should_be_withdrawn_when_paid_in_native_currency_work_with_tip() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 100)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 5;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));
			let post_dispatch_info = post_info_from_weight(Weight::from_parts(10, 0));
			let previous_total_issuance = Balances::total_issuance();

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 100 - 5 - 10 - 15 - tip);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&post_dispatch_info,
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 100 - 5 - 10 - 10 - tip);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 30);
			assert_eq!(Balances::total_issuance(), previous_total_issuance);
		});
}

#[test]
fn fee_should_be_transferred_when_paid_in_non_native_currency() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10_000)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 0;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(
				pre,
				(
					tip,
					CHARLIE,
					Some(PaymentInfo::NonNative(45, SUPPORTED_CURRENCY, Price::from_float(1.5)))
				)
			);

			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 45);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 0);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&default_post_info(),
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 45);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 45);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);
		});
}

#[test]
fn fee_should_be_withdrawn_when_paid_in_non_native_currency() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10_000)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 0;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));
			let previous_total_issuance = Tokens::total_issuance(SUPPORTED_CURRENCY);

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 45);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 0);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&default_post_info(),
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 45);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 45);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);
			assert_eq!(Tokens::total_issuance(SUPPORTED_CURRENCY), previous_total_issuance);
		});
}

#[test]
fn fee_should_be_transferred_when_paid_in_non_native_currency_with_tip() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10_000)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 5;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));
			let post_dispatch_info = post_info_from_weight(Weight::from_parts(10, 0));

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(
				pre,
				(
					tip,
					CHARLIE,
					Some(PaymentInfo::NonNative(52, SUPPORTED_CURRENCY, Price::from_float(1.5)))
				)
			);

			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 52);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 0);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&post_dispatch_info,
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 45);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 45);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);
		});
}

#[test]
fn fee_should_be_withdrawn_and_not_refunded_when_paid_in_non_native_currency_with_tip() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10_000)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 5;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));
			let post_dispatch_info = post_info_from_weight(Weight::from_parts(10, 0));
			let previous_total_issuance = Tokens::total_issuance(SUPPORTED_CURRENCY);

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 52);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 0);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&post_dispatch_info,
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 10_000 - 45);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 45);
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);
			assert_eq!(Tokens::total_issuance(SUPPORTED_CURRENCY), previous_total_issuance);
		});
}

#[test]
fn fee_payment_in_native_currency_with_no_balance() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.build()
		.execute_with(|| {
			let len = 10;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Balances::free_balance(CHARLIE), 10);
			assert_eq!(Balances::free_balance(FeeReceiver::get()), 0);
		});
}

#[test]
fn fee_payment_in_non_native_currency_with_no_balance() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 100)
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.build()
		.execute_with(|| {
			let len = 1000;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 100);
			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &FeeReceiver::get()), 0);
		});
}

#[test]
fn fee_payment_in_non_native_currency_with_no_price() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10_000)
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.build()
		.execute_with(|| {
			// Make sure Charlie ain't got a penny!
			assert_eq!(Balances::free_balance(CHARLIE), 0);

			let len = 10;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &FEE_RECEIVER), 0);

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_ok());

			//Native balance check - Charlie should be still broke!
			assert_eq!(Balances::free_balance(CHARLIE), 0);

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 9970);
			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &FEE_RECEIVER), 0);
		});
}

#[test]
fn fee_payment_in_unregistered_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 100)
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.build()
		.execute_with(|| {
			let len = 1000;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert_ok!(PaymentPallet::remove_currency(
				RuntimeOrigin::root(),
				SUPPORTED_CURRENCY
			));

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 100);
		});
}

#[test]
fn fee_payment_non_native_insufficient_balance_with_no_pool() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 100)
		.with_currencies(vec![(CHARLIE, SUPPORTED_CURRENCY)])
		.build()
		.execute_with(|| {
			let len = 1000;
			let info = info_from_weight(Weight::from_parts(5, 0));

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 100);
		});
}

#[test]
fn fee_transfer_can_kill_account_when_paid_in_native() {
	// Arrange
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 30)
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 0;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(pre, (tip, CHARLIE, Some(PaymentInfo::Native(30))));
			assert_eq!(Balances::free_balance(CHARLIE), 0);
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&default_post_info(),
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Balances::free_balance(CHARLIE), 0); // zero balance indicates that the account can be killed
			assert_eq!(Balances::free_balance(FEE_RECEIVER), 30);
		});
}

#[test]
fn fee_transfer_can_kill_account_when_paid_in_non_native() {
	// Arrange
	ExtBuilder::default()
		.with_currencies(vec![(ALICE, SUPPORTED_CURRENCY)])
		.base_weight(5)
		.build()
		.execute_with(|| {
			let len = 10;
			let tip = 0;
			let dispatch_info = info_from_weight(Weight::from_parts(15, 0));

			assert_ok!(Currencies::withdraw(SUPPORTED_CURRENCY, &ALICE, INITIAL_BALANCE - 45));

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&ALICE, CALL, &dispatch_info, len)
				.unwrap();

			// Assert
			assert_eq!(
				pre,
				(
					tip,
					ALICE,
					Some(PaymentInfo::NonNative(45, SUPPORTED_CURRENCY, Price::from_float(1.5)))
				)
			);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &ALICE), 0);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FEE_RECEIVER), 0);

			// Act
			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&default_post_info(),
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &ALICE), 0); // zero balance indicates that the account can be killed
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &FEE_RECEIVER), 45);
		});
}

#[test]
fn set_and_remove_currency_on_lifecycle_callbacks() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10)
		.build()
		.execute_with(|| {
			assert_ok!(Tokens::transfer(Some(CHARLIE).into(), BOB, SUPPORTED_CURRENCY, 5));

			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &CHARLIE), 5);
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY, &BOB), 5);
			// Bob's fee currency was set on transfer (due to account creation)
			assert_eq!(PaymentPallet::get_currency(BOB), Some(SUPPORTED_CURRENCY));

			// currency should be removed if account is killed
			assert_ok!(Tokens::transfer_all(
				Some(BOB).into(),
				CHARLIE,
				SUPPORTED_CURRENCY,
				false
			));
			assert_eq!(PaymentPallet::get_currency(BOB), None);
		});
}

#[test]
fn currency_stays_around_until_reaping() {
	const CHARLIE: AccountId = 5;
	const DAVE: AccountId = 6;

	use frame_support::traits::fungibles::Balanced;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10)
		.build()
		.execute_with(|| {
			// setup
			assert_ok!(<Tokens as Balanced<AccountId>>::deposit(
				HIGH_ED_CURRENCY,
				&DAVE,
				HIGH_ED * 2,
				Precision::Exact
			)
			.map(|_| ()));
			assert_eq!(Currencies::free_balance(HIGH_ED_CURRENCY, &DAVE), HIGH_ED * 2);
			assert_eq!(PaymentPallet::get_currency(DAVE), Some(HIGH_ED_CURRENCY));

			// currency is not removed when account goes below existential deposit but stays around
			// until the account is reaped
			assert_ok!(Tokens::transfer(Some(DAVE).into(), BOB, HIGH_ED_CURRENCY, HIGH_ED + 1,));
			assert_eq!(PaymentPallet::get_currency(DAVE), Some(HIGH_ED_CURRENCY));
			assert_eq!(PaymentPallet::get_currency(BOB), Some(HIGH_ED_CURRENCY));

			// ... and account is reaped when all funds are transferred
			assert_ok!(Tokens::transfer_all(Some(DAVE).into(), BOB, HIGH_ED_CURRENCY, false));
			assert_eq!(PaymentPallet::get_currency(DAVE), None);
		});
}

#[test]
fn currency_is_removed_when_balance_hits_zero() {
	const CHARLIE: AccountId = 5;
	const DAVE: AccountId = 6;

	use frame_support::traits::fungibles::Balanced;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10)
		.build()
		.execute_with(|| {
			// setup
			assert_ok!(<Tokens as Balanced<AccountId>>::deposit(
				SUPPORTED_CURRENCY_WITH_PRICE,
				&DAVE,
				10,
				Precision::Exact
			)
			.map(|_| ()));
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY_WITH_PRICE, &DAVE), 10);
			assert_eq!(PaymentPallet::get_currency(DAVE), Some(SUPPORTED_CURRENCY_WITH_PRICE));

			// currency is removed when all funds of tx fee currency are transferred (even if
			// account still has other funds)
			assert_ok!(Tokens::transfer(Some(CHARLIE).into(), DAVE, SUPPORTED_CURRENCY, 2));
			assert_ok!(Tokens::transfer_all(
				Some(DAVE).into(),
				BOB,
				SUPPORTED_CURRENCY_WITH_PRICE,
				false
			));
			assert_eq!(PaymentPallet::get_currency(DAVE), None);
		});
}

#[test]
fn currency_is_not_changed_on_unrelated_account_activity() {
	const CHARLIE: AccountId = 5;
	const DAVE: AccountId = 6;

	use frame_support::traits::fungibles::Balanced;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY, 10)
		.build()
		.execute_with(|| {
			// setup
			assert_ok!(<Tokens as Balanced<AccountId>>::deposit(
				SUPPORTED_CURRENCY_WITH_PRICE,
				&DAVE,
				10,
				Precision::Exact
			)
			.map(|_| ()));
			assert_eq!(Currencies::free_balance(SUPPORTED_CURRENCY_WITH_PRICE, &DAVE), 10);
			assert_eq!(PaymentPallet::get_currency(DAVE), Some(SUPPORTED_CURRENCY_WITH_PRICE));

			// tx fee currency is not changed when a new currency is added to the account
			assert_ok!(Tokens::transfer(Some(CHARLIE).into(), DAVE, SUPPORTED_CURRENCY, 2));
			assert_eq!(PaymentPallet::get_currency(DAVE), Some(SUPPORTED_CURRENCY_WITH_PRICE));

			// tx fee currency is not removed when an unrelated account is removed
			assert_ok!(Tokens::transfer_all(
				Some(DAVE).into(),
				CHARLIE,
				SUPPORTED_CURRENCY,
				false
			));
			assert_eq!(PaymentPallet::get_currency(DAVE), Some(SUPPORTED_CURRENCY_WITH_PRICE));
		});
}

#[test]
fn only_set_fee_currency_for_supported_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.account_tokens(CHARLIE, UNSUPPORTED_CURRENCY, 10)
		.build()
		.execute_with(|| {
			assert_ok!(Tokens::transfer(Some(CHARLIE).into(), BOB, UNSUPPORTED_CURRENCY, 5));

			assert_eq!(Currencies::free_balance(UNSUPPORTED_CURRENCY, &CHARLIE), 5);
			assert_eq!(Currencies::free_balance(UNSUPPORTED_CURRENCY, &BOB), 5);
			// Bob's fee currency was not set on transfer (due to the currency being unsupported)
			assert_eq!(PaymentPallet::get_currency(BOB), None);
		});
}

#[test]
fn only_set_fee_currency_when_without_native_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 10)
		.build()
		.execute_with(|| {
			assert_eq!(PaymentPallet::get_currency(CHARLIE), None);

			assert_ok!(Currencies::transfer(
				Some(ALICE).into(),
				CHARLIE,
				SUPPORTED_CURRENCY,
				10,
			));

			assert_eq!(PaymentPallet::get_currency(CHARLIE), None);
		});
}

#[test]
fn do_not_set_fee_currency_for_new_native_account() {
	const CHARLIE: AccountId = 5;
	const DAVE: AccountId = 6;

	ExtBuilder::default()
		.account_native_balance(CHARLIE, 10)
		.build()
		.execute_with(|| {
			assert_eq!(PaymentPallet::get_currency(DAVE), None);

			assert_ok!(Currencies::transfer(Some(CHARLIE).into(), DAVE, 0, 10,));

			assert_eq!(PaymentPallet::get_currency(DAVE), None);
		});
}

#[test]
fn returns_prices_for_supported_currencies() {
	use hydradx_traits::NativePriceOracle;

	ExtBuilder::default().build().execute_with(|| {
		// returns constant price of 1 for native asset
		assert_eq!(PaymentPallet::price(HdxAssetId::get()), Some(1.into()));
		// returns default price configured at genesis
		assert_eq!(PaymentPallet::price(SUPPORTED_CURRENCY_NO_BALANCE), Some(1.into()));
		assert_eq!(PaymentPallet::price(SUPPORTED_CURRENCY), Some(Price::from_float(1.5)));
		assert_eq!(PaymentPallet::price(HIGH_ED_CURRENCY), Some(3.into()));
		// returns spot price
		assert_eq!(
			PaymentPallet::price(SUPPORTED_CURRENCY_WITH_PRICE),
			Some(Price::from_float(0.1))
		);
	});
}
