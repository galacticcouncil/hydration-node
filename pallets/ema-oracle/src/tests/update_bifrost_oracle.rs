// This file is part of pallet-ema-oracle.

// Copyright (C) 2022-2023  Intergalactic, Limited (GIB).
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
pub use mock::{expect_events, EmaOracle, RuntimeOrigin, Test, DOT, HDX, ORACLE_ENTRY_1};
use std::sync::Arc;

use frame_support::{assert_noop, assert_ok};

use polkadot_xcm::VersionedLocation;
use pretty_assertions::assert_eq;

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

pub const HYDRA_PARA_ID: u32 = 2_034;

use crate::tests::mock::ALICE;
use hydradx_traits::evm::EvmAddress;
use polkadot_xcm::v3::Junction::{AccountKey20, GeneralIndex, Parachain};
use polkadot_xcm::v3::Junctions::{Here, X1, X2};
use polkadot_xcm::v3::{Junction, MultiLocation};
use sp_runtime::{DispatchResult, TransactionOutcome};

#[test]
fn add_oracle_should_add_entry_to_storage() {
	new_test_ext().execute_with(|| {
		//Arrange
		let hdx =
			polkadot_xcm::v3::MultiLocation::new(0, polkadot_xcm::v3::Junctions::X1(GeneralIndex(0))).into_versioned();

		let dot = polkadot_xcm::v3::MultiLocation::parent().into_versioned();

		let asset_a = Box::new(hdx);
		let asset_b = Box::new(dot);

		//Act
		System::set_block_number(3);

		assert_ok!(EmaOracle::update_bifrost_oracle(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			(100, 99)
		));

		update_aggregated_oracles();

		//Assert
		let entry = Oracles::<Test>::get((BITFROST_SOURCE, ordered_pair(0, 5), OraclePeriod::Day)).map(|(e, _)| e);
		assert!(entry.is_some());
		let entry = entry.unwrap();
		assert_eq!(entry.price, EmaPrice::new(100, 99));
		assert_eq!(entry.volume, Volume::default());
		assert_eq!(entry.liquidity, Liquidity::default());
		assert_eq!(entry.updated_at, 3);
	});
}

pub fn update_aggregated_oracles() {
	EmaOracle::on_finalize(6);
	System::set_block_number(7);
	EmaOracle::on_initialize(7);
}

//TODO: add negative test when it is not called by bitfrost origni
