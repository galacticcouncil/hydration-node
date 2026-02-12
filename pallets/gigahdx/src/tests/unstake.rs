use super::mock::*;
use crate::{Error, Event};
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect, Mutate as FungiblesMutate},
};
use sp_runtime::FixedU128;

fn setup_stake(who: AccountId, amount: Balance) {
	assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(who), amount));
}

#[test]
fn giga_unstake_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		// With no-op MoneyMarket, user has stHDX.
		let st_hdx_balance = <Test as crate::Config>::Currency::balance(ST_HDX, &ALICE);
		assert_eq!(st_hdx_balance, 100 * ONE);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// stHDX burned
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &ALICE), 0);

		// HDX transferred back to user (but locked)
		assert_eq!(
			<Test as crate::Config>::Currency::balance(HDX, &ALICE),
			1_000 * ONE // Back to original balance
		);

		// Unstake position created
		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 1);
		assert_eq!(positions[0].amount, 100 * ONE);
		assert_eq!(positions[0].unlock_at, 1 + 100); // block 1 + CooldownPeriod 100

		// Check event
		System::assert_last_event(
			Event::Unstaked {
				who: ALICE,
				gigahdx_withdrawn: 100 * ONE,
				st_hdx_burned: 100 * ONE,
				hdx_amount: 100 * ONE,
				unlock_at: 101,
			}
			.into(),
		);
	});
}

#[test]
fn giga_unstake_should_fail_with_zero_amount() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		assert_noop!(
			GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn giga_unstake_multiple_positions() {
	ExtBuilder::default().build().execute_with(|| {
		setup_stake(ALICE, 100 * ONE);

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 30 * ONE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions.len(), 2);
		assert_eq!(positions[0].amount, 30 * ONE);
		assert_eq!(positions[1].amount, 30 * ONE);

		// Each position has a unique lock_id
		assert_ne!(positions[0].lock_id, positions[1].lock_id);
	});
}

#[test]
fn giga_unstake_too_many_positions_should_fail() {
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 100_000 * ONE)])
		.build()
		.execute_with(|| {
			setup_stake(ALICE, 11_000 * ONE);

			// Create MaxUnstakePositions (10) positions
			for _ in 0..10 {
				assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE));
			}

			// 11th should fail
			assert_noop!(
				GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), ONE),
				Error::<Test>::TooManyUnstakePositions
			);
		});
}

#[test]
fn giga_unstake_at_increased_rate() {
	ExtBuilder::default().build().execute_with(|| {
		// Stake 100 HDX at 1:1
		setup_stake(ALICE, 100 * ONE);

		// Simulate fee accrual: double the gigapot
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 100 * ONE));

		// Exchange rate is now 2.0
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::from(2));

		// Unstake 100 stHDX worth of GIGAHDX → should get 200 HDX
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		let positions = GigaHdx::unstake_positions(&ALICE);
		assert_eq!(positions[0].amount, 200 * ONE);
	});
}
