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

// Create a mock runtime for testing
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

// Balances pallet configuration
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
	// Removed MaxHolds - not in newer version
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

// Pallet ID for account derivation
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

// Build test environment with initial balances
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		// Fund accounts directly in tests instead of using GenesisConfig
		let _ = Balances::deposit_creating(&1, 1_000_000); // Admin has 1M tokens
		let _ = Balances::deposit_creating(&2, 1_000_000); // User has 1M tokens
		let _ = Balances::deposit_creating(&3, 100); // Poor user has only 100 tokens
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
			RuntimeOrigin::root(),
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
			RuntimeOrigin::root(),
			1,
			1000,
			b"test-chain".to_vec()
		));

		assert_noop!(
			Signet::initialize(RuntimeOrigin::root(), 2, 2000, b"test-chain".to_vec()),
			Error::<Test>::AlreadyInitialized
		);

		assert_eq!(Signet::admin(), Some(1));
	});
}

#[test]
fn test_cannot_use_before_initialization() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::emit_custom_event(RuntimeOrigin::signed(1), b"hello".to_vec(), 123),
			Error::<Test>::NotInitialized
		);
	});
}

#[test]
fn test_emit_event_after_initialization() {
	new_test_ext().execute_with(|| {
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			1,
			1000,
			b"test-chain".to_vec()
		));

		let sender = 2u64;
		let message = b"Hello World".to_vec();
		let value = 12345u128;

		assert_ok!(Signet::emit_custom_event(
			RuntimeOrigin::signed(sender),
			message.clone(),
			value
		));

		System::assert_last_event(
			Event::DataEmitted {
				who: sender,
				message: BoundedVec::try_from(message).unwrap(),
				value,
			}
			.into(),
		);
	});
}

#[test]
fn test_only_root_can_initialize() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Signet::initialize(RuntimeOrigin::signed(1), 1, 1000, b"test-chain".to_vec()),
			sp_runtime::DispatchError::BadOrigin
		);

		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			1,
			1000,
			b"test-chain".to_vec()
		));
	});
}

#[test]
fn test_initialize_sets_deposit() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let initial_deposit = 1000u128; // Changed from u128 to u64

		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			initial_deposit,
			b"test-chain".to_vec()
		));

		assert_eq!(Signet::signature_deposit(), initial_deposit);

		System::assert_last_event(
			Event::Initialized {
				admin,
				signature_deposit: initial_deposit,
				chain_id: b"test-chain".to_vec(), // Add this line
			}
			.into(),
		);
	});
}

#[test]
fn test_update_deposit_as_admin() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let initial_deposit = 1000u128; // Changed from u128 to u64
		let new_deposit = 2000u128; // Changed from u128 to u64

		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
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
			RuntimeOrigin::root(),
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
		let amount = 5000u128; // Changed from u128 to u64

		// Initialize
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			1000,
			b"test-chain".to_vec()
		));

		// Fund the pallet account (simulate deposits)
		let pallet_account = Signet::account_id();
		let _ = Balances::deposit_creating(&pallet_account, 10_000);

		// Check initial balances
		let recipient_balance_before = Balances::free_balance(&recipient);
		assert_eq!(Balances::free_balance(&pallet_account), 10_000);

		// Admin withdraws funds
		assert_ok!(Signet::withdraw_funds(RuntimeOrigin::signed(admin), recipient, amount));

		// Check balances after withdrawal
		assert_eq!(Balances::free_balance(&pallet_account), 5_000); // 10k - 5k
		assert_eq!(Balances::free_balance(&recipient), recipient_balance_before + amount);

		// Check event
		System::assert_last_event(Event::FundsWithdrawn { amount, recipient }.into());
	});
}

#[test]
fn test_non_admin_cannot_withdraw() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let non_admin = 2u64;

		// Initialize and fund pallet
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			1000,
			b"test-chain".to_vec()
		));
		let pallet_account = Signet::account_id();
		let _ = Balances::deposit_creating(&pallet_account, 10_000);

		// Non-admin tries to withdraw
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

		// Initialize and fund pallet with 10k
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			1000,
			b"test-chain".to_vec()
		));
		let pallet_account = Signet::account_id();
		let _ = Balances::deposit_creating(&pallet_account, 10_000);

		// Try to withdraw 20k (more than balance)
		assert_noop!(
			Signet::withdraw_funds(RuntimeOrigin::signed(admin), admin, 20_000),
			Error::<Test>::InsufficientFunds
		);
	});
}

