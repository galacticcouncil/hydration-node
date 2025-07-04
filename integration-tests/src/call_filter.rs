#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{
	assert_ok,
	sp_runtime::{FixedU128, Permill},
	traits::{Contains, StorePreimage},
};
use hydradx_runtime::{origins, Preimage};
use hydradx_traits::router::{AssetPair, PoolType, Route, Trade};
use polkadot_xcm::v3::prelude::*;
use polkadot_xcm::VersionedXcm;
use primitives::constants::currency::UNITS;
use xcm_emulator::TestExt;

#[test]
fn transfer_should_not_work_when_transfering_omnipool_assets_to_omnipool_account() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		init_omnipool();

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		// Act & Assert

		// Balances::transfer
		// transfer to Alice should not be filtered
		let successful_call = hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: ALICE.into(),
			value: 10 * UNITS,
		});
		let filtered_call = hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: omnipool_account.clone(),
			value: 10 * UNITS,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));

		// Balances::transfer_keep_alive
		// transfer to Alice should not be filtered
		let successful_call = hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
			dest: ALICE.into(),
			value: 10 * UNITS,
		});
		let filtered_call = hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
			dest: omnipool_account.clone(),
			value: 10 * UNITS,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));

		// Balances::transfer_all
		// transfer to Alice should not be filtered
		let successful_call = hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_all {
			dest: ALICE.into(),
			keep_alive: true,
		});
		let filtered_call = hydradx_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_all {
			dest: omnipool_account.clone(),
			keep_alive: true,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));

		// Currencies::transfer_native_currency
		// transfer to Alice should not be filtered
		let successful_call =
			hydradx_runtime::RuntimeCall::Currencies(pallet_currencies::Call::transfer_native_currency {
				dest: ALICE.into(),
				amount: 10 * UNITS,
			});
		let filtered_call =
			hydradx_runtime::RuntimeCall::Currencies(pallet_currencies::Call::transfer_native_currency {
				dest: omnipool_account.clone(),
				amount: 10 * UNITS,
			});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call));

		// Currencies::transfer
		// transfer to Alice should not be filtered
		let successful_call_alice = hydradx_runtime::RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: ALICE.into(),
			currency_id: DOT,
			amount: 10 * UNITS,
		});
		// transfer of a token that's not registered in omnipool should not be filtered
		let successful_call = hydradx_runtime::RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: ETH,
			amount: 10 * UNITS,
		});
		let filtered_call_lrna = hydradx_runtime::RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			amount: 10 * UNITS,
		});
		let filtered_call_dot = hydradx_runtime::RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: DOT,
			amount: 10 * UNITS,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call_alice));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_lrna));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_dot));

		// Tokens::transfer
		// transfer to Alice should not be filtered
		let successful_call_alice = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: ALICE.into(),
			currency_id: DOT,
			amount: 10 * UNITS,
		});
		let successful_call = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: ETH,
			amount: 10 * UNITS,
		});
		let filtered_call_lrna = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			amount: 10 * UNITS,
		});
		let filtered_call_dot = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: omnipool_account.clone(),
			currency_id: DOT,
			amount: 10 * UNITS,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call_alice));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_lrna));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_dot));

		// Tokens::transfer_keep_alive
		// transfer to Alice should not be filtered
		let successful_call_alice = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_keep_alive {
			dest: ALICE.into(),
			currency_id: DOT,
			amount: 10 * UNITS,
		});
		let successful_call = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_keep_alive {
			dest: omnipool_account.clone(),
			currency_id: ETH,
			amount: 10 * UNITS,
		});
		let filtered_call_lrna = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_keep_alive {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			amount: 10 * UNITS,
		});
		let filtered_call_dot = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_keep_alive {
			dest: omnipool_account.clone(),
			currency_id: DOT,
			amount: 10 * UNITS,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call_alice));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_dot));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_lrna));

		// Tokens::transfer_all
		// transfer to Alice should not be filtered
		let successful_call_alice = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_all {
			dest: ALICE.into(),
			currency_id: DOT,
			keep_alive: true,
		});
		let successful_call = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_all {
			dest: omnipool_account.clone(),
			currency_id: ETH,
			keep_alive: true,
		});
		let filtered_call_lrna = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_all {
			dest: omnipool_account.clone(),
			currency_id: LRNA,
			keep_alive: true,
		});
		let filtered_call_dot = hydradx_runtime::RuntimeCall::Tokens(orml_tokens::Call::transfer_all {
			dest: omnipool_account,
			currency_id: DOT,
			keep_alive: true,
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call_alice));
		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_lrna));
		assert!(!hydradx_runtime::CallFilter::contains(&filtered_call_dot));
	});
}

