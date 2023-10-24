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
use crate::Error::RouteHasNoTrades;
use crate::{Error, Event, Trade};
use frame_support::pallet_prelude::*;
use frame_support::{assert_noop, assert_ok};
use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};
use hydradx_traits::router::PoolType;
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use test_utils::assert_balance;

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
		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::No.into()
		);

		//Assert
		let stored_route = Router::route(asset_pair).unwrap();
		assert_eq!(stored_route, route);

		expect_events(vec![Event::RouteUpdated {
			asset_ids: vec![HDX, AUSD],
		}
		.into()]);
	});
}

#[test]
fn set_route_should_work_when_new_price_is_better() {
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

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::No.into()
		);

		expect_events(vec![Event::RouteUpdated {
			asset_ids: vec![HDX, AUSD],
		}
		.into()]);

		//Act
		let cheaper_route = create_bounded_vec(vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}]);

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, cheaper_route.clone()),
			Pays::No.into()
		);

		//Assert
		let stored_route = Router::route(asset_pair).unwrap();
		assert_eq!(stored_route, cheaper_route);

		expect_events(vec![
			Event::RouteUpdated {
				asset_ids: vec![HDX, AUSD],
			}
			.into(),
			Event::RouteUpdated {
				asset_ids: vec![HDX, AUSD],
			}
			.into(),
		]);
	});
}

#[test]
fn set_route_should_not_override_when_only_sell_price_is_better() {
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

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::No.into()
		);

		//Act
		let new_route = create_bounded_vec(vec![Trade {
			pool: PoolType::LBP,
			asset_in: HDX,
			asset_out: AUSD,
		}]);

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, new_route.clone()),
			Pays::Yes.into()
		);

		//Assert
		let stored_route = Router::route(asset_pair).unwrap();
		assert_eq!(stored_route, route);
	});
}

#[test]
fn set_route_should_not_override_when_only_buy_price_is_better() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = (HDX, AUSD);

		let route = create_bounded_vec(vec![Trade {
			pool: PoolType::LBP,
			asset_in: HDX,
			asset_out: AUSD,
		}]);

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::No.into()
		);

		//Act
		let new_route = create_bounded_vec(vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}]);

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, new_route.clone()),
			Pays::Yes.into()
		);

		//Assert
		let stored_route = Router::route(asset_pair).unwrap();
		assert_eq!(stored_route, route);
	});
}

#[test]
fn set_route_should_not_override_when_both_sell_and_buy_price_is_worse() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = (HDX, AUSD);
		let cheaper_route = create_bounded_vec(vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}]);

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, cheaper_route.clone()),
			Pays::No.into()
		);

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
		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::Yes.into()
		);

		//Assert
		let stored_route = Router::route(asset_pair).unwrap();
		assert_eq!(stored_route, cheaper_route);
	});
}

#[test]
fn set_route_should_fail_when_called_by_unsigned() {
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

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::none(), asset_pair, route.clone()),
			BadOrigin
		);
	});
}

#[test]
fn set_route_should_fail_when_asset_pair_is_invalid_for_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = (HDX, DOT);
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

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Error::<Test>::InvalidRouteForAssetPair
		);
	});
}

#[test]
fn set_route_should_fail_when_called_with_empty_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = (HDX, AUSD);
		let empty_route = create_bounded_vec(vec![]);

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, empty_route.clone()),
			Error::<Test>::RouteHasNoTrades
		);
	});
}

#[test]
fn set_route_should_fail_when_called_with_too_long_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = (HDX, AUSD);

		let trades = [Trade {
			pool: PoolType::XYK,
			asset_in: BSX,
			asset_out: AUSD,
		}; 4];

		let empty_route = create_bounded_vec(trades.to_vec());

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, empty_route.clone()),
			Error::<Test>::MaxTradesExceeded
		);
	});
}
