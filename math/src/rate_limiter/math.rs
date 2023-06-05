use sp_arithmetic::traits::SaturatedConversion;

use crate::types::Balance;

pub fn calculate_deferred_duration(global_duration: u32, rate_limit: Balance, total_accumulated: Balance) -> u32 {
	let global_duration: u128 = global_duration.max(1).saturated_into();
	// duration * (incoming + decayed - rate_limit)
	let deferred_duration =
		global_duration.saturating_mul(total_accumulated.saturating_sub(rate_limit)) / rate_limit.max(1);

	deferred_duration.saturated_into()
}

pub fn calculate_new_accumulated_amount(
	global_duration: u32,
	rate_limit: Balance,
	incoming_amount: Balance,
	accumulated_amount: Balance,
	blocks_since_last_update: u32,
) -> Balance {
	incoming_amount.saturating_add(decay_accumulated_amount(
		global_duration,
		rate_limit,
		accumulated_amount,
		blocks_since_last_update,
	))
}

pub fn decay_accumulated_amount(
	global_duration: u32,
	rate_limit: Balance,
	accumulated_amount: Balance,
	blocks_since_last_update: u32,
) -> Balance {
	let global_duration: u128 = global_duration.max(1).saturated_into();
	// acc - rate_limit * blocks / duration
	accumulated_amount
		.saturating_sub(rate_limit.saturating_mul(blocks_since_last_update.saturated_into()) / global_duration)
}
