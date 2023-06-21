use mock::*;

use crate::types::{Position, StakingData};
use crate::*;
use frame_support::{assert_noop, assert_ok};
use orml_tokens::BalanceLock;

mod claim;
pub(crate) mod mock;
mod stake;

/// Assert amount of locked tokens.
///
/// Parameters:
/// - `who`
/// - `amount`
/// - `lock_id`
#[macro_export]
macro_rules! assert_hdx_lock {
	($x: expr, $y: expr, $z: expr) => {
		let locks = Tokens::locks($x, HDX);
		let lock = locks.iter().find(|e| e.id == $z);

		assert_eq!(lock, Some(&BalanceLock { id: $z, amount: $y }));
	};
}

/// Assert StakingData saved in pallet staking storage.
///
/// Parameters:
/// - `total_stake`
/// - `accumulated_reward_per_stake`
/// - `pendig_rew`
#[macro_export]
macro_rules! assert_staking_data {
	($x: expr, $y: expr, $z: expr) => {
		assert_eq!(
			Staking::staking(),
			StakingData {
				total_stake: $x,
				accumulated_reward_per_stake: $y,
				pending_rew: $z,
			}
		);
	};
}

/// Asserts unlocked(unfrozen) balance on the account.
///
/// Parameters:
/// - `who`
/// - `currency_id`
/// - `expected_amount`
#[macro_export]
macro_rules! assert_unlocked_balance {
	($x: expr, $y: expr, $z: expr) => {
		let frozen = Tokens::accounts(&$x, $y).frozen;
		assert_eq!(Tokens::free_balance($y, &$x).saturating_sub(frozen), $z);
	};
}
