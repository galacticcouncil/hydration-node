// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Event, Stakes, TotalLocked};
use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_traits::gigahdx::MoneyMarketOperations;
use primitives::Balance;

fn last_events(n: usize) -> Vec<RuntimeEvent> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.rev()
		.take(n)
		.map(|r| r.event)
		.collect()
}

fn locked_under_ghdx(account: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

#[test]
fn migrate_should_open_gigahdx_position_from_legacy_unlock() {
	ExtBuilder::default().build().execute_with(|| {
		TestLegacyStaking::set_ok(100 * ONE);

		assert_ok!(GigaHdx::migrate(RawOrigin::Signed(ALICE).into()));

		assert_eq!(TestLegacyStaking::called_for(), Some(ALICE));
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 100 * ONE);

		let found = last_events(5).iter().any(|e| {
			matches!(
				e,
				RuntimeEvent::GigaHdx(Event::MigratedFromLegacy {
					who,
					hdx_unlocked,
					gigahdx_received,
				}) if *who == ALICE && *hdx_unlocked == 100 * ONE && *gigahdx_received == 100 * ONE
			)
		});
		assert!(found, "expected MigratedFromLegacy event");
	});
}

#[test]
fn migrate_should_fail_when_legacy_unstake_refuses() {
	ExtBuilder::default().build().execute_with(|| {
		TestLegacyStaking::set_err(sp_runtime::DispatchError::Other("no position"));

		assert_err!(
			GigaHdx::migrate(RawOrigin::Signed(ALICE).into()),
			sp_runtime::DispatchError::Other("no position")
		);

		// No gigahdx position opened.
		assert!(Stakes::<Test>::get(ALICE).is_none());
		assert_eq!(TotalLocked::<Test>::get(), 0);
	});
}

#[test]
fn migrate_should_fail_when_unlocked_below_min_stake() {
	ExtBuilder::default().build().execute_with(|| {
		// MinStake = ONE; legacy unlocks half that.
		TestLegacyStaking::set_ok(ONE / 2);

		assert_noop!(
			GigaHdx::migrate(RawOrigin::Signed(ALICE).into()),
			Error::<Test>::BelowMinStake
		);
		assert!(Stakes::<Test>::get(ALICE).is_none());
	});
}

#[test]
fn migrate_should_fail_when_external_claim_present_after_unstake() {
	ExtBuilder::default().build().execute_with(|| {
		TestLegacyStaking::set_ok(100 * ONE);
		// Simulate another lock surviving force_unstake (e.g. vesting).
		TestExternalClaims::set(10 * ONE);

		assert_noop!(
			GigaHdx::migrate(RawOrigin::Signed(ALICE).into()),
			Error::<Test>::BlockedByExternalLock
		);
		assert!(Stakes::<Test>::get(ALICE).is_none());
	});
}

#[test]
fn migrate_should_fail_when_unlocked_exceeds_stakeable_free_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE only has 1_000 ONE endowed; pretend legacy reports 2_000.
		TestLegacyStaking::set_ok(2_000 * ONE);

		assert_noop!(
			GigaHdx::migrate(RawOrigin::Signed(ALICE).into()),
			Error::<Test>::InsufficientFreeBalance
		);
	});
}

#[test]
fn migrate_should_top_up_existing_gigahdx_position() {
	ExtBuilder::default().build().execute_with(|| {
		// User already has a gigahdx position.
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		let before = Stakes::<Test>::get(ALICE).unwrap();

		TestLegacyStaking::set_ok(30 * ONE);
		assert_ok!(GigaHdx::migrate(RawOrigin::Signed(ALICE).into()));

		let after = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(after.hdx, before.hdx + 30 * ONE);
		assert_eq!(after.gigahdx, before.gigahdx + 30 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), 80 * ONE);
	});
}
