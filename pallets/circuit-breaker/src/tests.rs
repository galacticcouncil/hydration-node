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
	ExtBuilder::default()
		.build()
		.execute_with(|| {
			// Arrange
            let asset_id = 100;
            let initial_liquidity = 1_000_000;

			// storage should be empty prior calling on_trade
			assert_eq!(CircuitBreaker::initial_liquidity(asset_id), None);

			// Act
            assert_ok!(CircuitBreaker::on_trade(asset_id, initial_liquidity));

			// Assert
			assert_eq!(CircuitBreaker::initial_liquidity(asset_id), Some(initial_liquidity));
		});
}

#[test]
fn on_trade_should_overwrite_liquidity_when_called_consequently() {
	ExtBuilder::default()
		.build()
		.execute_with(|| {
			//Arrange
            let asset_id = 100;
            let initial_liquidity = 1_000_000;
            assert_ok!(CircuitBreaker::on_trade(asset_id, initial_liquidity));
			assert_eq!(CircuitBreaker::initial_liquidity(asset_id), Some(initial_liquidity));

			// Act
			let new_liquidity = 2_000_000;
			assert_ok!(CircuitBreaker::on_trade(asset_id, new_liquidity));

			// Assert
			assert_eq!(CircuitBreaker::initial_liquidity(asset_id), Some(initial_liquidity));
		});
}