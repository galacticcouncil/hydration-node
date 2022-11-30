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

use super::*;
use crate::mock::*;

#[test]
fn on_trade_should_store_liquidity_when_called_first_time() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let asset_id = 100;
		let initial_liquidity = 1_000_000;

		// storage should be empty prior calling on_trade
		assert_eq!(CircuitBreaker::allowed_liqudity_range_per_asset(asset_id), None);

		// Act
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			initial_liquidity
		));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);
	});
}

#[test]
fn on_trade_should_not_overwrite_liquidity_when_called_consequently() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_id = 100;
		let initial_liquidity = 1_000_000;
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			initial_liquidity
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);

		// Act
		let new_liquidity = 2_000_000;
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			new_liquidity
		));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);
	});
}

#[test]
fn liquidity_storage_should_be_cleared_in_the_next_block() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_id = 100;
		let initial_liquidity = 1_000_000;

		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			initial_liquidity
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);

		// Act
		CircuitBreaker::on_finalize(2);

		// Assert
		assert_eq!(CircuitBreaker::allowed_liqudity_range_per_asset(asset_id), None);
	});
}

#[test]
fn max_limit_calculation_throws_error_when_overflow_happens() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_id = 100;

		assert_noop!(
			CircuitBreaker::calculate_and_store_liquidity_limits(asset_id, Balance::MAX),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn test_liquidity_limits_should_work_when_liquidity_is_between_allowed_limits() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_id = 100;
		let initial_liquidity = 1_000_000;

		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			initial_liquidity
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);

		// Act & Assert
		assert_ok!(CircuitBreaker::test_liquidity_limits(asset_id, initial_liquidity));
		assert_ok!(CircuitBreaker::test_liquidity_limits(asset_id, 500_000));
		assert_ok!(CircuitBreaker::test_liquidity_limits(asset_id, 1_500_000));
	});
}

#[test]
fn test_liquidity_limits_should_fail_when_min_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_id = 100;
		let initial_liquidity = 1_000_000;

		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			initial_liquidity
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::test_liquidity_limits(asset_id, 499_999),
			Error::<Test>::MinTradeVolumePerBlockReached
		);
	});
}

#[test]
fn test_liquidity_limits_should_fail_when_max_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let asset_id = 100;
		let initial_liquidity = 1_000_000;

		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			asset_id,
			initial_liquidity
		));
		assert_eq!(
			CircuitBreaker::allowed_liqudity_range_per_asset(asset_id).unwrap(),
			(500_000, 1_500_000)
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::test_liquidity_limits(asset_id, 1_500_001),
			Error::<Test>::MaxTradeVolumePerBlockReached
		);
	});
}
