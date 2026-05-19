// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0
//
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

use frame_support::{
	migrations::RemovePallet,
	parameter_types,
	traits::OnRuntimeUpgrade,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_io::hashing::twox_128;
use sp_std::vec::Vec;

parameter_types! {
	pub const IsmpName: &'static str = "Ismp";
	pub const IsmpParachainName: &'static str = "IsmpParachain";
	pub const HyperbridgeName: &'static str = "Hyperbridge";
	pub const TokenGatewayName: &'static str = "TokenGateway";
}

/// Removes the `CleanupEnabled` storage value from `pallet_dispatcher` so that on the
/// next read it falls back to its `DefaultTrue` default.
pub struct KillDispatcherCleanupEnabled;

impl OnRuntimeUpgrade for KillDispatcherCleanupEnabled {
	fn on_runtime_upgrade() -> Weight {
		let mut key = Vec::with_capacity(32);
		key.extend_from_slice(&twox_128(b"Dispatcher"));
		key.extend_from_slice(&twox_128(b"CleanupEnabled"));
		frame_support::storage::unhashed::kill(&key);
		RocksDbWeight::get().writes(1)
	}
}

pub type CleanupHyperbridge = (
	RemovePallet<IsmpName, RocksDbWeight>,
	RemovePallet<IsmpParachainName, RocksDbWeight>,
	RemovePallet<HyperbridgeName, RocksDbWeight>,
	RemovePallet<TokenGatewayName, RocksDbWeight>,
	KillDispatcherCleanupEnabled,
);
