// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, GigaHdxPoolContract};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use primitives::EvmAddress;

#[test]
fn set_pool_contract_should_succeed_when_no_supply() {
	ExtBuilder::default().build().execute_with(|| {
		let new_pool = EvmAddress::from([0xCCu8; 20]);
		assert_ok!(GigaHdx::set_pool_contract(RawOrigin::Root.into(), new_pool));
		assert_eq!(GigaHdxPoolContract::<Test>::get(), Some(new_pool));
	});
}

#[test]
fn set_pool_contract_should_fail_when_active_stake_exists() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_noop!(
			GigaHdx::set_pool_contract(RawOrigin::Root.into(), EvmAddress::from([0xCCu8; 20])),
			Error::<Test>::OutstandingStake
		);
	});
}

#[test]
fn set_pool_contract_should_fail_when_only_residual_gigahdx_exists() {
	// Regression: when an unstake payout exceeds active stake, `Stakes.hdx`
	// can land at 0 (so `TotalLocked == 0`) while `Stakes.gigahdx > 0` —
	// those atokens are still bound to the current pool. Switching pools
	// then would orphan them.
	ExtBuilder::default()
		.with_pot_balance(200 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			// rate = 3.0; unstake 90 → active drained, gigahdx residue 10.
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 90 * ONE));

			assert_eq!(crate::TotalLocked::<Test>::get(), 0);
			assert!(GigaHdx::total_gigahdx_supply() > 0);

			assert_noop!(
				GigaHdx::set_pool_contract(RawOrigin::Root.into(), EvmAddress::from([0xCCu8; 20])),
				Error::<Test>::OutstandingStake
			);
		});
}

#[test]
fn set_pool_contract_should_fail_when_called_by_non_authority() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(GigaHdx::set_pool_contract(RawOrigin::Signed(ALICE).into(), EvmAddress::from([0xCCu8; 20]),).is_err());
	});
}
