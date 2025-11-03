#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{assert_ok, traits::Contains};
use pallet_duster::DusterWhitelist;

use hydradx_runtime::RuntimeOrigin;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use xcm_emulator::TestExt;

#[test]
fn dust_removal_whitelist_should_work_with_duster() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Make sure account is not in whitelist
		assert!(!DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));

		//Act add account to whitelist
		assert_ok!(hydradx_runtime::Duster::add_account(&ALICE.into()));
		//Assert - account should be in the whitelist
		assert!(DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));

		//Act remove account from whitelist
		assert_ok!(hydradx_runtime::Duster::remove_account(&ALICE.into()));
		//Assert - account should NOT be in the whitelist
		assert!(!DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));
	});
}

#[test]
fn whitelist_account_should_work_with_dust_removal_whitelist() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Make sure account is not in whitelist
		assert!(!DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));

		//Act add account to whitelist
		assert_ok!(hydradx_runtime::Duster::whitelist_account(
			RuntimeOrigin::root(),
			ALICE.into()
		));

		assert!(DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));
	});
}

#[test]
fn remove_from_whitelist_should_work_with_dust_removal_whitelist() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange - add account to whitelist
		assert_ok!(hydradx_runtime::Duster::add_account(&ALICE.into()));
		assert!(DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));

		//Act
		assert_ok!(hydradx_runtime::Duster::remove_from_whitelist(
			RuntimeOrigin::root(),
			ALICE.into()
		));

		//Assert - account should not be in the whitelist
		assert!(!DusterWhitelist::<hydradx_runtime::Runtime>::contains(&ALICE.into()));
	});
}
