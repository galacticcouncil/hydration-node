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
		assert_ok!(CircuitBreaker::set_global_lockdown(RuntimeOrigin::root(), 1000));
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
fn set_global_withdraw_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CircuitBreaker::set_global_withdraw_limit(RuntimeOrigin::root(), 1000));
		assert_eq!(CircuitBreaker::global_withdraw_limit(), Some(1000));

		expect_events(vec![Event::GlobalLimitUpdated { new_limit: 1000 }.into()]);
	});
}

#[test]
fn reset_global_lockdown_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		pallet_timestamp::Pallet::<Test>::set_timestamp(100);
		assert_ok!(CircuitBreaker::set_global_lockdown(RuntimeOrigin::root(), 1000));
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());

		assert_ok!(CircuitBreaker::reset_global_lockdown(RuntimeOrigin::root()));
		assert!(CircuitBreaker::withdraw_lockdown_until().is_none());
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn set_asset_category_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 100u32;

		// Set to External
		assert_ok!(CircuitBreaker::set_asset_category(
			RuntimeOrigin::root(),
			asset_id,
			Some(GlobalAssetCategory::External)
		));
		assert_eq!(
			CircuitBreaker::global_asset_overrides(asset_id),
			Some(GlobalAssetCategory::External)
		);

		// Set to Local
		assert_ok!(CircuitBreaker::set_asset_category(
			RuntimeOrigin::root(),
			asset_id,
			Some(GlobalAssetCategory::Local)
		));
		assert_eq!(
			CircuitBreaker::global_asset_overrides(asset_id),
			Some(GlobalAssetCategory::Local)
		);

		// Set to None (explicitly excluded)
		assert_ok!(CircuitBreaker::set_asset_category(
			RuntimeOrigin::root(),
			asset_id,
			None
		));
		assert!(CircuitBreaker::global_asset_overrides(asset_id).is_none());
	});
}
