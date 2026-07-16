#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_runtime::*;
use pretty_assertions::assert_eq;
use xcm_emulator::TestExt;

const DEPOSIT: Balance = 100 * UNITS;
const CHAIN_ID: &[u8] = b"eip155:11155111";
const MAX_CHAIN_ID_LENGTH: u32 = 128;
const MAX_EVM_DATA_LENGTH: u32 = 100_000;

type SignetError = pallet_signet::Error<hydradx_runtime::Runtime>;

fn configure_signet(deposit: Balance) {
	assert_ok!(Signet::set_config(
		RuntimeOrigin::root(),
		deposit,
		MAX_CHAIN_ID_LENGTH,
		MAX_EVM_DATA_LENGTH,
		BoundedVec::truncate_from(CHAIN_ID.to_vec()),
	));
}

fn test_signature() -> pallet_signet::Signature {
	pallet_signet::Signature {
		big_r: pallet_signet::AffinePoint {
			x: [1u8; 32],
			y: [2u8; 32],
		},
		s: [3u8; 32],
		recovery_id: 0,
	}
}

fn sign_call(who: [u8; 32]) -> frame_support::pallet_prelude::DispatchResult {
	Signet::sign(
		RuntimeOrigin::signed(who.into()),
		[42u8; 32],
		1,
		BoundedVec::truncate_from(b"path".to_vec()),
		BoundedVec::truncate_from(b"ecdsa".to_vec()),
		BoundedVec::truncate_from(b"dest".to_vec()),
		BoundedVec::truncate_from(b"{}".to_vec()),
	)
}

// -----------------------------------------------------------------------------
// set_config
// -----------------------------------------------------------------------------

#[test]
fn set_config_should_store_configuration_when_called_by_root() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_eq!(Signet::signet_config(), None);

		configure_signet(DEPOSIT);

		let config = Signet::signet_config().unwrap();
		assert_eq!(config.signature_deposit, DEPOSIT);
		assert_eq!(config.max_chain_id_length, MAX_CHAIN_ID_LENGTH);
		assert_eq!(config.max_evm_data_length, MAX_EVM_DATA_LENGTH);
		assert_eq!(config.chain_id.to_vec(), CHAIN_ID.to_vec());
		assert!(!config.paused);
	});
}

#[test]
fn set_config_should_fail_when_called_by_non_authority() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			Signet::set_config(
				RuntimeOrigin::signed(ALICE.into()),
				DEPOSIT,
				MAX_CHAIN_ID_LENGTH,
				MAX_EVM_DATA_LENGTH,
				BoundedVec::truncate_from(CHAIN_ID.to_vec()),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_config_should_preserve_paused_flag_when_reconfigured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);
		assert_ok!(Signet::pause(RuntimeOrigin::root()));
		assert!(Signet::signet_config().unwrap().paused);

		configure_signet(2 * DEPOSIT);

		let config = Signet::signet_config().unwrap();
		assert_eq!(config.signature_deposit, 2 * DEPOSIT);
		assert!(config.paused);
	});
}

// -----------------------------------------------------------------------------
// pause / unpause
// -----------------------------------------------------------------------------

#[test]
fn pause_should_set_paused_flag_when_called_by_root() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);

		assert_ok!(Signet::pause(RuntimeOrigin::root()));

		assert!(Signet::signet_config().unwrap().paused);
	});
}

#[test]
fn unpause_should_clear_paused_flag_when_called_by_root() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);
		assert_ok!(Signet::pause(RuntimeOrigin::root()));

		assert_ok!(Signet::unpause(RuntimeOrigin::root()));

		assert!(!Signet::signet_config().unwrap().paused);
	});
}

#[test]
fn pause_should_fail_when_called_by_non_authority() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);

		assert_noop!(
			Signet::pause(RuntimeOrigin::signed(ALICE.into())),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn unpause_should_fail_when_called_by_non_authority() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);
		assert_ok!(Signet::pause(RuntimeOrigin::root()));

		assert_noop!(
			Signet::unpause(RuntimeOrigin::signed(ALICE.into())),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// -----------------------------------------------------------------------------
// sign
// -----------------------------------------------------------------------------

#[test]
fn sign_should_fail_when_not_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(sign_call(ALICE), SignetError::NotConfigured);
	});
}

#[test]
fn sign_should_fail_when_paused() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);
		assert_ok!(Signet::pause(RuntimeOrigin::root()));

		assert_noop!(sign_call(ALICE), SignetError::Paused);
	});
}

#[test]
fn sign_should_transfer_deposit_to_pallet_account_when_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);

		let requester: AccountId = ALICE.into();
		let pallet_account = Signet::account_id();
		let requester_before = Balances::free_balance(&requester);
		let pallet_before = Balances::free_balance(&pallet_account);

		assert_ok!(sign_call(ALICE));

		assert_eq!(Balances::free_balance(&requester), requester_before - DEPOSIT);
		assert_eq!(Balances::free_balance(&pallet_account), pallet_before + DEPOSIT);
	});
}

// -----------------------------------------------------------------------------
// sign_bidirectional
// -----------------------------------------------------------------------------

