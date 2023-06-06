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
use crate::*;
use frame_support::assert_storage_noop;
pub use pretty_assertions::{assert_eq, assert_ne};

#[test]
fn liquidity_limit_should_be_stored_when_called_first_time() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		// storage should be empty at the beginning
		assert_eq!(CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX), None);

		// Act
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);
	});
}

#[test]
fn liquidity_limit_should_not_be_stored_for_omnipool_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		// storage should be empty at the beginning
		assert_eq!(CircuitBreaker::allowed_add_liquidity_limit_per_asset(LRNA), None);

		// Act
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			LRNA,
			INITIAL_LIQUIDITY
		));

		// Assert
		assert_eq!(CircuitBreaker::allowed_add_liquidity_limit_per_asset(LRNA), None);
	});
}

#[test]
fn liquidity_limit_should_not_be_overwritten_when_called_consequently() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);

		// Act
		let new_liquidity = 2_000_000;
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(HDX, new_liquidity));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);
	});
}

#[test]
fn liquidity_storage_should_be_cleared_at_the_end_of_block() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);

		// Act
		CircuitBreaker::on_finalize(2);

		// Assert
		assert_eq!(CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX), None);
	});
}

#[test]
fn liquidity_limit_calculation_throws_error_when_overflow_happens() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CircuitBreaker::calculate_and_store_liquidity_limits(HDX, <Test as Config>::Balance::MAX),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_work_when_liquidity_is_within_limit() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);

		// Act & Assert
		assert_ok!(CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 400_000));
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_not_throw_error_when_turned_off() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::set_add_liquidity_limit(
			RuntimeOrigin::root(),
			HDX,
			None,
		));

		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert!(CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).is_none());

		// Act & Assert
		assert_ok!(CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 1_000_000));
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_not_throw_error_when_turned_off_after_storing_limit() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange

		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert_noop!(
			CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 1_000_000),
			Error::<Test>::MaxLiquidityLimitPerBlockReached
		);

		assert_ok!(CircuitBreaker::set_add_liquidity_limit(
			RuntimeOrigin::root(),
			HDX,
			None,
		));

		// the struct is in the storage, but is ignored
		assert!(CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).is_some());

		// Act & Assert
		assert_ok!(CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 1_000_000));
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_fail_when_max_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 400_001),
			Error::<Test>::MaxLiquidityLimitPerBlockReached
		);
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_fail_when_max_limit_is_reached_from_multiple_trades() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			HDX,
			INITIAL_LIQUIDITY
		));

		assert_eq!(
			CircuitBreaker::allowed_add_liquidity_limit_per_asset(HDX).unwrap(),
			LiquidityLimit {
				liquidity: 0,
				limit: 400_000,
			}
		);

		assert_ok!(CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 200_000));

		// Act & Assert
		assert_noop!(
			CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, 200_001),
			Error::<Test>::MaxLiquidityLimitPerBlockReached
		);
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_fail_when_liquidity_limit_not_stored() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CircuitBreaker::ensure_and_update_add_liquidity_limit(HDX, INITIAL_LIQUIDITY),
			Error::<Test>::LiquidityLimitNotStoredForAsset
		);
	});
}

#[test]
fn ensure_and_update_liquidity_limit_should_ingore_omnipool_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CircuitBreaker::calculate_and_store_liquidity_limits(
			LRNA,
			INITIAL_LIQUIDITY
		));
		assert_storage_noop!(CircuitBreaker::ensure_and_update_add_liquidity_limit(LRNA, INITIAL_LIQUIDITY).unwrap());
	});
}

#[test]
fn set_liquidity_limit_should_work_when_signed_by_technical_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = Some((7, 100));

		assert_ok!(CircuitBreaker::set_add_liquidity_limit(
			RuntimeOrigin::root(),
			HDX,
			new_limit
		));

		expect_events(vec![crate::Event::AddLiquidityLimitChanged {
			asset_id: HDX,
			liquidity_limit: new_limit,
		}
		.into()]);
	});
}

#[test]
fn set_liquidity_limit_should_fail_when_not_signed_by_technical_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = Some((7, 100));

		assert_noop!(
			CircuitBreaker::set_add_liquidity_limit(RuntimeOrigin::signed(ALICE), HDX, new_limit),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_liquidity_limit_should_store_new_trade_volume_limit() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let default_limit = <Test as Config>::DefaultMaxAddLiquidityLimitPerBlock::get();
		assert_eq!(default_limit, DefaultAddLiquidityLimit::<Test>::get());

		assert_eq!(CircuitBreaker::add_liquidity_limit_per_asset(HDX), default_limit);
		let new_limit = Some((7, 100));

		assert_ok!(CircuitBreaker::set_add_liquidity_limit(
			RuntimeOrigin::root(),
			HDX,
			new_limit
		));

		// Assert
		assert_eq!(CircuitBreaker::add_liquidity_limit_per_asset(HDX), new_limit);

		expect_events(vec![crate::Event::AddLiquidityLimitChanged {
			asset_id: HDX,
			liquidity_limit: new_limit,
		}
		.into()]);
	});
}

#[test]
fn set_liquidity_limit_should_fail_when_setting_limit_for_omnipool_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = (7, 100);

		// Assert
		assert_noop!(
			CircuitBreaker::set_add_liquidity_limit(RuntimeOrigin::root(), LRNA, Some(new_limit)),
			Error::<Test>::NotAllowed
		);
	});
}

#[test]
fn set_liquidity_limit_should_fail_if_limit_is_not_valid() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = (MAX_LIMIT_VALUE.checked_add(1).unwrap(), 1);

		// Assert
		assert_noop!(
			CircuitBreaker::set_add_liquidity_limit(RuntimeOrigin::root(), HDX, Some(new_limit)),
			Error::<Test>::InvalidLimitValue
		);

		assert_noop!(
			CircuitBreaker::set_trade_volume_limit(RuntimeOrigin::root(), HDX, (0, 100)),
			Error::<Test>::InvalidLimitValue
		);

		assert_noop!(
			CircuitBreaker::set_trade_volume_limit(RuntimeOrigin::root(), HDX, (100, 0)),
			Error::<Test>::InvalidLimitValue
		);
	});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max add liquidity limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_liquidity_limit_numerator_is_zero() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_add_liquidity_limit_per_block(Some((0, 10_000)))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max add liquidity limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_liquidity_limit_denominator_is_zero() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_add_liquidity_limit_per_block(Some((2_000, 0)))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max add liquidity limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_liquidity_limit_numerator_is_too_big() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_add_liquidity_limit_per_block(Some((MAX_LIMIT_VALUE.checked_add(1).unwrap(), 10_000)))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max add liquidity limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_liquidity_limit_denominator_is_too_big() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_add_liquidity_limit_per_block(Some((2_000, MAX_LIMIT_VALUE.checked_add(1).unwrap())))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}
