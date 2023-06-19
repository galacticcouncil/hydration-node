use mock::*;

use crate::types::{Position, StakingData};
use crate::*;
use frame_support::{assert_noop, assert_ok};
use orml_tokens::BalanceLock;

pub(crate) mod mock;
mod stake;

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
