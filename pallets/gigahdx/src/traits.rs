// SPDX-License-Identifier: Apache-2.0

//! Hooks injected by the runtime to customize gigahdx admission logic.

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
