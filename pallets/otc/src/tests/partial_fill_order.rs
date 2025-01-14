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
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use orml_tokens::Error::BalanceTooLow;
use orml_traits::{MultiCurrency, NamedMultiReservableCurrency};
use pallet_broadcast::types::{Asset, Destination, Fee};
use pretty_assertions::assert_eq;

#[test]
fn partial_fill_order_should_work_when_order_is_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_before = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount = 5 * ONE;
		assert_ok!(OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount));

		// Assert
		let expected_amount_out = 25_000_000_000_000_u128;
		let expected_new_amount_in = 15_000_000_000_000_u128;
		let expected_new_amount_out = 75_000_000_000_000_u128;
		let fee = OTC::calculate_fee(expected_amount_out);
		assert_eq!(fee, 25 * ONE / 100);

		let alice_free_hdx_balance_after = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_after = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_after = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_after = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_after = Tokens::free_balance(DAI, &BOB);

		// Alice: HDX *free* balance remains the same, reserved balance decreases with amount_receive; DAI grows
		assert_eq!(alice_free_hdx_balance_after, alice_free_hdx_balance_before);
		assert_eq!(
			alice_reserved_hdx_balance_after,
			alice_reserved_hdx_balance_before - expected_amount_out
		);
		assert_eq!(alice_dai_balance_after, alice_dai_balance_before + amount);

		// Bob: HDX grows, DAI decreases
		assert_eq!(
			bob_hdx_balance_after,
			bob_hdx_balance_before + expected_amount_out - fee
		);
		assert_eq!(bob_dai_balance_after, bob_dai_balance_before - amount);

		let order = OTC::orders(0).unwrap();
		assert_eq!(order.amount_in, expected_new_amount_in);
		assert_eq!(order.amount_out, expected_new_amount_out);

		// fee should be transferred to Treasury
		assert_eq!(
			Tokens::free_balance(HDX, &TreasuryAccount::get()),
			TREASURY_INITIAL_BALANCE + fee
		);

		let order_id = 0;
		expect_events(vec![
			Event::PartiallyFilled {
				order_id,
				who: BOB,
				amount_in: 5 * ONE,
				amount_out: expected_amount_out,
				fee,
			}
			.into(),
			pallet_broadcast::Event::Swapped {
				swapper: order.owner,
				filler: BOB,
				filler_type: pallet_broadcast::types::Filler::OTC(order_id),
				operation: pallet_broadcast::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(order.asset_in, 5 * ONE)],
				outputs: vec![Asset::new(order.asset_out, expected_amount_out)],
				fees: vec![Fee::new(
					order.asset_out,
					fee,
					Destination::Account(<Test as crate::Config>::FeeReceiver::get()),
				)],
				operation_stack: vec![],
			}
			.into(),
		]);
	});
}

#[test]
fn partial_fill_order_should_throw_error_when_order_is_not_partially_fillable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			false
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_before = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount = 5 * ONE;
		assert_noop!(
			OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount),
			Error::<Test>::OrderNotPartiallyFillable
		);

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

#[test]
fn partial_fill_order_should_throw_error_when_fill_is_complete() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_before = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount = 20 * ONE;
		assert_noop!(
			OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount),
			Error::<Test>::OrderAmountTooSmall
		);

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

#[test]
fn partial_fill_order_should_throw_error_when_remaining_amounts_are_too_low() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_before = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount = 20 * ONE - (ONE / 100);
		assert_noop!(
			OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount),
			Error::<Test>::OrderAmountTooSmall
		);

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
		let amount = 110 * ONE;
		assert_noop!(
			OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount),
			BalanceTooLow::<Test>
		);

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

#[test]
fn partial_fill_order_should_throw_error_when_amount_is_larger_than_order() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			20 * ONE,
			100 * ONE,
			true
		));

		let alice_free_hdx_balance_before = Tokens::free_balance(HDX, &ALICE);
		let alice_reserved_hdx_balance_before = Tokens::reserved_balance_named(&otc::NAMED_RESERVE_ID, HDX, &ALICE);
		let bob_hdx_balance_before = Tokens::free_balance(HDX, &BOB);

		let alice_dai_balance_before = Tokens::free_balance(DAI, &ALICE);
		let bob_dai_balance_before = Tokens::free_balance(DAI, &BOB);

		// Act
		let amount = 30 * ONE;
		assert_noop!(
			OTC::partial_fill_order(RuntimeOrigin::signed(BOB), 0, amount),
			Error::<Test>::MathError
		);

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