#[test]
fn sign_bidirectional_should_fail_when_transaction_is_empty() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);

		assert_noop!(
			Signet::sign_bidirectional(
				RuntimeOrigin::signed(ALICE.into()),
				BoundedVec::truncate_from(vec![]),
				BoundedVec::truncate_from(CHAIN_ID.to_vec()),
				1,
				BoundedVec::truncate_from(b"path".to_vec()),
				BoundedVec::truncate_from(b"ecdsa".to_vec()),
				BoundedVec::truncate_from(b"dest".to_vec()),
				BoundedVec::truncate_from(b"{}".to_vec()),
				BoundedVec::truncate_from(vec![]),
				BoundedVec::truncate_from(vec![]),
			),
			SignetError::InvalidTransaction
		);
	});
}

#[test]
fn sign_bidirectional_should_fail_when_not_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			Signet::sign_bidirectional(
				RuntimeOrigin::signed(ALICE.into()),
				BoundedVec::truncate_from(b"tx".to_vec()),
				BoundedVec::truncate_from(CHAIN_ID.to_vec()),
				1,
				BoundedVec::truncate_from(b"path".to_vec()),
				BoundedVec::truncate_from(b"ecdsa".to_vec()),
				BoundedVec::truncate_from(b"dest".to_vec()),
				BoundedVec::truncate_from(b"{}".to_vec()),
				BoundedVec::truncate_from(vec![]),
				BoundedVec::truncate_from(vec![]),
			),
			SignetError::NotConfigured
		);
	});
}

// -----------------------------------------------------------------------------
// withdraw_funds
// -----------------------------------------------------------------------------

#[test]
fn withdraw_funds_should_fail_when_called_by_non_authority() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			Signet::withdraw_funds(RuntimeOrigin::signed(ALICE.into()), BOB.into(), DEPOSIT),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn withdraw_funds_should_fail_when_insufficient_funds() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			Signet::withdraw_funds(RuntimeOrigin::root(), BOB.into(), DEPOSIT),
			SignetError::InsufficientFunds
		);
	});
}

#[test]
fn withdraw_funds_should_transfer_to_recipient_when_funds_available() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_signet(DEPOSIT);
		// Fund the pallet account deterministically by making a signature request.
		assert_ok!(sign_call(ALICE));

		let recipient: AccountId = BOB.into();
		let pallet_account = Signet::account_id();
		let recipient_before = Balances::free_balance(&recipient);
		let pallet_before = Balances::free_balance(&pallet_account);
		assert!(pallet_before >= DEPOSIT);

		assert_ok!(Signet::withdraw_funds(
			RuntimeOrigin::root(),
			recipient.clone(),
			DEPOSIT
		));

		assert_eq!(Balances::free_balance(&recipient), recipient_before + DEPOSIT);
		assert_eq!(Balances::free_balance(&pallet_account), pallet_before - DEPOSIT);
	});
}

// -----------------------------------------------------------------------------
// respond / respond_error / respond_bidirectional
// -----------------------------------------------------------------------------

#[test]
fn respond_should_emit_response_event_when_signed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let responder: AccountId = ALICE.into();
		let request_id = [99u8; 32];
		let signature = test_signature();

		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(responder.clone()),
			BoundedVec::truncate_from(vec![request_id]),
			BoundedVec::truncate_from(vec![signature.clone()]),
		));

		expect_hydra_events(vec![pallet_signet::Event::SignatureResponded {
			request_id,
			responder,
			signature,
		}
		.into()]);
	});
}

#[test]
fn respond_should_fail_when_arrays_length_mismatch() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			Signet::respond(
				RuntimeOrigin::signed(ALICE.into()),
				BoundedVec::truncate_from(vec![[1u8; 32], [2u8; 32]]),
				BoundedVec::truncate_from(vec![test_signature()]),
			),
			SignetError::InvalidInputLength
		);
	});
}

#[test]
fn respond_error_should_emit_error_event_when_signed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let responder: AccountId = ALICE.into();
		let request_id = [7u8; 32];
		let message = b"signature generation failed".to_vec();

		assert_ok!(Signet::respond_error(
			RuntimeOrigin::signed(responder.clone()),
			BoundedVec::truncate_from(vec![pallet_signet::ErrorResponse {
				request_id,
				error_message: BoundedVec::truncate_from(message.clone()),
			}]),
		));

		expect_hydra_events(vec![pallet_signet::Event::SignatureError {
			request_id,
			responder,
			error: message,
		}
		.into()]);
	});
}

#[test]
fn respond_bidirectional_should_emit_event_when_signed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let responder: AccountId = ALICE.into();
		let request_id = [11u8; 32];
		let output = b"read_output".to_vec();
		let signature = test_signature();

		assert_ok!(Signet::respond_bidirectional(
			RuntimeOrigin::signed(responder.clone()),
			request_id,
			BoundedVec::truncate_from(output.clone()),
			signature.clone(),
		));

		expect_hydra_events(vec![pallet_signet::Event::RespondBidirectionalEvent {
			request_id,
			responder,
			serialized_output: output,
			signature,
		}
		.into()]);
	});
}
