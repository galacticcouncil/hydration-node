use crate::{
	tests::{
		new_test_ext,
		utils::{bounded_array, bounded_err, bounded_sig, bounded_u8, create_test_signature},
		RuntimeCall, RuntimeOrigin, Signet, System, Test,
	},
	Error, ErrorResponse, Event, SignerCount, Signers, MAX_SIGNERS,
};
use frame_support::dispatch::{GetDispatchInfo, Pays};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

const SIGNER: u64 = 1;
const OUTSIDER: u64 = 2;

// -----------------------------------------------------------------------------
// add_signer / remove_signer
// -----------------------------------------------------------------------------

#[test]
fn add_signer_should_authorize_account_when_called_by_update_origin() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));

		assert!(Signers::<Test>::contains_key(SIGNER));
		assert_eq!(SignerCount::<Test>::get(), 1);
		System::assert_last_event(Event::SignerAdded { who: SIGNER }.into());
	});
}

#[test]
fn add_signer_should_fail_when_origin_is_not_update_origin() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::add_signer(RuntimeOrigin::signed(OUTSIDER), SIGNER),
			DispatchError::BadOrigin
		);
		assert!(!Signers::<Test>::contains_key(SIGNER));
	});
}

#[test]
fn add_signer_should_fail_when_account_already_authorized() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));

		assert_noop!(
			Signet::add_signer(RuntimeOrigin::root(), SIGNER),
			Error::<Test>::SignerAlreadyExists
		);
		assert_eq!(SignerCount::<Test>::get(), 1);
	});
}

#[test]
fn add_signer_should_fail_when_max_signers_reached() {
	new_test_ext().execute_with(|| {
		for i in 0..MAX_SIGNERS as u64 {
			assert_ok!(Signet::add_signer(RuntimeOrigin::root(), 1000 + i));
		}
		assert_eq!(SignerCount::<Test>::get(), MAX_SIGNERS);

		assert_noop!(
			Signet::add_signer(RuntimeOrigin::root(), 9999),
			Error::<Test>::TooManySigners
		);
		assert!(!Signers::<Test>::contains_key(9999u64));
	});
}

#[test]
fn remove_signer_should_deauthorize_account_when_authorized() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));

		assert_ok!(Signet::remove_signer(RuntimeOrigin::root(), SIGNER));

		assert!(!Signers::<Test>::contains_key(SIGNER));
		assert_eq!(SignerCount::<Test>::get(), 0);
		System::assert_last_event(Event::SignerRemoved { who: SIGNER }.into());
	});
}

#[test]
fn remove_signer_should_fail_when_account_not_authorized() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::remove_signer(RuntimeOrigin::root(), SIGNER),
			Error::<Test>::SignerNotFound
		);
	});
}

#[test]
fn remove_signer_should_fail_when_origin_is_not_update_origin() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));

		assert_noop!(
			Signet::remove_signer(RuntimeOrigin::signed(OUTSIDER), SIGNER),
			DispatchError::BadOrigin
		);
		assert!(Signers::<Test>::contains_key(SIGNER));
	});
}

#[test]
fn signer_count_should_track_additions_and_removals() {
	new_test_ext().execute_with(|| {
		assert_eq!(SignerCount::<Test>::get(), 0);

		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), OUTSIDER));
		assert_eq!(SignerCount::<Test>::get(), 2);

		assert_ok!(Signet::remove_signer(RuntimeOrigin::root(), SIGNER));
		assert_eq!(SignerCount::<Test>::get(), 1);
	});
}

// -----------------------------------------------------------------------------
// respond authorization
// -----------------------------------------------------------------------------

#[test]
fn respond_should_succeed_when_caller_is_authorized_signer() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));
		let request_id = [7u8; 32];
		let signature = create_test_signature();

		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(SIGNER),
			bounded_array::<100>(vec![request_id]),
			bounded_sig::<100>(vec![signature.clone()])
		));

		System::assert_last_event(
			Event::SignatureResponded {
				request_id,
				responder: SIGNER,
				signature,
			}
			.into(),
		);
	});
}

