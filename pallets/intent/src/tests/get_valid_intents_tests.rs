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
use crate::Pallet;
use frame_support::storage::{with_transaction, TransactionOutcome};
use frame_support::traits::Time;

/// Helper function to call add_intent within a transaction
fn add_intent_tx(intent: IntentType<AccountId>) -> Result<u128, frame_support::sp_runtime::DispatchError> {
	with_transaction(|| {
		let result = Pallet::<Test>::add_intent(intent);
		TransactionOutcome::Commit(result)
	})
}

#[test]
fn get_valid_intents_returns_empty_when_no_intents() {
	ExtBuilder::default().build().execute_with(|| {
		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 0);
	});
}

#[test]
fn get_valid_intents_returns_all_valid_intents() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline1 = current_time + 1000;
		let deadline2 = current_time + 2000;
		let deadline3 = current_time + 3000;

		// Add three valid intents
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
			who: BOB,
			kind: IntentKind::Swap(SwapData {
				asset_in: USDC,
				asset_out: DAI,
				amount_in: 50 * ONE,
				amount_out: 50 * ONE,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline: deadline2,
			on_success: None,
			on_failure: None,
		};

		let intent3 = IntentType {
			who: CHARLIE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 25 * ONE,
				amount_out: 25 * ONE,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: deadline3,
			on_success: None,
			on_failure: None,
		};

		add_intent_tx(intent1).unwrap();
		add_intent_tx(intent2).unwrap();
		add_intent_tx(intent3).unwrap();

		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 3);
	});
}

#[test]
fn get_valid_intents_filters_expired_intents() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline1 = current_time + 1000;
		let deadline2 = current_time + 2000;

		// Add two intents
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
			who: BOB,
			kind: IntentKind::Swap(SwapData {
				asset_in: USDC,
				asset_out: DAI,
				amount_in: 50 * ONE,
				amount_out: 50 * ONE,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline: deadline2,
			on_success: None,
			on_failure: None,
		};

		add_intent_tx(intent1).unwrap();
		add_intent_tx(intent2).unwrap();

		// Advance time to make the first intent expire
		advance_time(1500);

		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 1);

		// Verify it's the second intent
		let (_, stored_intent) = &valid_intents[0];
		assert_eq!(stored_intent.who, BOB);
	});
}

#[test]
fn get_valid_intents_returns_sorted_by_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();

		// Add intents in non-sequential order
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
			deadline: current_time + 3000,
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
			deadline: current_time + 1000,
			on_success: None,
			on_failure: None,
		};

		let intent3 = IntentType {
			who: CHARLIE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 25 * ONE,
				amount_out: 25 * ONE,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: current_time + 2000,
			on_success: None,
			on_failure: None,
		};

		add_intent_tx(intent1).unwrap();
		add_intent_tx(intent2).unwrap();
		add_intent_tx(intent3).unwrap();

		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 3);

		// Verify they are sorted by deadline (ascending)
		assert_eq!(valid_intents[0].1.who, BOB); // deadline: current_time + 1000
		assert_eq!(valid_intents[1].1.who, CHARLIE); // deadline: current_time + 2000
		assert_eq!(valid_intents[2].1.who, ALICE); // deadline: current_time + 3000
	});
}

#[test]
fn get_valid_intents_excludes_intent_with_deadline_equal_to_now() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();

		// Add intent with deadline slightly in the future
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
			deadline: current_time + 100,
			on_success: None,
			on_failure: None,
		};

		add_intent_tx(intent).unwrap();

		// Initially, the intent should be valid
		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 1);

		// Advance time to exactly the deadline
		advance_time(100);

		// Now the intent should not be valid (deadline must be > now)
		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 0);
	});
}

#[test]
fn get_valid_intents_works_when_all_intents_expired() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time + 1000;

		// Add an intent
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

		add_intent_tx(intent).unwrap();

		// Advance time past the deadline
		advance_time(1500);

		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 0);
	});
}

#[test]
fn get_valid_intents_handles_multiple_intents_with_same_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time + 1000;

		// Add multiple intents with the same deadline
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

		let intent3 = IntentType {
			who: CHARLIE,
			kind: IntentKind::Swap(SwapData {
				asset_in: DAI,
				asset_out: USDC,
				amount_in: 25 * ONE,
				amount_out: 25 * ONE,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline,
			on_success: None,
			on_failure: None,
		};

		add_intent_tx(intent1).unwrap();
		add_intent_tx(intent2).unwrap();
		add_intent_tx(intent3).unwrap();

		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 3);

		// All should have the same deadline
		for (_, intent) in valid_intents {
			assert_eq!(intent.deadline, deadline);
		}
	});
}

#[test]
fn get_valid_intents_includes_intent_expiring_in_one_millisecond() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time + 1;

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

		add_intent_tx(intent).unwrap();

		// Should be valid since deadline > now
		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 1);
	});
}

#[test]
fn get_valid_intents_maintains_consistency_across_multiple_calls() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time + 1000;

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

		add_intent_tx(intent).unwrap();

		// Call get_valid_intents multiple times
		let valid_intents1 = Pallet::<Test>::get_valid_intents();
		let valid_intents2 = Pallet::<Test>::get_valid_intents();
		let valid_intents3 = Pallet::<Test>::get_valid_intents();

		// Results should be consistent
		assert_eq!(valid_intents1.len(), valid_intents2.len());
		assert_eq!(valid_intents2.len(), valid_intents3.len());
		assert_eq!(valid_intents1[0].0, valid_intents2[0].0);
		assert_eq!(valid_intents2[0].0, valid_intents3[0].0);
	});
}

#[test]
fn get_valid_intents_preserves_intent_data() {
	ExtBuilder::default().build().execute_with(|| {
		let current_time = MockTimestampProvider::now();
		let deadline = current_time + 1000;
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
				partial: true,
			}),
			deadline,
			on_success: Some(on_success_data.clone().try_into().unwrap()),
			on_failure: Some(on_failure_data.clone().try_into().unwrap()),
		};

		add_intent_tx(intent.clone()).unwrap();

		let valid_intents = Pallet::<Test>::get_valid_intents();
		assert_eq!(valid_intents.len(), 1);

		let (_, stored_intent) = &valid_intents[0];
		assert_eq!(stored_intent.who, intent.who);
		assert_eq!(stored_intent.deadline, intent.deadline);
		assert_eq!(stored_intent.on_success, intent.on_success);
		assert_eq!(stored_intent.on_failure, intent.on_failure);

		if let IntentKind::Swap(stored_swap) = &stored_intent.kind {
			if let IntentKind::Swap(original_swap) = &intent.kind {
				assert_eq!(stored_swap.asset_in, original_swap.asset_in);
				assert_eq!(stored_swap.asset_out, original_swap.asset_out);
				assert_eq!(stored_swap.amount_in, original_swap.amount_in);
				assert_eq!(stored_swap.amount_out, original_swap.amount_out);
				assert_eq!(stored_swap.partial, original_swap.partial);
			}
		}
	});
}
