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
use frame_support::assert_ok;

#[test]
fn create_asset() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetRegistryModule::get_or_create_asset(b"HDX".to_vec()));

		let dot_asset = AssetRegistryModule::get_or_create_asset(b"DOT".to_vec());
		assert_ok!(dot_asset);
		let dot_asset_id = dot_asset.ok().unwrap();

		assert_ok!(AssetRegistryModule::get_or_create_asset(b"BTC".to_vec()));

		let current_asset_id = AssetRegistryModule::next_asset_id();

		// Existing asset should return previously created one.
		assert_ok!(AssetRegistryModule::get_or_create_asset(b"DOT".to_vec()), dot_asset_id);

		// Retrieving existing asset should not increased the next asset id counter.
		assert_eq!(AssetRegistryModule::next_asset_id(), current_asset_id);

		assert_eq!(AssetRegistryModule::asset_ids(b"DOT".to_vec()).unwrap(), 1u32);
		assert_eq!(AssetRegistryModule::asset_ids(b"AAA".to_vec()).is_none(), true);
	});
}