#[test]
fn respond_should_fail_when_caller_is_not_authorized() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::respond(
				RuntimeOrigin::signed(OUTSIDER),
				bounded_array::<100>(vec![[1u8; 32]]),
				bounded_sig::<100>(vec![create_test_signature()])
			),
			Error::<Test>::NotAuthorizedSigner
		);
	});
}

#[test]
fn respond_error_should_fail_when_caller_is_not_authorized() {
	new_test_ext().execute_with(|| {
		let error_response = ErrorResponse {
			request_id: [1u8; 32],
			error_message: bounded_u8::<1024>(b"boom".to_vec()),
		};

		assert_noop!(
			Signet::respond_error(
				RuntimeOrigin::signed(OUTSIDER),
				bounded_err::<100>(vec![error_response])
			),
			Error::<Test>::NotAuthorizedSigner
		);
	});
}

#[test]
fn respond_bidirectional_should_fail_when_caller_is_not_authorized() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::respond_bidirectional(
				RuntimeOrigin::signed(OUTSIDER),
				[1u8; 32],
				bounded_u8::<65536>(b"out".to_vec()),
				create_test_signature()
			),
			Error::<Test>::NotAuthorizedSigner
		);
	});
}

#[test]
fn respond_should_fail_when_signer_is_removed() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::add_signer(RuntimeOrigin::root(), SIGNER));

		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(SIGNER),
			bounded_array::<100>(vec![[1u8; 32]]),
			bounded_sig::<100>(vec![create_test_signature()])
		));

		assert_ok!(Signet::remove_signer(RuntimeOrigin::root(), SIGNER));

		assert_noop!(
			Signet::respond(
				RuntimeOrigin::signed(SIGNER),
				bounded_array::<100>(vec![[2u8; 32]]),
				bounded_sig::<100>(vec![create_test_signature()])
			),
			Error::<Test>::NotAuthorizedSigner
		);
	});
}

// -----------------------------------------------------------------------------
// feeless (Pays::No) annotation
// -----------------------------------------------------------------------------

#[test]
fn respond_should_be_feeless_when_called() {
	new_test_ext().execute_with(|| {
		let call = RuntimeCall::Signet(crate::Call::respond {
			request_ids: bounded_array::<100>(vec![[1u8; 32]]),
			signatures: bounded_sig::<100>(vec![create_test_signature()]),
		});
		assert_eq!(call.get_dispatch_info().pays_fee, Pays::No);
	});
}

#[test]
fn respond_error_should_be_feeless_when_called() {
	new_test_ext().execute_with(|| {
		let call = RuntimeCall::Signet(crate::Call::respond_error {
			errors: bounded_err::<100>(vec![ErrorResponse {
				request_id: [1u8; 32],
				error_message: bounded_u8::<1024>(b"boom".to_vec()),
			}]),
		});
		assert_eq!(call.get_dispatch_info().pays_fee, Pays::No);
	});
}

#[test]
fn respond_bidirectional_should_be_feeless_when_called() {
	new_test_ext().execute_with(|| {
		let call = RuntimeCall::Signet(crate::Call::respond_bidirectional {
			request_id: [1u8; 32],
			serialized_output: bounded_u8::<65536>(b"out".to_vec()),
			signature: create_test_signature(),
		});
		assert_eq!(call.get_dispatch_info().pays_fee, Pays::No);
	});
}

// -----------------------------------------------------------------------------
// admin calls remain fee-paying
// -----------------------------------------------------------------------------

#[test]
fn add_signer_should_pay_fee_when_called() {
	new_test_ext().execute_with(|| {
		let call = RuntimeCall::Signet(crate::Call::add_signer { who: SIGNER });
		assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
	});
}
