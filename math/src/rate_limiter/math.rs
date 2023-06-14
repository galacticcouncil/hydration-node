use sp_arithmetic::traits::SaturatedConversion;

use crate::types::Balance;

/// Calculate how long to defer something based on ratio between `rate_limit` and `total_accumulated`.
/// Will return 0 if `total_accumulated` is less than `rate_limit`.
/// 2x `rate_limit` accumulated tokens will be deferred by `defer_duration`.
pub fn calculate_deferred_duration(defer_duration: u32, rate_limit: Balance, total_accumulated: Balance) -> u32 {
	let defer_duration: u128 = defer_duration.max(1).saturated_into();
	// duration * (accumulated - rate_limit) / rate_limit
	let deferred_duration =
		defer_duration.saturating_mul(total_accumulated.saturating_sub(rate_limit)) / rate_limit.max(1);

	deferred_duration.saturated_into()
}

/// Calculate how much balance has accumulated by decaying the previous `accumulated_amount` based on
/// `blocks_since_last_update` and adding `incoming_amount`.
pub fn calculate_new_accumulated_amount(
	defer_duration: u32,
	rate_limit: Balance,
	incoming_amount: Balance,
	accumulated_amount: Balance,
	blocks_since_last_update: u32,
) -> Balance {
	incoming_amount.saturating_add(decay_accumulated_amount(
		defer_duration,
		rate_limit,
		accumulated_amount,
		blocks_since_last_update,
	))
}

/// Calculate how much the `accumulated_amount` has decayed based on `blocks_since_last_update` and `rate_limit`.
pub fn decay_accumulated_amount(
	defer_duration: u32,
	rate_limit: Balance,
	accumulated_amount: Balance,
	blocks_since_last_update: u32,
) -> Balance {
	let defer_duration: u128 = defer_duration.max(1).saturated_into();
	// acc - rate_limit * blocks / duration
	accumulated_amount
		.saturating_sub(rate_limit.saturating_mul(blocks_since_last_update.saturated_into()) / defer_duration)
}
