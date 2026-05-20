// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes, TotalLocked};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_traits::gigahdx::MoneyMarketOperations;

#[test]
fn do_stake_should_create_new_stake_record_when_user_had_none() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(Stakes::<Test>::get(ALICE).is_none());

		assert_ok!(GigaHdx::do_stake(&ALICE, 5 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 5 * ONE);
		assert_eq!(s.gigahdx, 5 * ONE); // 1:1 bootstrap rate
		assert_eq!(s.frozen, 0);
		assert_eq!(TotalLocked::<Test>::get(), 5 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 5 * ONE);
	});
}

#[test]
fn do_stake_should_compound_into_existing_position() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::do_stake(&ALICE, 10 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 110 * ONE);
		assert_eq!(s.gigahdx, 110 * ONE);
	});
}

#[test]
fn do_stake_should_bypass_min_stake_check() {
	ExtBuilder::default().build().execute_with(|| {
		// Sub-MinStake amount that giga_stake would reject:
		let tiny = ONE / 10;
		assert!(tiny < <Test as crate::Config>::MinStake::get());

		assert_ok!(GigaHdx::do_stake(&ALICE, tiny));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, tiny);
	});
}

#[test]
fn do_stake_should_fail_when_amount_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(GigaHdx::do_stake(&ALICE, 0), Error::<Test>::ZeroAmount);
	});
}

#[test]
fn do_stake_should_fail_when_conversion_rounds_to_zero() {
	ExtBuilder::default()
		.with_pot_balance(1_000 * ONE)
		.build()
		.execute_with(|| {
			// Set up a heavily appreciated rate: stake → fund pot → next mint floors.
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			// After this, rate ≈ (100 + 1000) / 100 = 11 HDX per gigahdx.
			// To mint zero, we need amount < 11. Try with 1 picoHDX-equivalent.
			let amount = 10u128;
			assert_noop!(GigaHdx::do_stake(&ALICE, amount), Error::<Test>::ZeroAmount);
		});
}

#[test]
fn do_stake_should_revert_storage_when_mm_supply_fails() {
	ExtBuilder::default().build().execute_with(|| {
		TestMoneyMarket::fail_supply();

		assert_noop!(
			GigaHdx::do_stake(&ALICE, 5 * ONE),
			Error::<Test>::MoneyMarketSupplyFailed
		);

		// State untouched.
		assert!(Stakes::<Test>::get(ALICE).is_none());
		assert_eq!(TotalLocked::<Test>::get(), 0);
	});
}

#[test]
fn do_stake_should_lock_hdx_under_giga_lock_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::do_stake(&ALICE, 5 * ONE));

		let lock = pallet_balances::Locks::<Test>::get(ALICE)
			.iter()
			.find(|l| l.id == GIGAHDX_LOCK_ID)
			.map(|l| l.amount)
			.unwrap_or(0);
		assert_eq!(lock, 5 * ONE);
	});
}
