// SPDX-License-Identifier: Apache-2.0

//! Hooks injected by the runtime to customize gigahdx admission logic.

use primitives::Balance;

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
