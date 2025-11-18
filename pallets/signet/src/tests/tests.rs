use crate::{
	tests::{
		new_test_ext,
		utils::{bounded_array, bounded_chain_id, bounded_err, bounded_sig, bounded_u8, create_test_signature},
		Balances, MockCaller, MockCallerPalletId, RuntimeEvent, RuntimeOrigin, Signet, System, Test,
	},
	Error, ErrorResponse, Event, SerializationFormat,
};
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::AccountIdConversion;

#[test]
fn test_initialize_works() {
	new_test_ext().execute_with(|| {
		let admin_account = 1u64;
		let deposit = 1000u128;
		let chain_id = bounded_chain_id(b"test-chain".to_vec());

		assert_eq!(Signet::admin(), None);

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin_account,
			deposit,
			chain_id.clone()
		));

		assert_eq!(Signet::admin(), Some(admin_account));
		assert_eq!(Signet::signature_deposit(), deposit);
		assert_eq!(Signet::chain_id().to_vec(), chain_id.to_vec());

		System::assert_last_event(
			Event::Initialized {
				admin: admin_account,
				signature_deposit: deposit,
				chain_id: chain_id.to_vec(),
			}
			.into(),
		);
	});
}

#[test]
fn test_cannot_initialize_twice() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			1,
			1000,
			bounded_chain_id(b"test-chain".to_vec())
		));

		assert_noop!(
			Signet::initialize(
				RuntimeOrigin::signed(2),
				2,
				2000,
				bounded_chain_id(b"test-chain".to_vec())
			),
			Error::<Test>::AlreadyInitialized
		);
	});
}

#[test]
fn test_cannot_use_before_initialization() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::sign(
				RuntimeOrigin::signed(1),
				[0u8; 32],
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec())
			),
			Error::<Test>::NotInitialized
		);
	});
}

#[test]
fn test_any_signed_can_initialize_once() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(2),
			1,
			1000,
			bounded_chain_id(b"test-chain".to_vec())
		));

		assert_eq!(Signet::admin(), Some(1));
		assert_eq!(Signet::signature_deposit(), 1000);

		assert_noop!(
			Signet::initialize(
				RuntimeOrigin::signed(1),
				3,
				2000,
				bounded_chain_id(b"test-chain".to_vec())
			),
			Error::<Test>::AlreadyInitialized
		);

		assert_noop!(
			Signet::initialize(
				RuntimeOrigin::signed(3),
				3,
				2000,
				bounded_chain_id(b"test-chain".to_vec())
			),
			Error::<Test>::AlreadyInitialized
		);

		assert_eq!(Signet::admin(), Some(1));
		assert_eq!(Signet::signature_deposit(), 1000);
	});
}

#[test]
fn test_initialize_sets_deposit() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let initial_deposit = 1000u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			initial_deposit,
			bounded_chain_id(b"test-chain".to_vec())
		));

		assert_eq!(Signet::signature_deposit(), initial_deposit);

		System::assert_last_event(
			Event::Initialized {
				admin,
				signature_deposit: initial_deposit,
				chain_id: bounded_chain_id(b"test-chain".to_vec()).to_vec(),
			}
			.into(),
		);
	});
}

#[test]
fn test_update_deposit_as_admin() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let initial_deposit = 1000u128;
		let new_deposit = 2000u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			initial_deposit,
			bounded_chain_id(b"test-chain".to_vec())
		));

		assert_ok!(Signet::update_deposit(RuntimeOrigin::signed(admin), new_deposit));
		assert_eq!(Signet::signature_deposit(), new_deposit);

		System::assert_last_event(
			Event::DepositUpdated {
				old_deposit: initial_deposit,
				new_deposit,
			}
			.into(),
		);
	});
}

#[test]
fn test_non_admin_cannot_update_deposit() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let non_admin = 2u64;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			1000,
			bounded_chain_id(b"test-chain".to_vec())
		));

		assert_noop!(
			Signet::update_deposit(RuntimeOrigin::signed(non_admin), 2000),
			Error::<Test>::Unauthorized
		);

		assert_eq!(Signet::signature_deposit(), 1000);
	});
}

#[test]
fn test_cannot_update_deposit_before_initialization() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::update_deposit(RuntimeOrigin::signed(1), 1000),
			Error::<Test>::NotInitialized
		);
	});
}

