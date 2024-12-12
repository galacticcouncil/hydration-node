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
use crate::types::*;
use crate::Event;
#[test]
fn event_id_should_be_incremented() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(AmmSupport::incremental_id(), 0);
		assert_eq!(AmmSupport::next_incremental_id().unwrap(), 0);

		assert_eq!(AmmSupport::incremental_id(), 1);
		assert_eq!(AmmSupport::next_incremental_id().unwrap(), 1);

		assert_eq!(AmmSupport::incremental_id(), 2);
		assert_eq!(AmmSupport::next_incremental_id().unwrap(), 2);
	});
}

#[test]
fn stack_should_be_populated_when_pushed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AmmSupport::push(ExecutionType::Router(1)));
		assert_eq!(AmmSupport::get(), vec![ExecutionType::Router(1)]);
		assert_eq!(AmmSupport::id_stack().0.into_inner(), vec![ExecutionType::Router(1)]);

		assert_ok!(AmmSupport::push(ExecutionType::Router(2)));
		assert_eq!(
			AmmSupport::get(),
			vec![ExecutionType::Router(1), ExecutionType::Router(2)]
		);
		assert_eq!(
			AmmSupport::id_stack().0.into_inner(),
			vec![ExecutionType::Router(1), ExecutionType::Router(2)]
		);

		assert_ok!(AmmSupport::push(ExecutionType::ICE(3)));
		assert_eq!(
			AmmSupport::get(),
			vec![
				ExecutionType::Router(1),
				ExecutionType::Router(2),
				ExecutionType::ICE(3)
			]
		);
		assert_eq!(
			AmmSupport::id_stack().0.into_inner(),
			vec![
				ExecutionType::Router(1),
				ExecutionType::Router(2),
				ExecutionType::ICE(3)
			]
		);
	});
}

#[test]
fn stack_should_not_panic_when_full() {
	ExtBuilder::default().build().execute_with(|| {
		for id in 0..MAX_STACK_SIZE {
			assert_ok!(AmmSupport::push(ExecutionType::Router(id)));
		}

		assert_err!(
			AmmSupport::push(ExecutionType::Router(MAX_STACK_SIZE)),
			Error::<Test>::MaxStackSizeReached
		);
	});
}

#[test]
fn stack_should_be_reduced_when_poped() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AmmSupport::push(ExecutionType::Router(1)));
		assert_ok!(AmmSupport::push(ExecutionType::Router(2)));
		assert_ok!(AmmSupport::push(ExecutionType::ICE(3)));

		assert_ok!(AmmSupport::pop(), ExecutionType::ICE(3));
		assert_eq!(
			AmmSupport::get(),
			vec![ExecutionType::Router(1), ExecutionType::Router(2)]
		);
		assert_eq!(
			AmmSupport::id_stack().0.into_inner(),
			vec![ExecutionType::Router(1), ExecutionType::Router(2)]
		);

		assert_ok!(AmmSupport::push(ExecutionType::ICE(3)));
		assert_eq!(
			AmmSupport::get(),
			vec![
				ExecutionType::Router(1),
				ExecutionType::Router(2),
				ExecutionType::ICE(3)
			]
		);
		assert_eq!(
			AmmSupport::id_stack().0.into_inner(),
			vec![
				ExecutionType::Router(1),
				ExecutionType::Router(2),
				ExecutionType::ICE(3)
			]
		);
	});
}

#[test]
fn pop_from_empty_stack_should_not_panic() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(AmmSupport::pop(), Error::<Test>::EmptyStack);
	});
}

#[test]
fn event_should_be_deposited() {
	ExtBuilder::default().build().execute_with(|| {
		AmmSupport::deposit_trade_event(
			ALICE,
			BOB,
			Filler::Omnipool,
			TradeOperation::ExactIn,
			vec![(AssetType::Fungible(HDX), 1_000_000)],
			vec![(AssetType::Fungible(DOT), 2_000_000)],
			vec![Fee::new(HDX, 1_000, ALICE), Fee::new(DOT, 2_000, BOB)],
		);

		expect_events(vec![Event::Swapped {
			swapper: ALICE,
			filler: BOB,
			filler_type: Filler::Omnipool,
			operation: TradeOperation::ExactIn,
			inputs: vec![(AssetType::Fungible(HDX), 1_000_000)],
			outputs: vec![(AssetType::Fungible(DOT), 2_000_000)],
			fees: vec![Fee::new(HDX, 1_000, ALICE), Fee::new(DOT, 2_000, BOB)],
			operation_id: vec![],
		}
		.into()]);
	});
}
