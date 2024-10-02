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
use hydradx_traits::router::{Filler, TradeOperation};

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
fn event_should_be_deposited() {
	ExtBuilder::default().build().execute_with(|| {
		AmmSupport::deposit_trade_event(
			ALICE,
			BOB,
			Filler::Omnipool,
			TradeOperation::Sell,
			HDX,
			DOT,
			1_000_000,
			2_000_000,
			vec![(HDX, 1_000, ALICE), (DOT, 2_000, BOB)],
			Some(7),
		);

		expect_events(vec![Event::Swapped {
			swapper: ALICE,
			filler: BOB,
			filler_type: Filler::Omnipool,
			operation: TradeOperation::Sell,
			asset_in: HDX,
			asset_out: DOT,
			amount_in: 1_000_000,
			amount_out: 2_000_000,
			fees: vec![(HDX, 1_000, ALICE), (DOT, 2_000, BOB)],
			event_id: Some(7),
		}
		.into()]);
	});
}
