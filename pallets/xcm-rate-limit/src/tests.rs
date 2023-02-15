// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

use crate::mock::*;
use crate::Balance;
use crate::MAX_VOLUME_LIMIT;
use crate::{EcdsaSignature, Error, EthereumAddress, SignedExtension, ValidTransaction};
use frame_support::dispatch::DispatchInfo;
use frame_support::{assert_err, assert_noop, assert_ok};
use hex_literal::hex;
use polkadot_xcm::prelude::*;
use primitives::constants::currency::UNITS;
use sp_std::marker::PhantomData;
use xcm_executor::traits::TransactAsset;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn multi_loc(acc: [u8; 32]) -> MultiLocation {
	MultiLocation {
		interior: X1(AccountId32 { network: Any, id: acc }),
		parents: 0,
	}
}

#[test]
fn balance_should_be_locked_when_rate_limit_triggers() {
	new_test_ext().execute_with(|| {
		let create_asset = |id: u32, amount: Balance| -> MultiAsset {
			(
				MultiLocation {
					interior: X1(Parachain(id)),
					parents: 1,
				},
				amount,
			)
				.into()
		};

		//Arrange
		let bob_asset = create_asset(1000, MAX_VOLUME_LIMIT);
		let bob = multi_loc(BOB);
		let result = XcmRateLimit::deposit_asset(&bob_asset, &bob);

		let asset = create_asset(1000, 1 * UNITS);
		let who = multi_loc(ALICE);

		//Act
		let result = XcmRateLimit::deposit_asset(&asset, &who);
		assert_ok!(result);

		//Assert
		let locks = Tokens::locks(sp_runtime::AccountId32::from(ALICE), 1000);
		assert_eq!(locks[0].amount, 1 * UNITS);
	})
}
