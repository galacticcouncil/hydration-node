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
fn trade_volume_limit_should_be_stored_when_called_first_time() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		// storage should be empty at the beginning
		assert_eq!(CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX), None);

		// Act
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);
	});
}

#[test]
fn trade_volume_limit_should_not_be_stored_for_omnipool_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		// storage should be empty at the beginning
		assert_eq!(CircuitBreaker::allowed_trade_volume_limit_per_asset(LRNA), None);

		// Act
		assert_ok!(CircuitBreaker::initialize_trade_limit(LRNA, INITIAL_LIQUIDITY));

		// Assert
		assert_eq!(CircuitBreaker::allowed_trade_volume_limit_per_asset(LRNA), None);
	});
}

#[test]
fn trade_volume_limit_should_not_be_overwritten_when_called_consequently() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);

		// Act
		let new_liquidity = 2_000_000;
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, new_liquidity));

		// Assert
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);
	});
}

#[test]
fn trade_volume_storage_should_be_cleared_at_the_end_of_block() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);

		// Act
		CircuitBreaker::on_finalize(2);

		// Assert
		assert_eq!(CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX), None);
	});
}

#[test]
fn trade_volume_limit_calculation_throws_error_when_overflow_happens() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CircuitBreaker::initialize_trade_limit(HDX, <Test as Config>::Balance::MAX),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn ensure_and_update_trade_volume_limit_should_work_when_liquidity_is_between_allowed_limits() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));
		assert_ok!(CircuitBreaker::initialize_trade_limit(DOT, INITIAL_LIQUIDITY));
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);

		// Act & Assert
		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			HDX, 200_000, DOT, 0
		));
		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			DOT, 0, HDX, 200_000
		));
	});
}

#[test]
fn ensure_and_update_trade_volume_limit_should_fail_when_min_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));
		assert_ok!(CircuitBreaker::initialize_trade_limit(DOT, INITIAL_LIQUIDITY));
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::ensure_and_update_trade_volume_limit(DOT, 0, HDX, 200_001),
			Error::<Test>::TokenOutflowLimitReached
		);
	});
}

#[test]
fn ensure_and_update_trade_volume_limit_should_fail_when_max_limit_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));
		assert_ok!(CircuitBreaker::initialize_trade_limit(DOT, INITIAL_LIQUIDITY));
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::ensure_and_update_trade_volume_limit(HDX, 200_001, DOT, 0),
			Error::<Test>::TokenInfluxLimitReached
		);
	});
}

#[test]
fn ensure_and_update_trade_volume_limit_should_fail_when_max_limit_is_reached_from_combined_trades() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		assert_ok!(CircuitBreaker::initialize_trade_limit(HDX, INITIAL_LIQUIDITY));
		assert_ok!(CircuitBreaker::initialize_trade_limit(DOT, INITIAL_LIQUIDITY));
		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 0,
				volume_out: 0,
				limit: 200_000,
			}
		);

		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			HDX, 150_000, DOT, 0
		));
		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			DOT, 0, HDX, 150_000
		));
		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			HDX, 150_000, DOT, 0
		));
		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			DOT, 0, HDX, 150_000
		));
		assert_ok!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			HDX, 150_000, DOT, 0
		));

		assert_eq!(
			CircuitBreaker::allowed_trade_volume_limit_per_asset(HDX).unwrap(),
			TradeVolumeLimit {
				volume_in: 450_000,
				volume_out: 300_000,
				limit: 200_000,
			}
		);

		// Act & Assert
		assert_noop!(
			CircuitBreaker::ensure_and_update_trade_volume_limit(HDX, 150_000, DOT, 0),
			Error::<Test>::TokenInfluxLimitReached
		);
	});
}

#[test]
fn ensure_and_update_trade_volume_limit_should_fail_when_limit_not_stored() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CircuitBreaker::ensure_and_update_trade_volume_limit(HDX, INITIAL_LIQUIDITY, DOT, 0),
			Error::<Test>::LiquidityLimitNotStoredForAsset
		);
	});
}

#[test]
fn ensure_and_update_trade_volume_limit_should_ingore_omnipool_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CircuitBreaker::initialize_trade_limit(LRNA, INITIAL_LIQUIDITY));
		assert_storage_noop!(CircuitBreaker::ensure_and_update_trade_volume_limit(
			LRNA,
			INITIAL_LIQUIDITY,
			LRNA,
			INITIAL_LIQUIDITY
		)
		.unwrap());
	});
}

#[test]
fn set_trade_volume_limit_should_work_when_signed_by_technical_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = (7, 100);

		assert_ok!(CircuitBreaker::set_trade_volume_limit(
			RuntimeOrigin::root(),
			HDX,
			new_limit
		));

		expect_events(vec![crate::Event::TradeVolumeLimitChanged {
			asset_id: HDX,
			trade_volume_limit: new_limit,
		}
		.into()]);
	});
}

#[test]
fn set_trade_volume_limit_should_fail_when_not_signed_by_technical_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = (7, 100);

		assert_noop!(
			CircuitBreaker::set_trade_volume_limit(RuntimeOrigin::signed(ALICE), HDX, new_limit),
			sp_runtime::DispatchError::BadOrigin
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
		let new_limit = (7, 100);

		assert_ok!(CircuitBreaker::set_trade_volume_limit(
			RuntimeOrigin::root(),
			HDX,
			new_limit
		));

		// Assert
		assert_eq!(CircuitBreaker::trade_volume_limit_per_asset(HDX), new_limit);

		expect_events(vec![crate::Event::TradeVolumeLimitChanged {
			asset_id: HDX,
			trade_volume_limit: new_limit,
		}
		.into()]);
	});
}

#[test]
fn set_trade_volume_limit_should_fail_when_setting_limit_for_omnipool_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = (7, 100);

		// Assert
		assert_noop!(
			CircuitBreaker::set_trade_volume_limit(RuntimeOrigin::root(), LRNA, new_limit),
			Error::<Test>::NotAllowed
		);
	});
}

#[test]
fn set_trade_volume_limit_should_fail_if_limit_is_not_valid() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let new_limit = (MAX_LIMIT_VALUE.checked_add(1).unwrap(), 1);

		// Assert
		assert_noop!(
			CircuitBreaker::set_trade_volume_limit(RuntimeOrigin::root(), HDX, new_limit),
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
#[should_panic(expected = "Circuit Breaker: Max net trade volume limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_trade_volume_limit_numerator_is_zero() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_trade_volume_limit_per_block((0, 10_000))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max net trade volume limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_trade_volume_limit_denominator_is_zero() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_trade_volume_limit_per_block((2_000, 0))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max net trade volume limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_trade_volume_limit_numerator_is_too_big() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_trade_volume_limit_per_block((MAX_LIMIT_VALUE.checked_add(1).unwrap(), 10_000))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}

#[test]
#[should_panic(expected = "Circuit Breaker: Max net trade volume limit per block is set to invalid value.")]
fn integrity_test_should_fail_when_trade_volume_limit_denominator_is_too_big() {
	use frame_support::traits::Hooks;
	ExtBuilder::default()
		.with_max_trade_volume_limit_per_block((2_000, MAX_LIMIT_VALUE.checked_add(1).unwrap()))
		.build()
		.execute_with(|| {
			CircuitBreaker::integrity_test();
		});
}
