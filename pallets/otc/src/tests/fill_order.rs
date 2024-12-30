// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate as otc;
use crate::tests::mock::*;
use crate::Event;
use frame_support::{assert_noop, assert_ok};
use orml_tokens::Error::BalanceTooLow;
use orml_traits::{MultiCurrency, NamedMultiReservableCurrency};
use pallet_support::types::{Asset, Fee};
use pretty_assertions::assert_eq;

#[test]
fn complete_fill_order_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let amount_in = 20 * ONE;
		let amount_out = 100 * ONE;
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			amount_in,
			amount_out,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		assert_ok!(OTC::fill_order(RuntimeOrigin::signed(BOB), 0));

		// Assert
		assert!(OTC::orders(0).is_none());

		let fee = OTC::calculate_fee(amount_out);

		let alice_free_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		// Alice: HDX *free* balance remains the same, reserved balance decreases with amount_receive; DAI grows
		assert_eq!(alice_free_hdx_balance_after, alice_free_hdx_balance_before);
		assert_eq!(Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE), 0);
		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_in);

		// Bob: HDX grows, DAI decreases
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + 100 * ONE - fee);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_in);

		// fee should be transferred to Treasury
		assert_eq!(
			Tokens::free_balance(HDX, &TreasuryAccount::get()),
			TREASURY_INITIAL_BALANCE + fee
		);

		expect_events(vec![
			Event::Filled {
				order_id: 0,
				who: BOB,
				amount_in: 20 * ONE,
				amount_out: 100 * ONE,
				fee: ONE,
			}
			.into(),
			pallet_support::Event::Swapped {
				swapper: BOB,
				filler: ALICE,
				filler_type:pallet_support::types::Filler::OTC(0),
				operation:pallet_support::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(DAI, 20 * ONE)],
				outputs: vec![Asset::new(HDX, 100 * ONE)],
				fees: vec![Fee::new(HDX, ONE, <Test as crate::Config>::FeeReceiver::get())],
				operation_stack: vec![],
			}
			.into(),
		]);
	});
}

#[test]
fn complete_fill_order_should_work_when_order_is_not_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let amount_in = 20 * ONE;
		let amount_out = 100 * ONE;
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			amount_in,
			amount_out,
			false
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		assert_ok!(OTC::fill_order(RuntimeOrigin::signed(BOB), 0));

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_none());

		let fee = OTC::calculate_fee(amount_out);

		let alice_free_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		// Alice: HDX *free* balance remains the same, reserved balance decreases with amount_receive; DAI grows
		assert_eq!(alice_free_hdx_balance_after, alice_free_hdx_balance_before);
		assert_eq!(Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE), 0);
		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_in);

		// Bob: HDX grows, DAI decreases
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + 100 * ONE - fee);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_in);

		// fee should be transferred to Treasury
		assert_eq!(
			Tokens::free_balance(HDX, &TreasuryAccount::get()),
			TREASURY_INITIAL_BALANCE + fee
		);

		let order_id = 0;
		expect_events(vec![
			Event::Filled {
				order_id,
				who: BOB,
				amount_in: 20 * ONE,
				amount_out: 100 * ONE,
				fee: ONE,
			}
			.into(),
			pallet_support::Event::Swapped {
				swapper: BOB,
				filler: ALICE,
				filler_type:pallet_support::types::Filler::OTC(order_id),
				operation:pallet_support::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(DAI, 20 * ONE)],
				outputs: vec![Asset::new(HDX, 100 * ONE)],
				fees: vec![Fee::new(HDX, ONE, <Test as crate::Config>::FeeReceiver::get())],
				operation_stack: vec![],
			}
			.into(),
		]);
	});
}

#[test]
fn complete_fill_order_should_work_when_there_are_multiple_orders() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let amount_in = 20 * ONE;
		let amount_out = 100 * ONE;
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			amount_in,
			amount_out,
			true
		));

		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			10 * ONE,
			50 * ONE,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		assert_ok!(OTC::fill_order(RuntimeOrigin::signed(BOB), 0));

		// Assert
		let order = OTC::orders(0);
		assert!(order.is_none());

		let fee = OTC::calculate_fee(amount_out);

		let alice_free_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		// Alice: HDX *free* balance remains the same, reserved balance decreases with amount_receive; DAI grows
		assert_eq!(alice_free_hdx_balance_after, alice_free_hdx_balance_before);
		assert_eq!(
			Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE),
			50 * ONE
		);
		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount_in);

		// Bob: HDX grows, DAI decreases
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before + 100 * ONE - fee);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount_in);

		// fee should be transferred to Treasury
		assert_eq!(
			Tokens::free_balance(HDX, &TreasuryAccount::get()),
			TREASURY_INITIAL_BALANCE + fee
		);

		let order_id = 0;
		expect_events(vec![
			Event::Filled {
				order_id: 0,
				who: BOB,
				amount_in: 20 * ONE,
				amount_out: 100 * ONE,
				fee: ONE,
			}
			.into(),
			pallet_support::Event::Swapped {
				swapper: BOB,
				filler: ALICE,
				filler_type:pallet_support::types::Filler::OTC(order_id),
				operation:pallet_support::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(DAI, 20 * ONE)],
				outputs: vec![Asset::new(HDX, 100 * ONE)],
				fees: vec![Fee::new(HDX, ONE, <Test as crate::Config>::FeeReceiver::get())],
				operation_stack: vec![],
			}
			.into(),
		]);
	});
}

#[test]
fn fill_order_should_throw_error_when_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			200 * ONE,
			100 * ONE,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_before = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		assert_noop!(OTC::fill_order(RuntimeOrigin::signed(BOB), 0), BalanceTooLow::<Test>);

		// Assert
		let alice_free_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_after = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		// Alice: nothing changes
		assert_eq!(alice_free_hdx_balance_after, alice_free_hdx_balance_before);
		assert_eq!(alice_reserved_hdx_balance_after, alice_reserved_hdx_balance_before);
		assert_eq!(alice_dai_balance_after, alice_dai_balance_before);

		// Bob: nothing changes
		assert_eq!(bob_hdx_balance_after, bob_hdx_balance_before);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before);
	});
}
