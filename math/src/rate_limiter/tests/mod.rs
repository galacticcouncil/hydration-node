use super::*;

mod invariants;

pub const ONE: u128 = 1_000_000_000_000;

#[test]
fn decay_accumulated_amount_works() {
	assert_eq!(decay_accumulated_amount(600, 100 * ONE, 50 * ONE, 150), 25 * ONE);
}

#[test]
fn deferred_duration_should_be_calculated_based_on_limit_and_incoming_amounts() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 1500 * ONE;
	let accumulated_amount = 400 * ONE;
	let total_accumulated_amount = accumulated_amount + incoming_amount;
	let duration = calculate_deferred_duration(global_duration, rate_limit, total_accumulated_amount);

	assert_eq!(duration, 9);
}

#[test]
fn deferred_duration_should_return_zero_when_limit_not_reached() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 900 * ONE;
	let accumulated_amount = 0;
	let total_accumulated_amount = accumulated_amount + incoming_amount;

	let duration = calculate_deferred_duration(global_duration, rate_limit, total_accumulated_amount);

	assert_eq!(duration, 0);
}

#[test]
fn accumulated_amount_for_deferred_duration_should_decay() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 1100 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 12;
	let accumulated_amount = calculate_new_accumulated_amount(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(accumulated_amount, 1100 * ONE);
}

#[test]
fn defer_duration_should_incorporate_decay_amounts_and_incoming() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 1100 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 6;
	let accumulated_amount = calculate_new_accumulated_amount(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(accumulated_amount, 1700 * ONE);
}

#[test]
fn long_time_since_update_should_reset_rate_limit() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 700 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 20;
	let accumulated_amount = calculate_new_accumulated_amount(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(accumulated_amount, 700 * ONE);
}

#[test]
fn calculate_new_accumulated_amount_should_decay_old_amounts_and_sum() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 700 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 6;
	let total_accumulated = calculate_new_accumulated_amount(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(total_accumulated, 700 * ONE + 600 * ONE);
}
