// This file is part of HydraDX.

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

use super::*;
use crate::mock::{expect_events, Currency, ExtBuilder, Faucet, RuntimeOrigin, System, Test, ALICE, HDX};
use frame_support::traits::OnFinalize;
use frame_support::{assert_noop, assert_ok};

#[test]
fn rampage_mints() {
	ExtBuilder::default().build_rampage().execute_with(|| {
		//Arrange
		System::set_block_number(1); //For event emitting

		//Act
		assert_ok!(Faucet::rampage_mint(RuntimeOrigin::signed(ALICE), HDX, 1000));

		//Assert
		assert_eq!(Currency::free_balance(HDX, &ALICE), 2000);
		expect_events(vec![Event::RampageMint {
			account_id: ALICE,
			asset_id: HDX,
			amount: 1000,
		}
		.into()]);
	});
}

#[test]
fn mints() {
	ExtBuilder::default().build_live().execute_with(|| {
		//Arrange
		System::set_block_number(1); //For event emitting

		assert_eq!(Currency::free_balance(2000, &ALICE), 0);

		//Act
		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));

		//Assert
		assert_eq!(Currency::free_balance(2000, &ALICE), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(3000, &ALICE), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(4000, &ALICE), 0);

		expect_events(vec![Event::Mint { account_id: ALICE }.into()]);
	});
}

#[test]
fn rampage_disabled() {
	ExtBuilder::default().build_live().execute_with(|| {
		assert_noop!(
			Faucet::rampage_mint(RuntimeOrigin::signed(ALICE), HDX, 1000),
			Error::<Test>::RampageMintNotAllowed
		);
	});
}

#[test]
fn mint_limit() {
	ExtBuilder::default().build_live().execute_with(|| {
		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));
		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));
		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));
		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));
		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));

		assert_noop!(
			Faucet::mint(RuntimeOrigin::signed(ALICE)),
			Error::<Test>::MaximumMintLimitReached
		);

		<Faucet as OnFinalize<u64>>::on_finalize(1);

		assert_ok!(Faucet::mint(RuntimeOrigin::signed(ALICE)));

		assert_eq!(Currency::free_balance(2000, &ALICE), 6_000_000_000_000_000);
	});
}
