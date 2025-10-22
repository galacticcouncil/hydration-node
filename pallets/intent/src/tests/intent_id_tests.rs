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
use crate::{NextIncrementalId, Pallet};
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
fn generate_new_intent_id_produces_unique_ids() {
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
fn generate_new_intent_id_encodes_deadline_in_upper_bits() {
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

		let intent_id = add_intent_tx(intent).unwrap();

		// Extract the deadline from the upper 64 bits
		let encoded_deadline = (intent_id >> 64) as u64;

		assert_eq!(encoded_deadline, deadline);
	});
}

#[test]
fn generate_new_intent_id_with_different_deadlines_produces_different_ids() {
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

		// Verify the deadline encoding
		let encoded_deadline1 = (id1 >> 64) as u64;
		let encoded_deadline2 = (id2 >> 64) as u64;

		assert_eq!(encoded_deadline1, deadline1);
		assert_eq!(encoded_deadline2, deadline2);
	});
}

#[test]
fn generate_new_intent_id_increments_sequential_counter() {
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

		// Extract the incremental ID from the lower 64 bits
		let incremental_id1 = id1 as u64;
		let incremental_id2 = id2 as u64;

		// Sequential IDs should increment by 1
		assert_eq!(incremental_id2, incremental_id1 + 1);
	});
}

#[test]
fn generate_new_intent_id_handles_sequential_id_overflow() {
	ExtBuilder::default().build().execute_with(|| {
		// Set the incremental ID to u64::MAX
		NextIncrementalId::<Test>::put(u64::MAX);

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

		// Add first intent with ID u64::MAX
		let id1 = add_intent_tx(intent1).unwrap();
		let incremental_id1 = id1 as u64;
		assert_eq!(incremental_id1, u64::MAX);

		// Add second intent, should overflow to 0
		let id2 = add_intent_tx(intent2).unwrap();
		let incremental_id2 = id2 as u64;
		assert_eq!(incremental_id2, 0);

		// Verify that the next incremental ID is 1
		assert_eq!(crate::Pallet::<Test>::next_incremental_id(), 1);
	});
}

#[test]
fn generate_new_intent_id_combines_deadline_and_sequential_id() {
	ExtBuilder::default().build().execute_with(|| {
		// Use a valid deadline that's in the future
		let deadline = MockTimestampProvider::now() + 5000;

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

		let intent_id = add_intent_tx(intent).unwrap();

		// Decode the intent ID
		let encoded_deadline = (intent_id >> 64) as u64;
		let incremental_id = intent_id as u64;

		// Verify the components
		assert_eq!(encoded_deadline, deadline);
		assert_eq!(incremental_id, 0); // First intent has incremental ID 0
	});
}

#[test]
fn generate_new_intent_id_with_max_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		// Use the maximum allowed deadline (current time + max duration - 1)
		let current_time = MockTimestampProvider::now();
		let max_duration = <Test as crate::Config>::MaxAllowedIntentDuration::get();
		let deadline = current_time + max_duration - 1;

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

		let intent_id = add_intent_tx(intent).unwrap();

		// Extract the deadline from the upper 64 bits
		let encoded_deadline = (intent_id >> 64) as u64;

		assert_eq!(encoded_deadline, deadline);
	});
}

#[test]
fn generate_new_intent_id_sequential_generation() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline = MockTimestampProvider::now() + 1000;

		let mut ids = Vec::new();

		// Generate 10 intents
		for i in 0..10 {
			let intent = IntentType {
				who: ALICE,
				kind: IntentKind::Swap(SwapData {
					asset_in: DAI,
					asset_out: USDC,
					amount_in: (100 + i) * ONE,
					amount_out: (100 + i) * ONE,
					swap_type: SwapType::ExactIn,
					partial: false,
				}),
				deadline,
				on_success: None,
				on_failure: None,
			};

			let id = add_intent_tx(intent).unwrap();
			ids.push(id);
		}

		// Verify all IDs are unique
		for i in 0..ids.len() {
			for j in (i + 1)..ids.len() {
				assert_ne!(ids[i], ids[j]);
			}
		}

		// Verify sequential increments in the lower 64 bits
		for i in 0..ids.len() - 1 {
			let incremental_id_i = ids[i] as u64;
			let incremental_id_next = ids[i + 1] as u64;
			assert_eq!(incremental_id_next, incremental_id_i + 1);
		}
	});
}

#[test]
fn generate_new_intent_id_with_minimal_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		// Use minimal valid deadline (current time + 1)
		let deadline = MockTimestampProvider::now() + 1;

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

		let intent_id = add_intent_tx(intent).unwrap();

		// Extract the deadline from the upper 64 bits
		let encoded_deadline = (intent_id >> 64) as u64;

		assert_eq!(encoded_deadline, deadline);
		assert_eq!(intent_id as u64, 0); // First intent has incremental ID 0
	});
}

#[test]
fn intent_id_format_allows_sorting_by_deadline() {
	ExtBuilder::default().build().execute_with(|| {
		let deadline1 = MockTimestampProvider::now() + 1000;
		let deadline2 = MockTimestampProvider::now() + 2000;
		let deadline3 = MockTimestampProvider::now() + 3000;

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

		let id1 = add_intent_tx(intent1).unwrap();
		let id2 = add_intent_tx(intent2).unwrap();
		let id3 = add_intent_tx(intent3).unwrap();

		// Since the deadline is in the upper 64 bits, sorting by intent_id
		// should sort by deadline first
		assert!(id1 < id2);
		assert!(id2 < id3);
	});
}
