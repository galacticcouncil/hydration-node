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
use frame_support::sp_runtime::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use primitives::Balance;

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

	/// Weight overhead this implementation contributes on top of the
	/// pallet's substrate-side `giga_stake` weight. EVM-backed impls should
	/// return the gas-equivalent weight of the underlying call so block
	/// weight tracks the real cost. Defaults to zero for tests / no-op impls.
	fn supply_weight() -> Weight {
		Weight::zero()
	}

	/// Symmetric for `withdraw`.
	fn withdraw_weight() -> Weight {
		Weight::zero()
	}
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

/// Protocol-only seize path for the gigahdx Money Market liquidation.
///
/// Contract called from `pallet_liquidation::liquidate_gigahdx`:
///   0. `realize_yield` — fold the borrower's accrued GIGAHDX yield into
///      `Stakes[borrower].hdx` so the snapshot reflects the position's true
///      HDX value (a drained-stake borrower otherwise shows `hdx = 0` and the
///      pro-rata `seize_hdx` would round to nothing). No-op when nothing accrued.
///   1. `snapshot_stake` — read `(hdx, gigahdx)` before any state mutation.
///   2. `on_pre_seize` — zero `Stakes[borrower].gigahdx` so the lock-manager
///      precompile reports `locked = 0` and `LockableAToken` accepts Aave's
///      internal aToken transfer.
///   3. `on_seize` — after the Aave call has moved some aToken, shift the
///      matching `hdx` from borrower to recipient, restore the borrower's
///      `gigahdx` to `orig_gigahdx - seize_gigahdx`, refresh locks on both
///      accounts.
pub trait Seize<AccountId> {
	fn realize_yield(borrower: &AccountId) -> DispatchResult;

	fn snapshot_stake(borrower: &AccountId) -> Result<(Balance, Balance), DispatchError>;

	fn on_pre_seize(borrower: &AccountId) -> Result<Balance, DispatchError>;

	fn on_seize(
		borrower: &AccountId,
		recipient: &AccountId,
		seize_hdx: Balance,
		seize_gigahdx: Balance,
		orig_gigahdx: Balance,
	) -> DispatchResult;
}

/// Conviction-voting interaction used by `pallet-liquidation` during a gigahdx
/// liquidation: it removes votes no longer backed by the borrower's residual
/// stake and resyncs the `pyconvot` lock via conviction-voting's `unlock`.
pub trait ClearConflictingVotes<AccountId> {
	fn clear_conflicting_votes(who: &AccountId, max_remaining_hdx: Balance) -> Result<u32, DispatchError>;
}

impl<AccountId> ClearConflictingVotes<AccountId> for () {
	fn clear_conflicting_votes(_who: &AccountId, _max_remaining_hdx: Balance) -> Result<u32, DispatchError> {
		Ok(0)
	}
}
