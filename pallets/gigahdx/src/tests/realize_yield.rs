// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Event, Stakes, TotalLocked};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use primitives::Balance;

fn locked_under_ghdx(account: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

fn gigapot_balance() -> Balance {
	Balances::free_balance(GigaHdx::gigapot_account_id())
}

fn yield_realized_amount(who: AccountId) -> Option<Balance> {
	System::events().into_iter().rev().find_map(|r| match r.event {
		RuntimeEvent::GigaHdx(Event::YieldRealized { who: w, amount }) if w == who => Some(amount),
		_ => None,
	})
}

#[test]
fn realize_yield_should_move_accrued_into_principal_when_rate_increased() {
	// Pot seeded so post-stake rate = (100 + 100) / 100 = 2.
	ExtBuilder::default()
		.with_pot_balance(100 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let free_before = Balances::free_balance(ALICE);

			assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 200 * ONE);
			assert_eq!(s.gigahdx, 100 * ONE);
			assert_eq!(TotalLocked::<Test>::get(), 200 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), 200 * ONE);
			assert_eq!(gigapot_balance(), 0);
			assert_eq!(Balances::free_balance(ALICE), free_before + 100 * ONE);
			assert_eq!(yield_realized_amount(ALICE), Some(100 * ONE));
		});
}

#[test]
fn realize_yield_should_not_change_gigahdx_balance_when_called() {
	ExtBuilder::default()
		.with_pot_balance(100 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let gigahdx_before = Stakes::<Test>::get(ALICE).unwrap().gigahdx;
			let supply_before = GigaHdx::total_gigahdx_supply();

			assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

			assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, gigahdx_before);
			assert_eq!(GigaHdx::total_gigahdx_supply(), supply_before);
		});
}

#[test]
fn realize_yield_should_preserve_exchange_rate_when_called() {
	ExtBuilder::default()
		.with_pot_balance(150 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let rate_before = GigaHdx::exchange_rate();

			assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

			assert_eq!(GigaHdx::exchange_rate(), rate_before);
		});
}

#[test]
fn realize_yield_should_increase_ghdxlock_by_accrued_amount() {
	ExtBuilder::default()
		.with_pot_balance(100 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			let lock_before = locked_under_ghdx(ALICE);

			assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

			assert_eq!(locked_under_ghdx(ALICE), lock_before + 100 * ONE);
		});
}

#[test]
fn realize_yield_should_be_noop_when_no_accrued_yield() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
		assert_eq!(gigapot_balance(), 0);
		assert_eq!(yield_realized_amount(ALICE), None);
	});
}

#[test]
fn realize_yield_should_be_noop_when_no_stake_record() {
	ExtBuilder::default()
		.with_pot_balance(100 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

			assert!(Stakes::<Test>::get(ALICE).is_none());
			assert_eq!(TotalLocked::<Test>::get(), 0);
			assert_eq!(yield_realized_amount(ALICE), None);
		});
}

// A *gross* gigapot shortfall (forced via a 2:1 MM mint, far beyond any
// rounding dust) must trip the defensive tripwire. In debug/fuzz builds the
// `debug_assert` panics so it is surfaced loudly; release builds compile the
// assert out and return `GigapotInsufficient` gracefully. The two variants
// pin both halves of that contract (CI runs `test --release`).
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "exceeds rounding tolerance")]
fn realize_yield_should_panic_in_debug_when_gigapot_grossly_insufficient() {
	ExtBuilder::default().build().execute_with(|| {
		TestMoneyMarket::set_supply_rounding(2, 1);
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		// Shortfall == 100 HDX ≫ MAX_GIGAPOT_ROUNDING_SHORTFALL → debug_assert panics.
		let _ = GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into());
	});
}

#[cfg(not(debug_assertions))]
#[test]
fn realize_yield_should_return_error_in_release_when_gigapot_grossly_insufficient() {
	ExtBuilder::default().build().execute_with(|| {
		TestMoneyMarket::set_supply_rounding(2, 1);
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 200 * ONE);
		assert_eq!(gigapot_balance(), 0);

		frame_support::assert_noop!(
			GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()),
			crate::Error::<Test>::GigapotInsufficient
		);

		// State fully rolled back.
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
	});
}

#[test]
fn realize_yield_should_work_when_pending_unstakes_exist() {
	// Stake at rate 1, then a yield-paying partial unstake leaves an active
	// remainder plus a pending position; realize the remainder's yield.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 0);
			assert_eq!(s.gigahdx, 50 * ONE);
			assert_eq!(s.unstaking, 150 * ONE);
			assert_eq!(s.unstaking_count, 1);

			assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into()));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.gigahdx, 50 * ONE); // unchanged
			assert_eq!(s.hdx, 150 * ONE); // accrued folded in
			assert_eq!(s.unstaking, 150 * ONE); // untouched
			assert_eq!(s.unstaking_count, 1);
			assert_eq!(TotalLocked::<Test>::get(), 150 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), 300 * ONE); // hdx + unstaking
			assert_eq!(gigapot_balance(), 0);
			assert_eq!(yield_realized_amount(ALICE), Some(150 * ONE));
		});
}
