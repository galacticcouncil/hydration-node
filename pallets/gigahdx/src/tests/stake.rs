use super::mock::*;
use crate::{Error, Event};
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect, Mutate as FungiblesMutate},
};
use sp_runtime::{traits::One, FixedU128};

#[test]
fn giga_stake_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 100 * ONE;

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), amount));

		let gigapot = GigaHdx::gigapot_account_id();

		// HDX transferred to gigapot
		assert_eq!(<Test as crate::Config>::Currency::balance(HDX, &gigapot), amount);

		// stHDX minted to user (1:1 at initial rate).
		// With no-op MoneyMarket, supply() is identity — stHDX stays with user.
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &ALICE), amount);

		// Exchange rate is still 1:1
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());

		// Check event
		System::assert_last_event(
			Event::Staked {
				who: ALICE,
				hdx_amount: amount,
				st_hdx_minted: amount,
				gigahdx_received: amount,
				exchange_rate: FixedU128::one(),
			}
			.into(),
		);
	});
}

#[test]
fn giga_stake_should_fail_below_min_stake() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = ONE / 2; // Below MinStake of 1 ONE

		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), amount),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn giga_stake_should_fail_with_zero_amount() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn giga_stake_should_fail_with_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE has 1000 ONE, try to stake 2000 ONE
		assert!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 2_000 * ONE).is_err());
	});
}

#[test]
fn giga_stake_proportional_at_increased_rate() {
	ExtBuilder::default().build().execute_with(|| {
		// First stake: 100 HDX at 1:1
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Simulate fee accrual: add 100 HDX to gigapot directly
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 100 * ONE));

		// Exchange rate is now 200 HDX / 100 stHDX = 2.0
		let rate = GigaHdx::exchange_rate();
		assert_eq!(rate, FixedU128::from(2));

		// Second stake: 100 HDX should get 50 stHDX (100 * 100 / 200 = 50)
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 100 * ONE));

		System::assert_last_event(
			Event::Staked {
				who: BOB,
				hdx_amount: 100 * ONE,
				st_hdx_minted: 50 * ONE,
				gigahdx_received: 50 * ONE,
				exchange_rate: GigaHdx::exchange_rate(),
			}
			.into(),
		);
	});
}

#[test]
fn giga_stake_multiple_users() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 200 * ONE));

		let gigapot = GigaHdx::gigapot_account_id();

		// Gigapot has 300 HDX
		assert_eq!(<Test as crate::Config>::Currency::balance(HDX, &gigapot), 300 * ONE);

		// Total stHDX supply is 300 ONE (1:1 rate)
		assert_eq!(GigaHdx::total_st_hdx_supply(), 300 * ONE);

		// Exchange rate unchanged
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}
