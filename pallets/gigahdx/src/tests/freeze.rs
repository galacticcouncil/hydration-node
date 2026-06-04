// SPDX-License-Identifier: Apache-2.0

//! `giga_unstake` voting-commitment guard. `frozen` is no longer stored on the
//! stake — the committed amount is pulled lazily from `T::VotingCommitment`
//! (the rewards pallet in production; `TestVotingCommitment` here).

use super::mock::*;
use crate::{Error, Stakes};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;

fn stake_alice_100() {
	assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
}

#[test]
fn giga_unstake_should_fail_when_below_committed_amount() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		TestVotingCommitment::set(80 * ONE);

		// unstaking everything would drop hdx to 0 < committed=80 → blocked.
		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::StakeFrozen
		);

		// stake state untouched
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 100 * ONE);
	});
}

#[test]
fn giga_unstake_should_succeed_when_payout_keeps_hdx_above_committed() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		TestVotingCommitment::set(30 * ONE);

		// unstaking 50 leaves hdx=50 ≥ committed=30 → ok.
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 50 * ONE);
	});
}

#[test]
fn giga_unstake_should_succeed_after_commitment_cleared() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		TestVotingCommitment::set(80 * ONE);

		assert_noop!(
			GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE),
			Error::<Test>::StakeFrozen
		);

		TestVotingCommitment::set(0);
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
	});
}

#[test]
fn giga_unstake_should_succeed_when_no_commitment() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		// committed defaults to 0 → full unstake allowed.
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
	});
}