#[test]
fn xyk_create_pool_with_lrna_should_be_filtered_by_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::XYK(pallet_xyk::Call::create_pool {
			asset_a: LRNA,
			amount_a: UNITS,
			asset_b: DOT,
			amount_b: UNITS,
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn calling_pallet_xcm_send_extrinsic_should_not_be_filtered_by_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::PolkadotXcm(pallet_xcm::Call::send {
			dest: Box::new(MultiLocation::parent().into_versioned()),
			message: Box::new(VersionedXcm::from(Xcm(vec![]))),
		});

		assert!(hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn calling_orml_xcm_extrinsic_should_be_filtered_by_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::OrmlXcm(orml_xcm::Call::send_as_sovereign {
			dest: Box::new(MultiLocation::parent().into_versioned()),
			message: Box::new(VersionedXcm::from(Xcm(vec![]))),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn create_contract_from_evm_pallet_should_be_filtered_by_call_filter() {
	use sp_core::{H160, H256, U256};

	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::EVM(pallet_evm::Call::create {
			source: H160::default(),
			init: vec![0, 1, 1, 0],
			value: U256::zero(),
			gas_limit: 1000000,
			max_fee_per_gas: U256::from(100000u64),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: [].into(),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));

		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::EVM(pallet_evm::Call::create2 {
			source: H160::default(),
			init: vec![0, 1, 1, 0],
			salt: H256::zero(),
			value: U256::zero(),
			gas_limit: 1000000,
			max_fee_per_gas: U256::from(100000u64),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: [].into(),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn referenda_can_not_be_filtered() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		// Try to pause transactions for Referenda/submit
		assert_ok!(hydradx_runtime::TransactionPause::pause_transaction(
			hydradx_runtime::RuntimeOrigin::root(),
			b"Referenda".to_vec(),
			b"submit".to_vec()
		));

		// Prepare a Referenda/submit call
		let spend_call = hydradx_runtime::RuntimeCall::Treasury(pallet_treasury::Call::spend_local {
			amount: 100 * UNITS,
			beneficiary: ALICE.into(),
		});
		let preimage = <Preimage as StorePreimage>::bound(spend_call).unwrap();

		// Act & Assert
		let successful_call = hydradx_runtime::RuntimeCall::Referenda(pallet_referenda::Call::submit {
			proposal_origin: Box::new(hydradx_runtime::OriginCaller::Origins(origins::Origin::Tipper)),
			proposal: preimage,
			enactment_moment: frame_support::traits::schedule::DispatchTime::After(100),
		});

		assert!(hydradx_runtime::CallFilter::contains(&successful_call));
	});
}

#[test]
fn burn_nft_should_be_filtered_out() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::Uniques(pallet_uniques::Call::burn {
			collection: 2222,
			item: 1,
			check_owner: None,
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}

#[test]
fn router_set_route_should_be_filtered_out() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// the values here don't need to make sense, all we need is a valid Call
		let call = hydradx_runtime::RuntimeCall::Router(pallet_route_executor::Call::set_route {
			asset_pair: AssetPair::new(LRNA, DOT),
			new_route: vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: LRNA,
				asset_out: DOT,
			}]
			.try_into()
			.unwrap(),
		});

		assert!(!hydradx_runtime::CallFilter::contains(&call));
	});
}
