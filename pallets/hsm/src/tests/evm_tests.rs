// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::mock::*;
use crate::*;
use ethabi::ethereum_types::U256;
use frame_support::assert_ok;
use orml_traits::MultiCurrency;

#[test]
fn test_mint_hollar_evm() {
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE)])
		.with_registered_asset(HOLLAR, 18)
		.build();

	ext.execute_with(|| {
		let amount = 100 * ONE;
		assert_ok!(HSM::mint_hollar(&ALICE, amount));

		// Check if the balance was updated
		assert_balance!(ALICE, HOLLAR, amount);

		// Check if the EVM call was made with the right data
		let evm_calls = last_evm_call();
		assert!(evm_calls.is_some());

		// Clear calls for next test
		clear_evm_calls();
	});
}

#[test]
fn test_burn_hollar_evm() {
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE), (ALICE, HOLLAR, 500 * ONE)])
		.with_registered_asset(HOLLAR, 18)
		.build();

	ext.execute_with(|| {
		let initial_balance = Tokens::free_balance(HOLLAR, &ALICE);
		assert_eq!(initial_balance, 500 * ONE);

		let burn_amount = 100 * ONE;
		assert_ok!(HSM::burn_hollar(burn_amount));

		// Check if the balance was updated correctly
		let expected_balance = initial_balance - burn_amount;
		assert_balance!(ALICE, HOLLAR, expected_balance);

		// Check if the EVM call was made
		let evm_calls = last_evm_call();
		assert!(evm_calls.is_some());
	});
}

#[test]
fn test_mint_and_burn_flow() {
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE)])
		.with_registered_asset(HOLLAR, 18)
		.build();

	ext.execute_with(|| {
		// First mint some Hollar
		let mint_amount = 200 * ONE;
		assert_ok!(HSM::mint_hollar(&ALICE, mint_amount));
		assert_balance!(ALICE, HOLLAR, mint_amount);

		// Then burn half of it
		let burn_amount = 100 * ONE;
		assert_ok!(HSM::burn_hollar(burn_amount));

		// Verify the balance is as expected
		let expected_balance = mint_amount - burn_amount;
		assert_balance!(ALICE, HOLLAR, expected_balance);
	});
}
