// This file is part of HydraDX-node.

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
use crate::Event;
use frame_support::assert_err;

#[test]
fn stack_should_be_populated_when_pushed() {
	ExtBuilder::default().build().execute_with(|| {
		Broadcast::add_to_context(ExecutionType::Router).unwrap();
		assert_eq!(Broadcast::execution_context(), vec![ExecutionType::Router(0)]);
		assert_eq!(
			Broadcast::execution_context().into_inner(),
			vec![ExecutionType::Router(0)]
		);

		Broadcast::add_to_context(ExecutionType::Router).unwrap();
		assert_eq!(
			Broadcast::execution_context(),
			vec![ExecutionType::Router(0), ExecutionType::Router(1)]
		);
		assert_eq!(
			Broadcast::execution_context().into_inner(),
			vec![ExecutionType::Router(0), ExecutionType::Router(1)]
		);

		Broadcast::add_to_context(ExecutionType::Omnipool).unwrap();
		assert_eq!(
			Broadcast::execution_context(),
			vec![
				ExecutionType::Router(0),
				ExecutionType::Router(1),
				ExecutionType::Omnipool(2)
			]
		);
		assert_eq!(
			Broadcast::execution_context().into_inner(),
			vec![
				ExecutionType::Router(0),
				ExecutionType::Router(1),
				ExecutionType::Omnipool(2)
			]
		);
	});
}

#[test]
fn stack_should_be_reduced_when_poped() {
	ExtBuilder::default().build().execute_with(|| {
		Broadcast::add_to_context(ExecutionType::Router).unwrap();
		Broadcast::add_to_context(ExecutionType::Router).unwrap();
		Broadcast::add_to_context(ExecutionType::Omnipool).unwrap();

		Broadcast::reset_context().unwrap();
		assert_eq!(
			Broadcast::execution_context(),
			vec![ExecutionType::Router(0), ExecutionType::Router(1)]
		);
		assert_eq!(
			Broadcast::execution_context().into_inner(),
			vec![ExecutionType::Router(0), ExecutionType::Router(1)]
		);

		Broadcast::add_to_context(ExecutionType::Omnipool).unwrap();
		assert_eq!(
			Broadcast::execution_context(),
			vec![
				ExecutionType::Router(0),
				ExecutionType::Router(1),
				ExecutionType::Omnipool(3)
			]
		);
		assert_eq!(
			Broadcast::execution_context().into_inner(),
			vec![
				ExecutionType::Router(0),
				ExecutionType::Router(1),
				ExecutionType::Omnipool(3)
			]
		);
	});
}

#[test]
fn event_should_be_deposited() {
	ExtBuilder::default().build().execute_with(|| {
		Broadcast::deposit_trade_event(
			ALICE,
			BOB,
			Filler::Omnipool,
			TradeOperation::ExactIn,
			vec![Asset::new(HDX, 1_000_000)],
			vec![Asset::new(DOT, 2_000_000)],
			vec![
				Fee::new(HDX, 1_000, Destination::Account(ALICE)),
				Fee::new(DOT, 2_000, Destination::Account(BOB)),
			],
		);

		expect_events(vec![Event::Swapped {
			swapper: ALICE,
			filler: BOB,
			filler_type: Filler::Omnipool,
			operation: TradeOperation::ExactIn,
			inputs: vec![Asset::new(HDX, 1_000_000)],
			outputs: vec![Asset::new(DOT, 2_000_000)],
			fees: vec![
				Fee::new(HDX, 1_000, Destination::Account(ALICE)),
				Fee::new(DOT, 2_000, Destination::Account(BOB)),
			],
			operation_stack: vec![],
		}
		.into()]);
	});
}

#[test]
fn entry_is_removed_when_type_matched_with_last_stack_item() {
	ExtBuilder::default().build().execute_with(|| {
		Broadcast::add_to_context(ExecutionType::Router).unwrap();

		Broadcast::reset_context().unwrap();

		assert_eq!(Broadcast::execution_context().into_inner(), vec![]);
	});
}

#[test]
fn should_return_error_when_stack_reaches_max_size() {
	ExtBuilder::default().build().execute_with(|| {
		for _ in 0..MAX_STACK_SIZE {
			Broadcast::add_to_context(ExecutionType::Batch).unwrap();
		}

		assert_err!(
			Broadcast::add_to_context(ExecutionType::Batch),
			Error::<Test>::ExecutionCallStackOverflow
		);
	});
}

#[test]
fn should_return_error_when_trying_to_remove_from_empty_stack() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			Broadcast::reset_context(),
			Error::<Test>::ExecutionCallStackUnderflow
		);
	});
}
