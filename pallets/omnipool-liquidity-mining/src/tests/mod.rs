// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

use super::*;
use mock::*;

use frame_support::{assert_noop, assert_ok};
use sp_runtime::{traits::BadOrigin, traits::One, FixedU128};

macro_rules! assert_last_event {
	( $x:expr ) => {{
		pretty_assertions::assert_eq!(System::events().last().expect("events expected").event, $x);
	}};
}

pub fn has_event(event: mock::RuntimeEvent) -> bool {
	System::events().iter().any(|record| record.event == event)
}

pub mod create_global_farm;
pub mod create_yield_farm;
pub mod deposit_shares;
pub mod mock;
pub mod redeposit_shares;
pub mod resume_yield_farm;
pub mod stop_yield_farm;
pub mod terminate_global_farm;
pub mod terminate_yield_farm;
pub mod update_yield_farm;
pub mod withdraw_shares;
