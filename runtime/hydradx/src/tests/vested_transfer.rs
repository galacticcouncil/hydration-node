use super::*;
use frame_support::{assert_noop, assert_ok};
use orml_vesting::VestingSchedule;
use sp_core::crypto::AccountId32;
use sp_runtime::traits::BadOrigin;

type AccountId = AccountId32;
type Balance = u128;
type Schedule = VestingSchedule<BlockNumber, Balance>;

fn schedule_object() -> Schedule {
	Schedule {
		start: 0,
		period: 1,
		period_count: 3,
		per_period: 1_100,
	}
}

#[test]
fn vested_transfer_from_vesting_account_should_work() {
	new_test_ext().execute_with(|| {
		let from: AccountId = vesting_account();
		let to: AccountId = BOB;

		let vesting_schedule = schedule_object();

		assert_ok!(Vesting::vested_transfer(
			RawOrigin::Signed(from).into(),
			to,
			vesting_schedule
		));
	});
}

#[test]
fn vested_transfer_from_gc_account_should_work() {
	new_test_ext().execute_with(|| {
		let from: AccountId = GALACTIC_COUNCIL_ACCOUNT.into();
		let to: AccountId = BOB;

		let vesting_schedule = schedule_object();

		assert_ok!(Vesting::vested_transfer(
			RawOrigin::Signed(from).into(),
			to,
			vesting_schedule
		));
	});
}

#[test]
fn vested_transfer_from_other_account_than_gc_and_vesting_should_not_work() {
	new_test_ext().execute_with(|| {
		let from: AccountId = ALICE;
		let to: AccountId = BOB;

		let vesting_schedule = schedule_object();

		assert_noop!(
			Vesting::vested_transfer(RawOrigin::Signed(from).into(), to, vesting_schedule),
			BadOrigin
		);
	});
}