#[test]
fn test_pallet_account_id_is_deterministic() {
	new_test_ext().execute_with(|| {
		// The pallet account should always be the same
		let account1 = Signet::account_id();
		let account2 = Signet::account_id();
		assert_eq!(account1, account2);

		// And it should be different from regular accounts
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

		// Initialize first
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			deposit,
			b"test-chain".to_vec()
		));

		// Check requester balance before
		let balance_before = Balances::free_balance(&requester);

		// Create signature request
		let payload = [42u8; 32];
		let key_version = 1u32;
		let path = b"path".to_vec();
		let algo = b"ecdsa".to_vec();
		let dest = b"callback_contract".to_vec();
		let params = b"{}".to_vec();

		// Submit signature request
		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester),
			payload,
			key_version,
			path.clone(),
			algo.clone(),
			dest.clone(),
			params.clone()
		));

		// Check that deposit was transferred to pallet
		assert_eq!(Balances::free_balance(&requester), balance_before - deposit);
		let pallet_account = Signet::account_id();
		assert_eq!(Balances::free_balance(&pallet_account), deposit);

		// Check event was emitted
		System::assert_last_event(
			Event::SignatureRequested {
				sender: requester,
				payload,
				key_version,
				deposit,
				chain_id: b"test-chain".to_vec(), // Add this line
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
		let poor_user = 3u64; // Has only 100 tokens
		let deposit = 1000u128; // Deposit is 1000

		// Initialize
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			deposit,
			b"test-chain".to_vec()
		));

		// Try to request signature without enough balance
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
		// Try to request signature before initialization
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

		// Initialize with small deposit
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			deposit,
			b"test-chain".to_vec()
		));

		let pallet_account = Signet::account_id();

		// First request
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

		// Second request - funds accumulate
		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester2),
			[2u8; 32],
			2,
			b"path2".to_vec(),
			b"algo".to_vec(),
			b"dest".to_vec(),
			b"params".to_vec()
		));

		// Pallet should have accumulated both deposits
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

		// Initialize
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			deposit,
			b"test-chain".to_vec()
		));

		// Create transaction data
		let tx_data = b"mock_transaction_data".to_vec();
		let slip44_chain_id = 60u32; // Ethereum

		// Check balance before
		let balance_before = Balances::free_balance(&requester);

		// Submit sign-respond request
		assert_ok!(Signet::sign_respond(
			RuntimeOrigin::signed(requester),
			tx_data.clone(),
			slip44_chain_id,
			1, // key_version
			b"path".to_vec(),
			b"ecdsa".to_vec(),
			b"callback".to_vec(),
			b"{}".to_vec(),
			SerializationFormat::AbiJson,
			b"schema1".to_vec(),
			SerializationFormat::Borsh,
			b"schema2".to_vec()
		));

		// Check deposit was taken
		assert_eq!(Balances::free_balance(&requester), balance_before - deposit);

		// Check event
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

		// Initialize
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			100,
			b"test-chain".to_vec()
		));

		// Try with empty transaction
		assert_noop!(
			Signet::sign_respond(
				RuntimeOrigin::signed(requester),
				vec![], // Empty transaction
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

		// No initialization needed for respond
		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(responder),
			vec![request_id],
			vec![signature.clone()]
		));

		// Check event
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

		// Batch respond
		assert_ok!(Signet::respond(
			RuntimeOrigin::signed(responder),
			request_ids.clone(),
			signatures.clone()
		));

		// Check that 3 events were emitted
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

		// 2 request IDs but 3 signatures
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

		// Check event
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

		// Check that 2 error events were emitted
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

		// Check event
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

		// Initialize with a specific deposit
		assert_ok!(Signet::initialize(
			RuntimeOrigin::root(),
			admin,
			deposit,
			b"test-chain".to_vec()
		));

		// The getter should return the deposit
		assert_eq!(Signet::signature_deposit(), deposit);

		// The extrinsic should succeed (it's mainly for RPC)
		assert_ok!(Signet::get_signature_deposit(RuntimeOrigin::signed(1)));
	});
}

// Update test for sign to include chain_id in event
#[test]
fn test_sign_includes_chain_id() {
	new_test_ext().execute_with(|| {
		let admin = 1u64;
		let requester = 2u64;
		let chain_id = b"hydradx:polkadot:0".to_vec();

		// Initialize with specific chain_id
		assert_ok!(Signet::initialize(RuntimeOrigin::root(), admin, 100, chain_id.clone()));

		// Submit signature request
		assert_ok!(Signet::sign(
			RuntimeOrigin::signed(requester),
			[42u8; 32],
			1,
			b"path".to_vec(),
			b"algo".to_vec(),
			b"dest".to_vec(),
			b"params".to_vec()
		));

		// Check that event includes chain_id
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
