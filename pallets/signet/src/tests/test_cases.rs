use crate::{
	tests::{
		new_test_ext,
		utils::{bounded_array, bounded_err, bounded_sig, bounded_u8, create_test_signature},
		Balances, MockCaller, MockCallerPalletId, RuntimeEvent, RuntimeOrigin, Signet, System, Test,
	},
	Error, ErrorResponse, Event,
};
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::AccountIdConversion;

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const REQUESTER: u64 = 1;
const OTHER_USER: u64 = 2;
const POOR_USER: u64 = 3;

const INITIAL_DEPOSIT: u128 = 100;

const WITHDRAW_AMOUNT: u128 = 5_000;
const PALLET_INITIAL_BALANCE: u128 = 10_000;

const CAIP2_SEPOLIA: &[u8] = b"eip155:11155111";

const TEST_CHAIN_ID_BYTES: &[u8] = b"test-chain";
const HYDRADX_CHAIN_ID_BYTES: &[u8] = b"hydradx:polkadot:0";

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Configure Signet with default test values.
fn configure_signet(deposit: u128) {
	assert_ok!(Signet::set_config(
		RuntimeOrigin::root(),
		deposit,
		128,
		100_000,
		bounded_u8::<128>(TEST_CHAIN_ID_BYTES.to_vec()),
	));
}

/// Fund the Signet pallet account with the given amount and return its account id.
fn fund_signet_pallet(amount: u128) -> u64 {
	let pallet_account = Signet::account_id();
	let _ = Balances::deposit_creating(&pallet_account, amount);
	pallet_account
}

// -----------------------------------------------------------------------------
// set_config tests
// -----------------------------------------------------------------------------

#[test]
fn test_set_config_works() {
	new_test_ext().execute_with(|| {
		let deposit = INITIAL_DEPOSIT;
		let chain_id = bounded_u8::<128>(TEST_CHAIN_ID_BYTES.to_vec());

		assert_eq!(Signet::signet_config(), None);

		assert_ok!(Signet::set_config(
			RuntimeOrigin::root(),
			deposit,
			128,
			100_000,
			chain_id.clone(),
		));

		let config = Signet::signet_config().unwrap();
		assert_eq!(config.signature_deposit, deposit);
		assert_eq!(config.max_chain_id_length, 128);
		assert_eq!(config.max_evm_data_length, 100_000);
		assert_eq!(config.chain_id.to_vec(), chain_id.to_vec());
		assert!(!config.paused);
	});
}

#[test]
fn test_set_config_can_be_called_multiple_times() {
	new_test_ext().execute_with(|| {
		configure_signet(INITIAL_DEPOSIT);
		let config1 = Signet::signet_config().unwrap();
		assert_eq!(config1.signature_deposit, INITIAL_DEPOSIT);

		// Update config
		assert_ok!(Signet::set_config(
			RuntimeOrigin::root(),
			200,
			128,
			100_000,
			bounded_u8::<128>(TEST_CHAIN_ID_BYTES.to_vec()),
		));
		let config2 = Signet::signet_config().unwrap();
		assert_eq!(config2.signature_deposit, 200);
	});
}

#[test]
fn test_set_config_preserves_paused_state() {
	new_test_ext().execute_with(|| {
		configure_signet(INITIAL_DEPOSIT);

		// Pause
		assert_ok!(Signet::pause(RuntimeOrigin::root()));
		assert!(Signet::signet_config().unwrap().paused);

		// Update config - paused state should be preserved
		assert_ok!(Signet::set_config(
			RuntimeOrigin::root(),
			200,
			128,
			100_000,
			bounded_u8::<128>(TEST_CHAIN_ID_BYTES.to_vec()),
		));

		let config = Signet::signet_config().unwrap();
		assert_eq!(config.signature_deposit, 200);
		assert!(config.paused);
	});
}

#[test]
fn test_cannot_use_before_config() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::sign(
				RuntimeOrigin::signed(REQUESTER),
				[0u8; 32],
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec())
			),
			Error::<Test>::NotConfigured
		);
	});
}

// -----------------------------------------------------------------------------
// Pause / Unpause tests
// -----------------------------------------------------------------------------

#[test]
fn test_pause_unpause_state() {
	new_test_ext().execute_with(|| {
		configure_signet(INITIAL_DEPOSIT);

		assert_ok!(Signet::pause(RuntimeOrigin::root()));
		assert!(Signet::signet_config().unwrap().paused);

		assert_ok!(Signet::unpause(RuntimeOrigin::root()));
		assert!(!Signet::signet_config().unwrap().paused);
	});
}

