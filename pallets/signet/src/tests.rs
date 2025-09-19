use crate::{self as pallet_signet, *};
use crate::{AffinePoint, ErrorResponse, SerializationFormat, Signature};
use frame_support::{
	assert_noop, assert_ok, parameter_types,
	traits::{ConstU16, ConstU64, Currency as CurrencyTrait},
	PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Signet: pallet_signet,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = ();
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
}

impl pallet_balances::Config for Test {
	type Balance = u128;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

parameter_types! {
	pub const SignetPalletId: PalletId = PalletId(*b"py/signt");
	pub const MaxChainIdLength: u32 = 128;
}

impl pallet_signet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = SignetPalletId;
	type MaxChainIdLength = MaxChainIdLength;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		let _ = Balances::deposit_creating(&1, 1_000_000);
		let _ = Balances::deposit_creating(&2, 1_000_000);
		let _ = Balances::deposit_creating(&3, 100);
	});
	ext
}

// ========================================
// 🧪 TESTS START HERE
// ========================================

#[test]
fn test_initialize_works() {
	new_test_ext().execute_with(|| {
		let admin_account = 1u64;
		let deposit = 1000u128;
		let chain_id = b"test-chain".to_vec();

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
				chain_id,
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
			b"test-chain".to_vec()
		));

		assert_noop!(
			Signet::initialize(RuntimeOrigin::signed(2), 2, 2000, b"test-chain".to_vec()),
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
				b"path".to_vec(),
				b"algo".to_vec(),
				b"dest".to_vec(),
				b"params".to_vec()
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
			b"test-chain".to_vec()
		));

		assert_eq!(Signet::admin(), Some(1));
		assert_eq!(Signet::signature_deposit(), 1000);

		assert_noop!(
			Signet::initialize(
				RuntimeOrigin::signed(1),
				3,
				2000,
				b"other-chain".to_vec()
			),
			Error::<Test>::AlreadyInitialized
		);

		assert_noop!(
			Signet::initialize(RuntimeOrigin::signed(3), 3, 2000, b"other-chain".to_vec()),
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
            b"test-chain".to_vec()
        ));

        assert_eq!(Signet::signature_deposit(), initial_deposit);

        System::assert_last_event(
            Event::Initialized {
                admin,
                signature_deposit: initial_deposit,
                chain_id: b"test-chain".to_vec(),
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
            b"test-chain".to_vec()
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
            b"test-chain".to_vec()
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
            b"test-chain".to_vec()
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
            b"test-chain".to_vec()
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
            b"test-chain".to_vec()
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
            b"test-chain".to_vec()
        ));

        let balance_before = Balances::free_balance(&requester);
        let payload = [42u8; 32];
        let key_version = 1u32;
        let path = b"path".to_vec();
        let algo = b"ecdsa".to_vec();
        let dest = b"callback_contract".to_vec();
        let params = b"{}".to_vec();

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
                chain_id: b"test-chain".to_vec(),
                path,
                algo,
                dest,
                params,
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
            b"test-chain".to_vec()
        ));

        assert_noop!(
            Signet::sign(
                RuntimeOrigin::signed(poor_user),
                [0u8; 32],
                1,
                b"path".to_vec(),
                b"algo".to_vec(),
                b"dest".to_vec(),
                b"params".to_vec()
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
				b"path".to_vec(),
				b"algo".to_vec(),
				b"dest".to_vec(),
				b"params".to_vec()
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
            b"test-chain".to_vec()
        ));

        let pallet_account = Signet::account_id();

        assert_ok!(Signet::sign(
            RuntimeOrigin::signed(requester1),
            [1u8; 32],
            1,
            b"path1".to_vec(),
            b"algo".to_vec(),
            b"dest".to_vec(),
            b"params".to_vec()
        ));

        assert_eq!(Balances::free_balance(&pallet_account), deposit);

        assert_ok!(Signet::sign(
            RuntimeOrigin::signed(requester2),
            [2u8; 32],
            2,
            b"path2".to_vec(),
            b"algo".to_vec(),
            b"dest".to_vec(),
            b"params".to_vec()
        ));

        assert_eq!(Balances::free_balance(&pallet_account), deposit * 2);
    });
}

fn create_test_signature() -> Signature {
	Signature {
		big_r: AffinePoint {
			x: [1u8; 32],
			y: [2u8; 32],
		},
		s: [3u8; 32],
		recovery_id: 0,
	}
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
            b"test-chain".to_vec()
        ));

        let tx_data = b"mock_transaction_data".to_vec();
        let slip44_chain_id = 60u32;
        let balance_before = Balances::free_balance(&requester);

        assert_ok!(Signet::sign_respond(
            RuntimeOrigin::signed(requester),
            tx_data.clone(),
            slip44_chain_id,
            1,
            b"path".to_vec(),
            b"ecdsa".to_vec(),
            b"callback".to_vec(),
            b"{}".to_vec(),
            SerializationFormat::AbiJson,
            b"schema1".to_vec(),
            SerializationFormat::Borsh,
            b"schema2".to_vec()
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
            b"test-chain".to_vec()
        ));

        assert_noop!(
            Signet::sign_respond(
                RuntimeOrigin::signed(requester),
                vec![],
                60,
                1,
                b"path".to_vec(),
                b"algo".to_vec(),
                b"dest".to_vec(),
                b"params".to_vec(),
                SerializationFormat::Borsh,
                vec![],
                SerializationFormat::Borsh,
                vec![]
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
			vec![request_id],
			vec![signature.clone()]
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
			request_ids.clone(),
			signatures.clone()
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
				vec![[1u8; 32], [2u8; 32]],
				vec![
					create_test_signature(),
					create_test_signature(),
					create_test_signature(),
				]
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
			error_message: b"Signature generation failed".to_vec(),
		};

		assert_ok!(Signet::respond_error(
			RuntimeOrigin::signed(responder),
			vec![error_response]
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
				error_message: b"Error 1".to_vec(),
			},
			ErrorResponse {
				request_id: [2u8; 32],
				error_message: b"Error 2".to_vec(),
			},
		];

		assert_ok!(Signet::respond_error(RuntimeOrigin::signed(responder), errors));

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
			output.clone(),
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
fn test_get_signature_deposit() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        let deposit = 5000u128;

        assert_ok!(Signet::initialize(
            RuntimeOrigin::signed(1),
            admin,
            deposit,
            b"test-chain".to_vec()
        ));

        assert_eq!(Signet::signature_deposit(), deposit);
        assert_ok!(Signet::get_signature_deposit(RuntimeOrigin::signed(1)));
    });
}

#[test]
fn test_sign_includes_chain_id() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        let requester = 2u64;
        let chain_id = b"hydradx:polkadot:0".to_vec();

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
            b"path".to_vec(),
            b"algo".to_vec(),
            b"dest".to_vec(),
            b"params".to_vec()
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

        assert_eq!(sign_event, Some(chain_id));
    });
}
