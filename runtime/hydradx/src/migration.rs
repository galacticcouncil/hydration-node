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

use super::*;
use frame_support::{
	dispatch::{GetDispatchInfo, RawOrigin},
	traits::OnRuntimeUpgrade,
	weights::Weight,
};
pub struct OnRuntimeUpgradeMigration;
use super::Runtime;

pub fn bind_pallet_account() -> Weight {
	match EVMAccounts::bind_evm_address(RawOrigin::Signed(Liquidation::account_id()).into()) {
		Ok(_) => {
			log::info!(
				target: "runtime::pallet_liquidation",
				"Migration to v1 for Liquidation pallet"
			);
		}
		Err(error) => {
			log::info!(
				target: "runtime::pallet_liquidation",
				"Migration to v1 for Liquidation pallet failed: {:?}", error
			);
		}
	}

	let call = pallet_evm_accounts::Call::<Runtime>::bind_evm_address {};

	call.get_dispatch_info().weight
}

impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	fn on_runtime_upgrade() -> Weight {
		bind_pallet_account()
	}
}
