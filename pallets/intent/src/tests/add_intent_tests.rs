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
use crate::{Error, Intents, Pallet};
use frame_support::storage::{with_transaction, TransactionOutcome};
use frame_support::traits::Time;
use frame_support::{assert_err, assert_ok};

/// Helper function to call add_intent within a transaction
fn add_intent_tx(intent: IntentType<AccountId>) -> Result<u128, frame_support::sp_runtime::DispatchError> {
	with_transaction(|| {
		let result = Pallet::<Test>::add_intent(intent);
		TransactionOutcome::Commit(result)
	})
}

#[test]
fn add_intent_works() {
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

		let result = add_intent_tx(intent.clone());
		assert_ok!(result);

		let intent_id = result.unwrap();

		// Verify intent was stored
		let stored_intent = Intents::<Test>::get(intent_id);
		assert!(stored_intent.is_some());
		assert_eq!(stored_intent.unwrap(), intent);
	});
}

#[test]
fn add_intent_returns_unique_ids() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

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
			deadline,
			on_success: None,
			on_failure: None,
		};

		let intent2 = IntentType {
			who: BOB,
			kind: IntentKind::Swap(SwapData {
				asset_in: USDC,
				asset_out: DAI,
				amount_in: 50 * ONE,
				amount_out: 50 * ONE,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		let id1 = add_intent_tx(intent1).unwrap();
		let id2 = add_intent_tx(intent2).unwrap();

		// IDs should be unique
		assert_ne!(id1, id2);
	});
}

#[test]
fn add_intent_fails_with_deadline_in_past() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time - 1;

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

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidDeadline);
	});
}

#[test]
fn add_intent_fails_with_deadline_equal_to_now() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();

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
			deadline: current_time,
			on_success: None,
			on_failure: None,
		};

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidDeadline);
	});
}

#[test]
fn add_intent_fails_with_deadline_too_far() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let max_duration = <Test as crate::Config>::MaxAllowedIntentDuration::get();
		let deadline = current_time + max_duration + 1;

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

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidDeadline);
	});
}

#[test]
fn add_intent_fails_with_zero_amount_in() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 0,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidIntent);
	});
}

#[test]
fn add_intent_fails_with_zero_amount_out() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 0,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidIntent);
	});
}

#[test]
fn add_intent_fails_with_same_asset_in_and_out() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: DAI,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidIntent);
	});
}

#[test]
fn add_intent_fails_when_asset_out_is_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

		let intent = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: HDX,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		assert_err!(add_intent_tx(intent), Error::<Test>::InvalidIntent);
	});
}

#[test]
fn add_intent_with_different_deadlines_produces_different_ids() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline1 = MockTimestampProvider::now() + 1000;
		let deadline2 = MockTimestampProvider::now() + 2000;

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

		let intent2 = IntentType {
			who: ALICE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 100 * ONE,
				amount_out: 100 * ONE,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: deadline2,
			on_success: None,
			on_failure: None,
		};

		let id1 = add_intent_tx(intent1).unwrap();
		let id2 = add_intent_tx(intent2).unwrap();

		// IDs should be different due to different deadlines
		assert_ne!(id1, id2);
	});
}

#[test]
fn add_intent_stores_callback_data_correctly() {
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

		let intent_id = add_intent_tx(intent.clone()).unwrap();

		// Verify callback data was stored
		let stored_intent = Intents::<Test>::get(intent_id).unwrap();
		assert_eq!(stored_intent.on_success, intent.on_success);
		assert_eq!(stored_intent.on_failure, intent.on_failure);
	});
}

#[test]
fn add_intent_with_partial_flag_works() {
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
				partial: true,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		let intent_id = add_intent_tx(intent.clone()).unwrap();

		let stored_intent = Intents::<Test>::get(intent_id).unwrap();
		if let IntentKind::Swap(swap_data) = stored_intent.kind {
			assert!(swap_data.partial);
		} else {
			panic!("Expected Swap intent kind");
		}
	});
}

#[test]
fn add_intent_works_at_boundary_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let max_duration = <Test as crate::Config>::MaxAllowedIntentDuration::get();
		let deadline = current_time + max_duration - 1; // Just within the limit

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

		assert_ok!(add_intent_tx(intent));
	});
}

#[test]
fn add_intent_increments_next_id() {
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

		let initial_next_id = crate::Pallet::<Test>::next_incremental_id();

		add_intent_tx(intent.clone()).unwrap();

		let new_next_id = crate::Pallet::<Test>::next_incremental_id();

		assert_eq!(new_next_id, initial_next_id + 1);
	});
}
