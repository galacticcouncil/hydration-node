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

use crate::tests::mock::RuntimeCall;
use crate::tests::mock::*;
use crate::*;
use cumulus_pallet_xcmp_queue::XcmDeferFilter;
use frame_support::assert_storage_noop;
pub use pretty_assertions::{assert_eq, assert_ne};
use xcm::latest::prelude::*;
use xcm::VersionedXcm;

#[test]
fn deferred_by_should_track_incoming_asset_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited();
		let para_id = 999.into();

		//Act
		XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
	});
}
pub fn create_versioned_reserve_asset_deposited() -> VersionedXcm<RuntimeCall> {
	//TODO: pass an asset with volume then assert it in the test
	let multi_asset = MultiAssets::from_sorted_and_deduplicated(vec![]).unwrap();
	VersionedXcm::from(Xcm::<RuntimeCall>(vec![
		Instruction::<RuntimeCall>::ReserveAssetDeposited(multi_asset),
	]))
}
