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
pub use pretty_assertions::{assert_eq, assert_ne};

#[test]
fn ensure_remove_liquidity_limit_should_be_ignored_for_admin_when_limit_is_reached() {
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
		assert_ok!(CircuitBreaker::ensure_remove_liquidity_limit(
			RuntimeOrigin::signed(WHITELISTED_ACCCOUNT),
			HDX,
			INITIAL_LIQUIDITY,
			400_001
		));
	});
}
