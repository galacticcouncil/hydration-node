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

use crate::test::mock::*;
use crate::*;
use sp_runtime::Percent;

#[test]
fn on_trade_should_store_liquidity_when_called_first_time() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		// storage should be empty prior calling on_trade
		assert_eq!(CircuitBreaker::allowed_liqudity_range_per_asset(HDX), None);

		// Act
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);
	});
}

#[test]
fn on_trade_should_not_overwrite_liquidity_when_called_consequently() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);

		// Act
		let new_liquidity = 2_000_000;
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(HDX, new_liquidity));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);
	});
}

#[test]
fn liquidity_storage_should_be_cleared_in_the_next_block() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);

		// Act
		CircuitBreaker::on_finalize(2);

		// Assert
		assert_eq!(CircuitBreaker::allowed_liqudity_range_per_asset(HDX), None);
	});
}

#[test]
fn max_limit_calculation_throws_error_when_overflow_happens() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CircuitBreaker::calculate_and_store_liquidity_limits(HDX, Balance::MAX),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn test_liquidity_limits_should_work_when_liquidity_is_between_allowed_limits() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);

		// Act & Assert
		assert_ok!(CircuitBreaker::test_liquidity_limits(HDX, INITIAL_LIQUIDITY));
		assert_ok!(CircuitBreaker::test_liquidity_limits(HDX, 800_000));
		assert_ok!(CircuitBreaker::test_liquidity_limits(HDX, 1_200_000));
	});
}

#[test]
fn test_liquidity_limits_should_fail_when_min_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::test_liquidity_limits(HDX, 799_999),
			Error::<Test>::MinTradeVolumePerBlockReached
		);
	});
}

#[test]
fn test_liquidity_limits_should_fail_when_max_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(HDX).unwrap(),
			(800_000, 1_200_000)
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::test_liquidity_limits(HDX, 1_200_001),
			Error::<Test>::MaxTradeVolumePerBlockReached
		);
	});
}

#[test]
fn test_liquidity_limits_should_fail_when_liqudity_limit_not_stored() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CircuitBreaker::test_liquidity_limits(HDX, INITIAL_LIQUIDITY),
			Error::<Test>::LiquidityLimitNotStoredForAsset
		);
	});
}

#[test]
fn set_trade_volume_limit_should_store_new_trade_volume_limit() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let default_limit = <Test as Config>::DefaultMaxNetTradeVolumeLimitPerBlock::get();
		assert_eq!(default_limit, DefaultTradeVolumeLimit::<Test>::get());

		assert_eq!(CircuitBreaker::trade_volume_limit_per_asset(HDX), default_limit);
		let new_limit = Percent::from_percent(7);

		assert_ok!(CircuitBreaker::set_trade_volume_limit(Origin::root(), HDX, new_limit,));

		// Assert
		assert_eq!(CircuitBreaker::trade_volume_limit_per_asset(HDX), new_limit);
	});
}
