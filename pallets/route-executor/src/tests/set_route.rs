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

use crate::tests::create_bounded_vec;
use crate::tests::mock::*;
use crate::{Error, Event, Trade};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::router::PoolType;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn set_route_should_work_when_no_prestored_route_for_asset_pair() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = (HDX, AUSD);
		let route = create_bounded_vec(vec![
			Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: STABLE_SHARE_ASSET,
			},
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: STABLE_SHARE_ASSET,
				asset_out: AUSD,
			},
		]);

		//Act
		assert_ok!(Router::set_route(
			RuntimeOrigin::signed(ALICE),
			asset_pair,
			route.clone()
		));

		//Assert
		let stored_route = Router::route(asset_pair).unwrap();
		assert_eq!(stored_route, route);
	});
}

//TODO
//- can not be set with empty route
//- check for origin
