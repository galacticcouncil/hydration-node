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
fn vested_transfer_should_work_when_signed_by_vesting_account() {
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
fn vested_transfer_should_work_when_sent_from_root() {
	new_test_ext().execute_with(|| {
		let to: AccountId = BOB;

		let vesting_schedule = schedule_object();

		// let bob_balance_before = Balances::free_balance(BOB);
		let vesting_balance_before = Balances::free_balance(vesting_account());

		assert_ok!(Vesting::vested_transfer(RawOrigin::Root.into(), to, vesting_schedule));

		// let bob_balance_after = Balances::free_balance(BOB);
		let vesting_balance_after = Balances::free_balance(vesting_account());

		// vesting_balance_after is 0 ?!
		assert_eq!(vesting_balance_before, vesting_balance_after);
	});
}

#[test]
fn vested_transfer_should_not_work_when_signed_by_other_account() {
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
