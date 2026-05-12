// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Stakes};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;

fn stake_alice_100() {
	assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
}

#[test]
fn freeze_should_create_stake_record_when_user_had_none() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(Stakes::<Test>::get(ALICE).is_none());

		GigaHdx::freeze(&ALICE, 10 * ONE);

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 0);
		assert_eq!(s.gigahdx, 0);
		assert_eq!(s.frozen, 10 * ONE);
	});
}

#[test]
fn freeze_should_be_additive_when_called_repeatedly() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		GigaHdx::freeze(&ALICE, 30 * ONE);
		GigaHdx::freeze(&ALICE, 20 * ONE);

		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().frozen, 50 * ONE);
	});
}

#[test]
fn freeze_should_noop_when_delta_zero() {
	ExtBuilder::default().build().execute_with(|| {
		GigaHdx::freeze(&ALICE, 0);
		assert!(Stakes::<Test>::get(ALICE).is_none());
	});
}

#[test]
fn unfreeze_should_saturate_when_delta_exceeds_frozen() {
	ExtBuilder::default().build().execute_with(|| {
		GigaHdx::freeze(&ALICE, 10 * ONE);
		GigaHdx::unfreeze(&ALICE, 100 * ONE);

		// saturating_sub clamped to 0; record gets cleaned up (all fields zero).
		assert!(Stakes::<Test>::get(ALICE).is_none());
	});
}

#[test]
fn unfreeze_should_remove_record_when_all_fields_zero() {
	ExtBuilder::default().build().execute_with(|| {
		GigaHdx::freeze(&ALICE, 10 * ONE);
		assert!(Stakes::<Test>::get(ALICE).is_some());

		GigaHdx::unfreeze(&ALICE, 10 * ONE);
		assert!(Stakes::<Test>::get(ALICE).is_none());
	});
}

#[test]
fn unfreeze_should_keep_record_when_hdx_or_gigahdx_nonzero() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		GigaHdx::freeze(&ALICE, 10 * ONE);
		GigaHdx::unfreeze(&ALICE, 10 * ONE);

		// hdx and gigahdx still active → record persists with frozen=0.
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.frozen, 0);
	});
}

#[test]
fn giga_unstake_should_fail_when_below_frozen_amount() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		GigaHdx::freeze(&ALICE, 80 * ONE);

		// unstaking everything would drop hdx to 0 < frozen=80 → blocked.
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::StakeFrozen
		);

		// stake state untouched
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.frozen, 80 * ONE);
	});
}

#[test]
fn giga_unstake_should_succeed_when_payout_keeps_hdx_above_frozen() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		GigaHdx::freeze(&ALICE, 30 * ONE);

		// unstaking 50 leaves hdx=50 ≥ frozen=30 → ok.
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 50 * ONE);
		assert_eq!(s.frozen, 30 * ONE);
	});
}

#[test]
fn giga_unstake_should_succeed_after_unfreeze_releases_stake() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		GigaHdx::freeze(&ALICE, 80 * ONE);

		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::StakeFrozen
		);

		GigaHdx::unfreeze(&ALICE, 80 * ONE);
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
	});
}