#[test]
fn test_request_rejected_when_paused() {
	new_test_ext().execute_with(|| {
		configure_signet(INITIAL_DEPOSIT);
		assert_ok!(Signet::pause(RuntimeOrigin::root()));

		assert_noop!(
			Signet::sign(
				RuntimeOrigin::signed(REQUESTER),
				[0u8; 32],
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec())
			),
			Error::<Test>::Paused
		);
	});
}

// -----------------------------------------------------------------------------
// Withdraw tests
// -----------------------------------------------------------------------------

#[test]
fn test_withdraw_funds() {
	new_test_ext().execute_with(|| {
		let recipient = OTHER_USER;

		let pallet_account = fund_signet_pallet(PALLET_INITIAL_BALANCE);

		let recipient_balance_before = Balances::free_balance(recipient);
		assert_eq!(Balances::free_balance(pallet_account), PALLET_INITIAL_BALANCE);

		assert_ok!(Signet::withdraw_funds(
			RuntimeOrigin::root(),
			recipient,
			WITHDRAW_AMOUNT
		));

		assert_eq!(
			Balances::free_balance(pallet_account),
			PALLET_INITIAL_BALANCE - WITHDRAW_AMOUNT
		);
		assert_eq!(
			Balances::free_balance(recipient),
			recipient_balance_before + WITHDRAW_AMOUNT
		);

		System::assert_last_event(
			Event::FundsWithdrawn {
				amount: WITHDRAW_AMOUNT,
				recipient,
			}
			.into(),
		);
	});
}

#[test]
fn test_cannot_withdraw_more_than_balance() {
	new_test_ext().execute_with(|| {
		let pallet_account = fund_signet_pallet(PALLET_INITIAL_BALANCE);
		assert_eq!(Balances::free_balance(pallet_account), PALLET_INITIAL_BALANCE);

		assert_noop!(
			Signet::withdraw_funds(RuntimeOrigin::root(), REQUESTER, 20_000),
			Error::<Test>::InsufficientFunds
		);
	});
}

// -----------------------------------------------------------------------------
// Sign tests
// -----------------------------------------------------------------------------

#[test]
fn test_pallet_account_id_is_deterministic() {
	new_test_ext().execute_with(|| {
		let account1 = Signet::account_id();
		let account2 = Signet::account_id();
		assert_eq!(account1, account2);

		assert_ne!(account1, REQUESTER);
		assert_ne!(account1, OTHER_USER);
	});
}

#[test]
fn test_sign_request_works() {
	new_test_ext().execute_with(|| {
		let requester = OTHER_USER;
		let deposit = INITIAL_DEPOSIT;

		configure_signet(deposit);

		let balance_before = Balances::free_balance(requester);
		let payload = [42u8; 32];
		let key_version = 1u32;
		let path = bounded_u8::<256>(b"path".to_vec());
		let algo = bounded_u8::<32>(b"ecdsa".to_vec());
		let dest = bounded_u8::<64>(b"callback_contract".to_vec());
		let params = bounded_u8::<1024>(b"{}".to_vec());

		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester),
			payload,
			key_version,
			path.clone(),
			algo.clone(),
			dest.clone(),
			params.clone()
		));

		assert_eq!(Balances::free_balance(requester), balance_before - deposit);
		let pallet_account = Signet::account_id();
		assert_eq!(Balances::free_balance(pallet_account), deposit);

		System::assert_last_event(
			Event::SignatureRequested {
				sender: requester,
				payload,
				key_version,
				deposit,
				chain_id: TEST_CHAIN_ID_BYTES.to_vec(),
				path: path.to_vec(),
				algo: algo.to_vec(),
				dest: dest.to_vec(),
				params: params.to_vec(),
			}
			.into(),
		);
	});
}

#[test]
fn test_sign_request_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let poor_user = POOR_USER;
		let deposit = 100_000u128;

		configure_signet(deposit);

		assert_noop!(
			Signet::sign(
				RuntimeOrigin::signed(poor_user),
				[0u8; 32],
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec())
			),
			sp_runtime::TokenError::FundsUnavailable
		);
	});
}