#[test]
fn test_withdraw_funds_as_admin() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let recipient = 2u64;
		let amount = 5000u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			1000,
			bounded_chain_id(b"test-chain".to_vec())
		));

		let pallet_account = Signet::account_id();
		let _ = Balances::deposit_creating(&pallet_account, 10_000);

		let recipient_balance_before = Balances::free_balance(&recipient);
		assert_eq!(Balances::free_balance(&pallet_account), 10_000);

		assert_ok!(Signet::withdraw_funds(RuntimeOrigin::signed(admin), recipient, amount));

		assert_eq!(Balances::free_balance(&pallet_account), 5_000);
		assert_eq!(Balances::free_balance(&recipient), recipient_balance_before + amount);

		System::assert_last_event(Event::FundsWithdrawn { amount, recipient }.into());
	});
}

#[test]
fn test_non_admin_cannot_withdraw() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let non_admin = 2u64;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			1000,
			bounded_chain_id(b"test-chain".to_vec())
		));

		let pallet_account = Signet::account_id();
		let _ = Balances::deposit_creating(&pallet_account, 10_000);

		assert_noop!(
			Signet::withdraw_funds(RuntimeOrigin::signed(non_admin), non_admin, 5000),
			Error::<Test>::Unauthorized
		);
	});
}

#[test]
fn test_cannot_withdraw_more_than_balance() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			1000,
			bounded_chain_id(b"test-chain".to_vec())
		));

		let pallet_account = Signet::account_id();
		let _ = Balances::deposit_creating(&pallet_account, 10_000);

		assert_noop!(
			Signet::withdraw_funds(RuntimeOrigin::signed(admin), admin, 20_000),
			Error::<Test>::InsufficientFunds
		);
	});
}

#[test]
fn test_pallet_account_id_is_deterministic() {
	new_test_ext().execute_with(|| {
		let account1 = Signet::account_id();
		let account2 = Signet::account_id();
		assert_eq!(account1, account2);

		assert_ne!(account1, 1u64);
		assert_ne!(account1, 2u64);
	});
}

#[test]
fn test_sign_request_works() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let requester = 2u64;
		let deposit = 1000u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			deposit,
			bounded_chain_id(b"test-chain".to_vec())
		));

		let balance_before = Balances::free_balance(&requester);
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

		assert_eq!(Balances::free_balance(&requester), balance_before - deposit);
		let pallet_account = Signet::account_id();
		assert_eq!(Balances::free_balance(&pallet_account), deposit);

		System::assert_last_event(
			Event::SignatureRequested {
				sender: requester,
				payload,
				key_version,
				deposit,
				chain_id: bounded_chain_id(b"test-chain".to_vec()).to_vec(),
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
		let admin = 1u64;
		let poor_user = 3u64;
		let deposit = 1000u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			deposit,
			bounded_chain_id(b"test-chain".to_vec())
		));

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
fn test_sign_request_before_initialization() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::sign(
				RuntimeOrigin::signed(1),
				[0u8; 32],
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec())
			),
			Error::<Test>::NotInitialized
		);
	});
}

#[test]
fn test_multiple_sign_requests() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let requester1 = 1u64;
		let requester2 = 2u64;
		let deposit = 100u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			deposit,
			bounded_chain_id(b"test-chain".to_vec())
		));

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

		assert_eq!(Balances::free_balance(&pallet_account), deposit);

		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester2),
			[2u8; 32],
			2,
			bounded_u8::<256>(b"path2".to_vec()),
			bounded_u8::<32>(b"algo".to_vec()),
			bounded_u8::<64>(b"dest".to_vec()),
			bounded_u8::<1024>(b"params".to_vec())
		));

		assert_eq!(Balances::free_balance(&pallet_account), deposit * 2);
	});
}

