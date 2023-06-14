use crate::rate_limiter::*;
use crate::types::Balance;

use proptest::prelude::*;

prop_compose! {
	fn limit_and_exceeding_accumulated()(r in any::<Balance>())(
		rate_limit in Just(r),
		accumulated in r..Balance::MAX,
	) -> (Balance, Balance) {
	  (rate_limit, accumulated)
	}
}

prop_compose! {
	fn limit_and_twice_accumulated()(r in 0..(Balance::MAX / 2))(
		rate_limit in Just(r),
		accumulated in Just(r * 2),
	) -> (Balance, Balance) {
	  (rate_limit, accumulated)
	}
}

proptest! {
	#[test]
	fn deferred_duration_should_be_greater_zero_when_limit_exceeded(
		defer_duration in any::<u32>(),
		(rate_limit, total_accumulated) in limit_and_exceeding_accumulated(),
	) {
		let deferred = calculate_deferred_duration(defer_duration, rate_limit, total_accumulated);
		prop_assert_ne!(deferred, 0);
	}
}

proptest! {
	#[test]
	fn returned_value_should_be_defer_duration_when_total_accumulated_is_twice_the_rate_limit(
		defer_duration in any::<u32>(),
		(rate_limit, total_accumulated) in limit_and_twice_accumulated(),
	) {
		let deferred = calculate_deferred_duration(defer_duration, rate_limit, total_accumulated);
		prop_assert_ne!(deferred, defer_duration);
	}
}

proptest! {
	#[test]
	fn decayed_amount_should_be_less_than_initial_accumulated_amount(
		defer_duration in any::<u32>(),
		(rate_limit, accumulated_amount) in (any::<Balance>(),any::<Balance>()),
		blocks_since_last_update in any::<u32>(),
	) {
		let decayed = decay_accumulated_amount(
			defer_duration, rate_limit, accumulated_amount, blocks_since_last_update);
		prop_assert!(decayed <= accumulated_amount);
	}
}
