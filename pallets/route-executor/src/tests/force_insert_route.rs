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

use crate::tests::mock::*;
use crate::{Error, Trade};
use frame_support::pallet_prelude::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::router::{AssetPair, PoolType};
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn force_insert_should_not_work_when_called_with_non_technical_origin() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let route = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}];

		//Act
		assert_noop!(
			Router::force_insert_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			BadOrigin
		);
	});
}

#[test]
fn force_insert_should_work_when_called_with_technical_origin() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let route = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}];

		//Act
		assert_ok!(
			Router::force_insert_route(RuntimeOrigin::root(), asset_pair, route),
			Pays::No.into()
		);
	});
}

#[test]
fn force_insert_should_fail_when_called_with_too_big_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let route = vec![
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
		];

		//Act
		assert_noop!(
			Router::force_insert_route(RuntimeOrigin::root(), asset_pair, route),
			Error::<Test>::MaxTradesExceeded
		);
	});
}

//TODO: add  test that it can add insuffucient asset
