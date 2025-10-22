// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

use crate::tests::mock::*;
use crate::types::{Intent as IntentType, IntentKind, SwapData, SwapType};
use crate::{Error, Event, Intents};
use frame_support::traits::Time;
use frame_support::{assert_noop, assert_ok};

#[test]
fn submit_intent_works() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(ALICE),
			intent.clone()
		));

		// Verify intent was stored
		let stored_intents: Vec<_> = Intents::<Test>::iter().collect();
		assert_eq!(stored_intents.len(), 1);

		// Verify event was emitted
		System::assert_has_event(RuntimeEvent::Intent(Event::IntentSubmitted::<Test>(
			stored_intents[0].0,
			intent,
		)));
	});
}

#[test]
fn submit_intent_fails_when_origin_mismatch() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		// BOB tries to submit ALICE's intent
		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(BOB), intent),
			Error::<Test>::InvalidIntent
		);
	});
}

#[test]
fn submit_intent_fails_with_past_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time - 1; // Past deadline

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidDeadline
		);
	});
}

#[test]
fn submit_intent_fails_with_deadline_equal_to_now() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time; // Deadline equals now (not greater than)

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidDeadline
		);
	});
}

#[test]
fn submit_intent_fails_with_deadline_too_far_in_future() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let max_duration = <Test as crate::Config>::MaxAllowedIntentDuration::get();
		let deadline = current_time + max_duration + 1; // One millisecond beyond max

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidDeadline
		);
	});
}

#[test]
fn submit_intent_fails_with_zero_amount_in() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 0, // Zero amount
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidIntent
		);
	});
}

#[test]
fn submit_intent_fails_with_zero_amount_out() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 0, // Zero amount
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidIntent
		);
	});
}

#[test]
fn submit_intent_fails_with_same_asset_in_and_out() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: DAI, // Same as asset_in
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidIntent
		);
	});
}

#[test]
fn submit_intent_fails_when_asset_out_is_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: HDX, // Hub asset
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_noop!(
			crate::Pallet::<Test>::submit_intent(RuntimeOrigin::signed(ALICE), intent),
			Error::<Test>::InvalidIntent
		);
	});
}

#[test]
fn submit_intent_works_with_callback_data() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;
		let on_success_data = vec![1, 2, 3, 4, 5];
		let on_failure_data = vec![6, 7, 8, 9, 10];

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: Some(on_success_data.clone().try_into().unwrap()),
			on_failure: Some(on_failure_data.clone().try_into().unwrap()),
		};

		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(ALICE),
			intent.clone()
		));

		// Verify intent was stored with callback data
		let stored_intents: Vec<_> = Intents::<Test>::iter().collect();
		assert_eq!(stored_intents.len(), 1);
		let (_, stored_intent) = &stored_intents[0];
		assert_eq!(stored_intent.on_success, intent.on_success);
		assert_eq!(stored_intent.on_failure, intent.on_failure);
	});
}

#[test]
fn submit_multiple_intents_works() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline1 = MockTimestampProvider::now() + 1000;
		let intent1 = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: deadline1,
			on_success: None,
			on_failure: None,
		};

		let deadline2 = MockTimestampProvider::now() + 2000;
		let intent2 = IntentType {
			who: BOB,
			kind: IntentKind::Swap(SwapData {
				asset_in: USDC,
				asset_out: DAI,
				amount_in: 50 * ONE,
				amount_out: 50 * ONE,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: deadline2,
			on_success: None,
			on_failure: None,
		};

		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(ALICE),
			intent1
		));
		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(BOB),
			intent2
		));

		// Verify both intents were stored
		let stored_intents: Vec<_> = Intents::<Test>::iter().collect();
		assert_eq!(stored_intents.len(), 2);
	});
}

#[test]
fn submit_intent_works_at_maximum_allowed_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let max_duration = <Test as crate::Config>::MaxAllowedIntentDuration::get();
		let deadline = current_time + max_duration; // Exactly at the limit

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		// This should fail because the deadline must be strictly less than (now + max_duration)
		assert_noop!(
			Intent::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()),
			Error::<Test>::InvalidDeadline
		);

		// But one millisecond less should work
		let mut intent_valid = intent;
		intent_valid.deadline = current_time + max_duration - 1;
		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(ALICE),
			intent_valid
		));
	});
}

#[test]
fn submit_intent_works_with_different_swap_types() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

		// Test ExactIn
		let intent_exact_in = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(ALICE),
			intent_exact_in
		));

		// Test ExactOut
		let intent_exact_out = IntentType {
			who: BOB,
			kind: IntentKind::Swap(SwapData {
				asset_in: USDC,
				asset_out: DAI,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_ok!(crate::Pallet::<Test>::submit_intent(
			RuntimeOrigin::signed(BOB),
			intent_exact_out
		));

		// Verify both were stored
		let stored_intents: Vec<_> = Intents::<Test>::iter().collect();
		assert_eq!(stored_intents.len(), 2);
	});
}
