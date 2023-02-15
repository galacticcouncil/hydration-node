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
use crate::{EcdsaSignature, Error, EthereumAddress, SignedExtension, ValidTransaction};
use frame_support::dispatch::DispatchInfo;
use frame_support::{assert_err, assert_noop, assert_ok};
use hex_literal::hex;
use polkadot_xcm::prelude::*;
use sp_std::marker::PhantomData;
use crate::MAX_VOLUME_LIMIT;
use xcm_executor::traits::TransactAsset;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

#[test]
fn balance_should_be_locked_when_rate_limit_triggers() {
	new_test_ext().execute_with(|| {
		let asset =(MultiLocation {
			interior: X1(Parachain(1000)),
			parents: 1
		},MAX_VOLUME_LIMIT+1).into();
		let who = MultiLocation {
			interior: X1(AccountId32{network: Any, id: ALICE}),
			parents: 0
		};
		let result =  XcmRateLimit::deposit_asset(&asset,&who);
	})
}
