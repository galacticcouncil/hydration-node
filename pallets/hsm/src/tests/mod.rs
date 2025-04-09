// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod mock;

// Test modules for collateral operations
pub mod add_collateral_tests;
pub mod remove_collateral_tests;
pub mod update_collateral_tests;

// Test modules for EVM operations
pub mod evm_tests;

// Test modules for core functionality
pub mod sell_tests;

// Any test modules should be declared here
// For example:
// pub mod collateral_tests;
// pub mod buy_tests;
// pub mod arbitrage_tests;
