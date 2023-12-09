#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{assert_ok, traits::Contains};

use hydradx_runtime::RuntimeOrigin;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use xcm_emulator::TestExt;

#[test]
fn dust_removal_whitelist_should_work_with_duster() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Make sure account is not in whitelist
		assert!(!hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));

		//Act add account to whitelist
		assert_ok!(hydradx_runtime::Duster::add_account(&ALICE.into()));
		//Assert - account should be in the whitelist
		assert!(hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));

		//Act remove account from whitelist
		assert_ok!(hydradx_runtime::Duster::remove_account(&ALICE.into()));
		//Assert - account should NOT be in the whitelist
		assert!(!hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));
	});
}

#[test]
fn add_nondustable_account_should_work_with_dust_removal_whitelist() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Make sure account is not in whitelist
		assert!(!hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));

		//Act add account to whitelist
		assert_ok!(hydradx_runtime::Duster::add_nondustable_account(
			RuntimeOrigin::root(),
			ALICE.into()
		));

		assert!(hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));
	});
}

#[test]
fn remove_nondustable_account_should_work_with_dust_removal_whitelist() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange - add account to whitelist
		assert_ok!(hydradx_runtime::Duster::add_account(&ALICE.into()));
		assert!(hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));

		//Act
		assert_ok!(hydradx_runtime::Duster::remove_nondustable_account(
			RuntimeOrigin::root(),
			ALICE.into()
		));

		//Assert - account should not be in the whitelist
		assert!(!hydradx_runtime::DustRemovalWhitelist::contains(&ALICE.into()));
	});
}