#[test]
fn test_multiple_sign_requests() {
	new_test_ext().execute_with(|| {
		let requester1 = REQUESTER;
		let requester2 = OTHER_USER;
		let deposit = INITIAL_DEPOSIT;

		configure_signet(deposit);

		let pallet_account = Signet::account_id();

		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester1),
			[1u8; 32],
			1,
			bounded_u8::<256>(b"path1".to_vec()),
			bounded_u8::<32>(b"algo".to_vec()),
			bounded_u8::<64>(b"dest".to_vec()),
			bounded_u8::<1024>(b"params".to_vec())
		));

		assert_eq!(Balances::free_balance(pallet_account), deposit);

		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester2),
			[2u8; 32],
			2,
			bounded_u8::<256>(b"path2".to_vec()),
			bounded_u8::<32>(b"algo".to_vec()),
			bounded_u8::<64>(b"dest".to_vec()),
			bounded_u8::<1024>(b"params".to_vec())
		));

		assert_eq!(Balances::free_balance(pallet_account), deposit * 2);
	});
}

#[test]
fn test_sign_bidirectional_works() {
	new_test_ext().execute_with(|| {
		let requester = OTHER_USER;
		let deposit = INITIAL_DEPOSIT;

		configure_signet(deposit);

		let tx_data = b"mock_transaction_data".to_vec();
		let caip2_id = CAIP2_SEPOLIA;
		let balance_before = Balances::free_balance(requester);

		assert_ok!(Signet::sign_bidirectional(
			RuntimeOrigin::signed(requester),
			bounded_u8::<65536>(tx_data.clone()),
			bounded_u8::<64>(caip2_id.to_vec()),
			1,
			bounded_u8::<256>(b"path".to_vec()),
			bounded_u8::<32>(b"ecdsa".to_vec()),
			bounded_u8::<64>(b"callback".to_vec()),
			bounded_u8::<1024>(b"{}".to_vec()),
			bounded_u8::<4096>(b"schema1".to_vec()),
			bounded_u8::<4096>(b"schema2".to_vec())
		));

		assert_eq!(Balances::free_balance(requester), balance_before - deposit);

		let events = System::events();
		let event_found = events
			.iter()
			.any(|e| matches!(&e.event, RuntimeEvent::Signet(Event::SignBidirectionalRequested { .. })));
		assert!(event_found);
	});
}

#[test]
fn test_sign_bidirectional_empty_transaction_fails() {
	new_test_ext().execute_with(|| {
		let requester = OTHER_USER;

		configure_signet(INITIAL_DEPOSIT);

		assert_noop!(
			Signet::sign_bidirectional(
				RuntimeOrigin::signed(requester),
				bounded_u8::<65536>(vec![]),
				bounded_u8::<64>(CAIP2_SEPOLIA.to_vec()),
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec()),
				bounded_u8::<4096>(vec![]),
				bounded_u8::<4096>(vec![])
			),
			Error::<Test>::InvalidTransaction
		);
	});
}

// -----------------------------------------------------------------------------
// Respond tests
// -----------------------------------------------------------------------------

#[test]
fn test_respond_single() {
	new_test_ext().execute_with(|| {
		let responder = REQUESTER;
		let request_id = [99u8; 32];
		let signature = create_test_signature();

		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(responder),
			bounded_array::<100>(vec![request_id]),
			bounded_sig::<100>(vec![signature.clone()])
		));

		System::assert_last_event(
			Event::SignatureResponded {
				request_id,
				responder,
				signature,
			}
			.into(),
		);
	});
}

#[test]
fn test_respond_batch() {
	new_test_ext().execute_with(|| {
		let responder = REQUESTER;
		let request_ids = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
		let signatures = vec![
			create_test_signature(),
			create_test_signature(),
			create_test_signature(),
		];

		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(responder),
			bounded_array::<100>(request_ids.clone()),
			bounded_sig::<100>(signatures.clone())
		));

		let events = System::events();
		let response_events = events
			.iter()
			.filter(|e| matches!(&e.event, RuntimeEvent::Signet(Event::SignatureResponded { .. })))
			.count();
		assert_eq!(response_events, 3);
	});
}

#[test]
fn test_respond_mismatched_arrays_fails() {
	new_test_ext().execute_with(|| {
		let responder = REQUESTER;

		assert_noop!(
			Signet::respond(
				RuntimeOrigin::signed(responder),
				bounded_array::<100>(vec![[1u8; 32], [2u8; 32]]),
				bounded_sig::<100>(vec![
					create_test_signature(),
					create_test_signature(),
					create_test_signature(),
				])
			),
			Error::<Test>::InvalidInputLength
		);
	});
}

