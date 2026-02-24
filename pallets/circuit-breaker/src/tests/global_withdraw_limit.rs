use crate::tests::mock::*;
use crate::*;
use frame_support::assert_ok;

const DAY: primitives::Moment = primitives::constants::time::unix_time::DAY;

#[test]
fn note_egress_should_increment_accumulator() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CircuitBreaker::note_egress(100));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator(), (100, 0));

		assert_ok!(CircuitBreaker::note_egress(200));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator(), (300, 0));
	});
}

#[test]
fn note_egress_should_not_trigger_lockdown_when_limit_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CircuitBreaker::set_global_withdraw_limit(RuntimeOrigin::root(), 1000));

		assert_ok!(CircuitBreaker::note_egress(999));
		// note_egress returns error when limit exceeded, and does NOT update storage or trigger lockdown
		let res = CircuitBreaker::note_egress(1);
		assert_eq!(res, Err(Error::<Test>::GlobalWithdrawLimitExceeded.into()));

		assert_eq!(CircuitBreaker::withdraw_limit_accumulator(), (999, 0));
		assert!(CircuitBreaker::withdraw_lockdown_until().is_none());
	});
}

#[test]
fn note_egress_should_fail_during_manual_lockdown() {
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(100);
		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			RuntimeOrigin::root(),
			1000
		));
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());

		let res = CircuitBreaker::note_egress(1);
		assert_eq!(res, Err(Error::<Test>::WithdrawLockdownActive.into()));
	});
}

#[test]
fn accumulator_should_decay_linearly() {
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(0);
		assert_ok!(CircuitBreaker::note_egress(1000));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator(), (1000, 0));

		// Window is 24h (86_400_000 ms)
		// 12h passed => 50% decay
		pallet_timestamp::Pallet::<Test>::set_timestamp(DAY / 2);
		CircuitBreaker::try_to_decay_withdraw_limit_accumulator();
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 500);

		// 100h passed from start => should be 0
		pallet_timestamp::Pallet::<Test>::set_timestamp(DAY * 4);
		CircuitBreaker::try_to_decay_withdraw_limit_accumulator();
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn decay_should_not_underflow() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CircuitBreaker::note_egress(1000));

		// 48h passed => should be 0, not underflow
		pallet_timestamp::Pallet::<Test>::set_timestamp(DAY * 2);
		CircuitBreaker::try_to_decay_withdraw_limit_accumulator();
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn decay_called_twice_same_now_is_idempotent() {
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(0);
		assert_ok!(CircuitBreaker::note_egress(1_000));

		// Advance half-window -> 50% decay
		pallet_timestamp::Pallet::<Test>::set_timestamp(DAY / 2);
		CircuitBreaker::try_to_decay_withdraw_limit_accumulator();
		let once = CircuitBreaker::withdraw_limit_accumulator();
		assert_eq!(once.0, 500, "half-window should reduce by half");

		// Same timestamp again -> idempotent
		CircuitBreaker::try_to_decay_withdraw_limit_accumulator();
		let twice = CircuitBreaker::withdraw_limit_accumulator();
		assert_eq!(once, twice, "Second decay at same `now` must not change state");
	});
}

#[test]
fn note_deposit_decrements_accumulator() {
	// `note_deposit` decreases accumulator and updates last_update when not in lockdown.
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(100);
		WithdrawLimitAccumulator::<Test>::put((1_000, 10));
		CircuitBreaker::note_deposit(300);
		let (current, last_update) = CircuitBreaker::withdraw_limit_accumulator();
		assert_eq!(current, 700);
		assert_eq!(last_update, 100);
	});
}

#[test]
fn note_deposit_saturates_at_zero() {
	// Saturating subtraction: underflow is prevented and result is 0.
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(200);
		WithdrawLimitAccumulator::<Test>::put((100, 10));
		CircuitBreaker::note_deposit(1_000);
		let (current, last_update) = CircuitBreaker::withdraw_limit_accumulator();
		assert_eq!(current, 0);
		assert_eq!(last_update, 200);
	});
}

#[test]
fn note_deposit_noop_during_lockdown() {
	// During lockdown, `note_deposit` must do nothing.
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(300);
		WithdrawLimitAccumulator::<Test>::put((500, 250));
		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			RuntimeOrigin::root(),
			10_000
		));
		CircuitBreaker::note_deposit(250);
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator(), (500, 250));
	});
}