#[test]
fn test_sign_respond_works() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let requester = 2u64;
		let deposit = 100u128;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			deposit,
			bounded_chain_id(b"test-chain".to_vec())
		));

		let tx_data = b"mock_transaction_data".to_vec();
		let slip44_chain_id = 60u32;
		let balance_before = Balances::free_balance(&requester);

		assert_ok!(Signet::sign_respond(
			RuntimeOrigin::signed(requester),
			bounded_u8::<65536>(tx_data.clone()),
			slip44_chain_id,
			1,
			bounded_u8::<256>(b"path".to_vec()),
			bounded_u8::<32>(b"ecdsa".to_vec()),
			bounded_u8::<64>(b"callback".to_vec()),
			bounded_u8::<1024>(b"{}".to_vec()),
			SerializationFormat::AbiJson,
			bounded_u8::<4096>(b"schema1".to_vec()),
			SerializationFormat::Borsh,
			bounded_u8::<4096>(b"schema2".to_vec())
		));

		assert_eq!(Balances::free_balance(&requester), balance_before - deposit);

		let events = System::events();
		let event_found = events
			.iter()
			.any(|e| matches!(&e.event, RuntimeEvent::Signet(Event::SignRespondRequested { .. })));
		assert!(event_found);
	});
}

#[test]
fn test_sign_respond_empty_transaction_fails() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let requester = 2u64;

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			100,
			bounded_chain_id(b"test-chain".to_vec())
		));

		assert_noop!(
			Signet::sign_respond(
				RuntimeOrigin::signed(requester),
				bounded_u8::<65536>(vec![]),
				60,
				1,
				bounded_u8::<256>(b"path".to_vec()),
				bounded_u8::<32>(b"algo".to_vec()),
				bounded_u8::<64>(b"dest".to_vec()),
				bounded_u8::<1024>(b"params".to_vec()),
				SerializationFormat::Borsh,
				bounded_u8::<4096>(vec![]),
				SerializationFormat::Borsh,
				bounded_u8::<4096>(vec![])
			),
			Error::<Test>::InvalidTransaction
		);
	});
}

#[test]
fn test_respond_single() {
	new_test_ext().execute_with(|| {
		let responder = 1u64;
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
		let responder = 1u64;
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
		let responder = 1u64;

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
		let responder = 1u64;
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
		let responder = 1u64;
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
fn test_read_respond() {
	new_test_ext().execute_with(|| {
		let responder = 1u64;
		let request_id = [99u8; 32];
		let output = b"read_output_data".to_vec();
		let signature = create_test_signature();

		assert_ok!(Signet::read_respond(
			RuntimeOrigin::signed(responder),
			request_id,
			bounded_u8::<65536>(output.clone()),
			signature.clone()
		));

		System::assert_last_event(
			Event::ReadResponded {
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
		let admin = 1u64;
		let requester = 2u64;
		let chain_id = bounded_chain_id(b"hydradx:polkadot:0".to_vec());

		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			admin,
			100,
			chain_id.clone()
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

		assert_eq!(sign_event, Some(chain_id.to_vec()));
	});
}

#[test]
fn test_cross_pallet_execution() {
	new_test_ext().execute_with(|| {
		// Initialize signet first
		assert_ok!(Signet::initialize(
			RuntimeOrigin::signed(1),
			1,
			100,
			bounded_chain_id(b"test-chain".to_vec())
		));

		// Fund the MockCaller pallet's account
		let mock_pallet_account: u64 = MockCallerPalletId::get().into_account_truncating();
		let _ = Balances::deposit_creating(&mock_pallet_account, 10_000);

		// User calls MockCaller, which then calls Signet
		assert_ok!(MockCaller::call_signet(RuntimeOrigin::signed(2)));

		// Check the event - the sender should be the PALLET's account
		System::assert_last_event(
			Event::SignatureRequested {
				sender: mock_pallet_account,
				payload: [99u8; 32],
				key_version: 1,
				deposit: 100,
				chain_id: bounded_chain_id(b"test-chain".to_vec()).to_vec(),
				path: b"from_pallet".to_vec(),
				algo: b"ecdsa".to_vec(),
				dest: b"".to_vec(),
				params: b"{}".to_vec(),
			}
			.into(),
		);

		// Verify the deposit was taken from the pallet's account
		assert_eq!(Balances::free_balance(&mock_pallet_account), 10_000 - 100);

		println!("✅ Cross-pallet test passed!");
		println!("   User 2 called MockCaller");
		println!("   MockCaller called Signet");
		println!(
			"   Signet saw sender as: {:?} (the pallet account)",
			mock_pallet_account
		);
		println!("   NOT as: 2 (the original user)");
	});
}
