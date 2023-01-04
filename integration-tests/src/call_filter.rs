#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::traits::Contains;
use polkadot_xcm::latest::prelude::*;
use polkadot_xcm::VersionedXcm;
use xcm_emulator::TestExt;

#[test]
fn transfer_should_not_work_when_transfering_lrna_to_omnipool_account() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		// Currencies::transfer
		let successful_call = hydradx_runtime::Call::Currencies(pallet_currencies::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: DAI,
			amount: 10 * UNITS,
		});
		let filtered_call = hydradx_runtime::Call::Currencies(pallet_currencies::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			amount: 10 * UNITS,
		});

		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));

		// Tokens::transfer
		let successful_call = hydradx_runtime::Call::Tokens(orml_tokens::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: DAI,
			amount: 10 * UNITS,
		});
		let filtered_call = hydradx_runtime::Call::Tokens(orml_tokens::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			amount: 10 * UNITS,
		});

		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));

		// Tokens::transfer_keep_alive
		let successful_call = hydradx_runtime::Call::Tokens(orml_tokens::Call::transfer_keep_alive {
			dest: omnipool_account.clone(),
			currency_id: DAI,
			amount: 10 * UNITS,
		});
		let filtered_call = hydradx_runtime::Call::Tokens(orml_tokens::Call::transfer_keep_alive {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			amount: 10 * UNITS,
		});

		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));

		// Tokens::transfer_all
		let successful_call = hydradx_runtime::Call::Tokens(orml_tokens::Call::transfer_all {
			dest: omnipool_account.clone(),
			currency_id: DAI,
			keep_alive: true,
		});
		let filtered_call = hydradx_runtime::Call::Tokens(orml_tokens::Call::transfer_all {
			dest: omnipool_account,
			currency_id: LRNA,
			keep_alive: true,
		});

		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
	});
}

#[test]
fn calling_pallet_uniques_extrinsic_should_be_filtered_by_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let call = hydradx_runtime::Call::Uniques(pallet_uniques::Call::create {
			collection: 1u128,
			admin: AccountId::from(ALICE),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn calling_pallet_xcm_extrinsic_should_be_filtered_by_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::Call::PolkadotXcm(pallet_xcm::Call::send {
			dest: Box::new(MultiLocation::parent().into()),
			message: Box::new(VersionedXcm::from(Xcm(vec![]))),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn calling_orml_xcm_extrinsic_should_be_filtered_by_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::Call::OrmlXcm(orml_xcm::Call::send_as_sovereign {
			dest: Box::new(MultiLocation::parent().into()),
			message: Box::new(VersionedXcm::from(Xcm(vec![]))),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}
