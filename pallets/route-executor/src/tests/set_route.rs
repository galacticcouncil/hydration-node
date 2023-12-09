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
use crate::{Error, Event, Trade};
use frame_support::pallet_prelude::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::router::RouteProvider;
use hydradx_traits::router::{AssetPair, PoolType};
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn set_route_should_work_when_overriting_default_omnipool() {
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
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::No.into()
		);

		//Assert
		let stored_route = Router::get_route(asset_pair);
		assert_eq!(stored_route, route);

		expect_events(vec![Event::RouteUpdated {
			asset_ids: vec![HDX, AUSD],
		}
		.into()]);

		expect_no_route_executed_event()
	});
}

#[test]
fn set_route_should_store_route_in_ordered_fashion() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(DOT, HDX);
		let route = vec![
			Trade {
				pool: PoolType::XYK,
				asset_in: DOT,
				asset_out: STABLE_SHARE_ASSET,
			},
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: STABLE_SHARE_ASSET,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: AUSD,
				asset_out: HDX,
			},
		];

		//Act
		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Pays::No.into()
		);

		//Assert
		let route_ordered = vec![
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: AUSD,
				asset_out: STABLE_SHARE_ASSET,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: STABLE_SHARE_ASSET,
				asset_out: DOT,
			},
		];
		let stored_route = Router::get_route(asset_pair.ordered_pair());
		assert_eq!(stored_route, route_ordered);
	});
}

#[test]
fn set_route_should_work_when_new_price_is_better() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let route = vec![
			Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DOT,
			},
			Trade {
				pool: PoolType::LBP,
				asset_in: DOT,
				asset_out: AUSD,
			},
		];

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Pays::No.into()
		);

		expect_events(vec![Event::RouteUpdated {
			asset_ids: vec![HDX, AUSD],
		}
		.into()]);

		//Act
		let cheaper_route = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}];

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, cheaper_route.clone()),
			Pays::No.into()
		);

		//Assert
		let stored_route = Router::get_route(asset_pair);
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
		expect_no_route_executed_event()
	});
}

#[test]
fn set_route_should_not_override_when_only_normal_sell_price_is_better() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);

		let route = vec![
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: HDX,
				asset_out: STABLE_SHARE_ASSET,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: STABLE_SHARE_ASSET,
				asset_out: AUSD,
			},
		];

		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Error::<Test>::RouteUpdateIsNotSuccessful,
		);

		//Act and Assert
		let stored_route = Router::get_route(asset_pair);
		assert_eq!(stored_route, default_omnipool_route());
	});
}

#[test]
fn set_route_should_not_override_when_only_inverse_sell_price_is_better() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);

		let route = vec![
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: STABLE_SHARE_ASSET,
			},
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: STABLE_SHARE_ASSET,
				asset_out: AUSD,
			},
		];

		//Act
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Error::<Test>::RouteUpdateIsNotSuccessful
		);

		//Assert
		let stored_route = Router::get_route(asset_pair);
		assert_eq!(stored_route, default_omnipool_route());
	});
}

#[test]
fn set_route_should_not_override_when_both_sell_and_buy_price_is_worse() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let cheaper_route = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}];

		assert_ok!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, cheaper_route.clone()),
			Pays::No.into()
		);

		let route = vec![
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
		];

		//Act
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Error::<Test>::RouteUpdateIsNotSuccessful
		);

		//Assert
		let stored_route = Router::get_route(asset_pair);
		assert_eq!(stored_route, cheaper_route);
	});
}

#[test]
fn set_route_should_fail_when_called_by_unsigned() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let route = vec![
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
		];

		//Act and assert
		assert_noop!(Router::set_route(RuntimeOrigin::none(), asset_pair, route), BadOrigin);
	});
}

#[test]
fn set_route_should_fail_when_asset_pair_is_invalid_for_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, DOT);
		let route = vec![
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
		];

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Error::<Test>::InvalidRoute
		);
	});
}

#[test]
fn set_route_should_fail_when_called_with_empty_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);
		let empty_route = vec![];

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, empty_route),
			Error::<Test>::InvalidRoute
		);
	});
}

#[test]
fn set_route_should_fail_when_called_with_too_long_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);

		let trades = [Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: AUSD,
		}; 6];

		let empty_route = trades.to_vec();

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, empty_route),
			Error::<Test>::MaxTradesExceeded
		);
	});
}

#[test]
fn set_route_should_fail_when_route_is_not_valid() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);

		let route = vec![
			Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: STABLE_SHARE_ASSET,
				asset_out: AUSD,
			},
		];

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Error::<Test>::InvalidRoute
		);

		assert!(Router::route(asset_pair).is_none());
	});
}

#[test]
fn set_route_should_fail_when_trying_to_override_with_invalid_route() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_pair = AssetPair::new(HDX, AUSD);

		let invalid_route = vec![
			Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: AUSD,
			},
			Trade {
				pool: PoolType::Stableswap(STABLE_SHARE_ASSET),
				asset_in: STABLE_SHARE_ASSET,
				asset_out: AUSD,
			},
		];

		//Act and assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, invalid_route),
			Error::<Test>::InvalidRoute
		);

		let stored_route = Router::get_route(asset_pair);
		assert_eq!(stored_route, default_omnipool_route());
	});
}

#[test]
fn set_route_should_not_work_when_readded_the_same() {
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
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route.clone()),
			Pays::No.into()
		);

		//Assert
		assert_noop!(
			Router::set_route(RuntimeOrigin::signed(ALICE), asset_pair, route),
			Error::<Test>::RouteUpdateIsNotSuccessful
		);
	});
}
