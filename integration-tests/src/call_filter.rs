#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::traits::Contains;
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
