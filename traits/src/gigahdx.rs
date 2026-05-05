// This file is part of hydradx-traits.

// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

use frame_support::sp_runtime::traits::Zero;
use frame_support::sp_runtime::DispatchError;

/// Bridges `pallet-gigahdx` to a money market (e.g. an Aave V3 fork on EVM).
///
/// Behaviour contract:
/// - `supply` is called by `pallet-gigahdx` after stHDX is minted to the
///   pallet account. The adapter consumes that stHDX and mints aToken
///   (GIGAHDX) to `who`. Returns the amount of aToken received.
/// - `withdraw` is called during unstake. The adapter burns the aToken from
///   `who` and returns the underlying stHDX. Returns the amount returned.
/// - `balance_of` returns the user's current aToken balance — used for
///   defensive checks before initiating a withdraw.
///
/// The `who` is the substrate `AccountId`. EVM-backed implementations are
/// responsible for resolving the user's H160.
pub trait MoneyMarketOperations<AccountId, AssetId, Balance> {
	/// Supply `amount` of `underlying_asset` on behalf of `who`. Returns the
	/// amount of aToken received.
	fn supply(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError>;

	/// Burn aToken from `who` and return `amount` of `underlying_asset`.
	/// Returns the amount of underlying received.
	fn withdraw(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError>;

	/// User's current aToken (GIGAHDX) balance in the money market.
	fn balance_of(who: &AccountId) -> Balance;
}

/// No-op implementation. Useful in tests and on chains that have not yet
/// deployed the money market. `supply`/`withdraw` are identity, `balance_of`
/// is always zero.
impl<AccountId, AssetId, Balance: Zero> MoneyMarketOperations<AccountId, AssetId, Balance> for () {
	fn supply(_who: &AccountId, _underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount)
	}

	fn withdraw(_who: &AccountId, _underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount)
	}

	fn balance_of(_who: &AccountId) -> Balance {
		Balance::zero()
	}
}
