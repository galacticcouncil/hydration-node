// SPDX-License-Identifier: Apache-2.0

//! Hooks injected by the runtime to customize gigahdx admission logic.

use frame_support::pallet_prelude::DispatchResult;
use primitives::Balance;
use sp_runtime::DispatchError;

/// Sum of HDX claimed by other pallets on `who`. `giga_stake` subtracts this
/// from the caller's free balance to ensure the new stake doesn't overlap
/// with HDX already pledged elsewhere. The runtime decides which lock ids
/// count as claims (legacy staking, vesting, …) and which are allowed to
/// overlap with a gigahdx stake (e.g. conviction voting).
pub trait ExternalClaims<AccountId> {
	fn on(who: &AccountId) -> Balance;
}

impl<AccountId> ExternalClaims<AccountId> for () {
	fn on(_who: &AccountId) -> Balance {
		0
	}
}

/// Migration source for users moving from the legacy NFT-based staking pallet
/// into gigahdx. The runtime adapts this to `pallet_staking::force_unstake`.
/// Returning `Ok(unlocked)` means the caller's legacy position has been
/// destroyed and `unlocked` HDX is now free of the legacy lock and any
/// withheld rewards are paid out. Wrapped in `#[transactional]` by the
/// implementor so any failure downstream rolls the unstake back atomically.
pub trait LegacyStakeMigrator<AccountId> {
	fn force_unstake(who: &AccountId) -> Result<Balance, DispatchError>;
}

impl<AccountId> LegacyStakeMigrator<AccountId> for () {
	fn force_unstake(_who: &AccountId) -> Result<Balance, DispatchError> {
		Err(DispatchError::Other("no legacy staking source configured"))
	}
}

/// Protocol-only seize path for the gigahdx Money Market liquidation.
///
/// Three-step contract called from `pallet_liquidation::liquidate_gigahdx`:
///   1. `snapshot_stake` — read `(hdx, gigahdx)` before any state mutation.
///   2. `pre_seize` — zero `Stakes[borrower].gigahdx` so the lock-manager
///      precompile reports `locked = 0` and `LockableAToken` accepts Aave's
///      internal aToken transfer.
///   3. `finalise_seize` — after the Aave call has moved some aToken, shift
///      the matching `hdx` from borrower to recipient, restore the borrower's
///      `gigahdx` to `orig - actually_seized`, refresh locks on both accounts.
pub trait Seize<AccountId> {
	fn snapshot_stake(borrower: &AccountId) -> Result<(Balance, Balance), DispatchError>;

	fn pre_seize(borrower: &AccountId) -> Result<Balance, DispatchError>;

	fn finalise_seize(
		borrower: &AccountId,
		recipient: &AccountId,
		seize_hdx: Balance,
		seize_gigahdx: Balance,
		residual_borrower_gigahdx: Balance,
	) -> DispatchResult;
}

impl<AccountId> Seize<AccountId> for () {
	fn snapshot_stake(_borrower: &AccountId) -> Result<(Balance, Balance), DispatchError> {
		Err(DispatchError::Other("no gigahdx seize source configured"))
	}
	fn pre_seize(_borrower: &AccountId) -> Result<Balance, DispatchError> {
		Err(DispatchError::Other("no gigahdx seize source configured"))
	}
	fn finalise_seize(
		_borrower: &AccountId,
		_recipient: &AccountId,
		_seize_hdx: Balance,
		_seize_gigahdx: Balance,
		_residual_borrower_gigahdx: Balance,
	) -> DispatchResult {
		Err(DispatchError::Other("no gigahdx seize source configured"))
	}
}
