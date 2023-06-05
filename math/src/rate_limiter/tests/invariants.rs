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

proptest! {
	#[test]
	fn deferred_duration_should_be_greater_zero_when_limit_exceeded(
		global_duration in any::<u32>(),
		(rate_limit, total_accumulated) in limit_and_exceeding_accumulated(),
	) {
		let deferred = calculate_deferred_duration(global_duration, rate_limit, total_accumulated);
		prop_assert_ne!(deferred, 0);
	}
}

proptest! {
	#[test]
	fn decayed_amount_should_be_less_than_initial_accumulated_amount(
		global_duration in any::<u32>(),
		(rate_limit, accumulated_amount) in (any::<Balance>(),any::<Balance>()),
		blocks_since_last_update in any::<u32>(),
	) {
		let decayed = decay_accumulated_amount(
			global_duration, rate_limit, accumulated_amount, blocks_since_last_update);
		prop_assert!(decayed <= accumulated_amount);
	}
}