#[test]
fn test_respond_error_single() {
	new_test_ext().execute_with(|| {
		let responder = REQUESTER;
		let error_response = ErrorResponse {
			request_id: [99u8; 32],
			error_message: bounded_u8::<1024>(b"Signature generation failed".to_vec()),
		};

		assert_ok!(Signet::respond_error(
			RuntimeOrigin::signed(responder),
			bounded_err::<100>(vec![error_response])
		));

		System::assert_last_event(
			Event::SignatureError {
				request_id: [99u8; 32],
				responder,
				error: b"Signature generation failed".to_vec(),
			}
			.into(),
		);
	});
}

#[test]
fn test_respond_error_batch() {
	new_test_ext().execute_with(|| {
		let responder = REQUESTER;
		let errors = vec![
			ErrorResponse {
				request_id: [1u8; 32],
				error_message: bounded_u8::<1024>(b"Error 1".to_vec()),
			},
			ErrorResponse {
				request_id: [2u8; 32],
				error_message: bounded_u8::<1024>(b"Error 2".to_vec()),
			},
		];

		assert_ok!(Signet::respond_error(
			RuntimeOrigin::signed(responder),
			bounded_err::<100>(errors)
		));

		let events = System::events();
		let error_events = events
			.iter()
			.filter(|e| matches!(&e.event, RuntimeEvent::Signet(Event::SignatureError { .. })))
			.count();
		assert_eq!(error_events, 2);
	});
}

#[test]
fn test_respond_bidirectional() {
	new_test_ext().execute_with(|| {
		let responder = REQUESTER;
		let request_id = [99u8; 32];
		let output = b"read_output_data".to_vec();
		let signature = create_test_signature();

		assert_ok!(Signet::respond_bidirectional(
			RuntimeOrigin::signed(responder),
			request_id,
			bounded_u8::<65536>(output.clone()),
			signature.clone()
		));

		System::assert_last_event(
			Event::RespondBidirectionalEvent {
				request_id,
				responder,
				serialized_output: output,
				signature,
			}
			.into(),
		);
	});
}

#[test]
fn test_sign_includes_chain_id() {
	new_test_ext().execute_with(|| {
		let requester = OTHER_USER;
		let chain_id_bytes = HYDRADX_CHAIN_ID_BYTES;

		assert_ok!(Signet::set_config(
			RuntimeOrigin::root(),
			INITIAL_DEPOSIT,
			128,
			100_000,
			bounded_u8::<128>(chain_id_bytes.to_vec()),
		));

		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester),
			[42u8; 32],
			1,
			bounded_u8::<256>(b"path".to_vec()),
			bounded_u8::<32>(b"algo".to_vec()),
			bounded_u8::<64>(b"dest".to_vec()),
			bounded_u8::<1024>(b"params".to_vec())
		));

		let events = System::events();
		let sign_event = events.iter().find_map(|e| {
			if let RuntimeEvent::Signet(Event::SignatureRequested {
				chain_id: event_chain_id,
				..
			}) = &e.event
			{
				Some(event_chain_id.clone())
			} else {
				None
			}
		});

		assert_eq!(sign_event, Some(chain_id_bytes.to_vec()));
	});
}

#[test]
fn test_cross_pallet_execution() {
	new_test_ext().execute_with(|| {
		configure_signet(INITIAL_DEPOSIT);

		// Fund the MockCaller pallet's account
		let mock_pallet_account: u64 = MockCallerPalletId::get().into_account_truncating();
		let _ = Balances::deposit_creating(&mock_pallet_account, PALLET_INITIAL_BALANCE);

		// User calls MockCaller, which then calls Signet
		assert_ok!(MockCaller::call_signet(RuntimeOrigin::signed(OTHER_USER)));

		// Check the event - the sender should be the PALLET's account
		System::assert_last_event(
			Event::SignatureRequested {
				sender: mock_pallet_account,
				payload: [99u8; 32],
				key_version: 1,
				deposit: INITIAL_DEPOSIT,
				chain_id: TEST_CHAIN_ID_BYTES.to_vec(),
				path: b"from_pallet".to_vec(),
				algo: b"ecdsa".to_vec(),
				dest: b"".to_vec(),
				params: b"{}".to_vec(),
			}
			.into(),
		);

		// Verify the deposit was taken from the pallet's account
		assert_eq!(
			Balances::free_balance(mock_pallet_account),
			PALLET_INITIAL_BALANCE - INITIAL_DEPOSIT
		);
	});
}
