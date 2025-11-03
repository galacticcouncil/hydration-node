use crate::{self as pallet_erc20_vault, *};
use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use codec::Encode;
use frame_support::{assert_noop, assert_ok, parameter_types, traits::Currency as CurrencyTrait, PalletId};
use frame_system as system;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};
extern crate alloc;

// Test secret key for signing
fn get_test_secret_key() -> SecretKey {
	SecretKey::from_slice(&[42u8; 32]).expect("Valid secret key")
}

fn bounded_u8<const N: u32>(v: Vec<u8>) -> BoundedVec<u8, ConstU32<N>> {
	BoundedVec::try_from(v).unwrap()
}

fn bounded_chain_id(v: Vec<u8>) -> BoundedVec<u8, MaxChainIdLength> {
	BoundedVec::try_from(v).unwrap()
}

// Get public key from secret key
fn get_test_public_key() -> PublicKey {
	let secp = Secp256k1::new();
	let secret_key = get_test_secret_key();
	PublicKey::from_secret_key(&secp, &secret_key)
}

fn public_key_to_eth_address(public_key: &PublicKey) -> [u8; 20] {
	// Get uncompressed public key (65 bytes: 0x04 + x + y)
	let uncompressed = public_key.serialize_uncompressed();
	// Skip the 0x04 prefix byte and hash the remaining 64 bytes
	let hash = keccak_256(&uncompressed[1..]);
	// Take the last 20 bytes as Ethereum address
	let mut address = [0u8; 20];
	address.copy_from_slice(&hash[12..]);
	address
}

// Create a valid signature for testing using secp256k1 directly
fn create_valid_signature(message_hash: &[u8; 32]) -> pallet_signet::Signature {
	let secp = Secp256k1::new();
	let secret_key = get_test_secret_key();
	let message = Message::from_slice(message_hash).expect("Valid message hash");

	// Sign without hashing (message is already hashed)
	let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
	let (recovery_id, sig_bytes) = sig.serialize_compact();

	// Extract r and s
	let mut r = [0u8; 32];
	let mut s = [0u8; 32];
	r.copy_from_slice(&sig_bytes[0..32]);
	s.copy_from_slice(&sig_bytes[32..64]);

	pallet_signet::Signature {
		big_r: pallet_signet::AffinePoint {
			x: r,
			y: [0u8; 32], // y-coordinate not used in recovery
		},
		s,
		recovery_id: recovery_id.to_i32() as u8,
	}
}

// Mock runtime construction
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Signet: pallet_signet,
		BuildEvmTx: pallet_build_evm_tx,
		Erc20Vault: pallet_erc20_vault,
	}
);

// System config
parameter_types! {
	pub const BlockHashCount: u64 = 250;
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
	type Block = frame_system::mocking::MockBlock<Test>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = ();
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

// Balances config
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

// Signet config
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

// Build EVM TX mock config
parameter_types! {
	pub const MaxDataLength: u32 = 1024;
}

impl pallet_build_evm_tx::Config for Test {
	type MaxDataLength = MaxDataLength;
}

parameter_types! {
	pub const Erc20VaultPalletId: PalletId = PalletId(*b"py/erc20");
}

// ERC20 Vault config
impl pallet_erc20_vault::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type VaultPalletId = Erc20VaultPalletId;
}

// Helper to build test externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		// Fund test accounts using Currency trait
		let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&1, 1_000_000);
		let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&2, 1_000_000);
		let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&3, 100);
		// Initialize signet pallet for tests that need it
		let _ = pallet_signet::Pallet::<Test>::initialize(
			RuntimeOrigin::signed(1),
			1,   // admin
			100, // deposit
			bounded_chain_id(b"test-chain".to_vec()),
		);
		let pallet_account = Erc20Vault::account_id();
		let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&pallet_account, 10_000);
	});
	ext
}

// Helper to create test signature (dummy, for invalid signature tests)
fn create_test_signature() -> pallet_signet::Signature {
	pallet_signet::Signature {
		big_r: pallet_signet::AffinePoint {
			x: [1u8; 32],
			y: [2u8; 32],
		},
		s: [3u8; 32],
		recovery_id: 0,
	}
}

// Helper to create valid EVM transaction params
fn create_test_tx_params() -> EvmTransactionParams {
	EvmTransactionParams {
		value: 0,
		gas_limit: 100_000,
		max_fee_per_gas: 30_000_000_000,
		max_priority_fee_per_gas: 1_000_000_000,
		nonce: 0,
		chain_id: 1,
	}
}

// Helper to create test addresses
fn create_test_erc20_address() -> [u8; 20] {
	[1u8; 20] // Mock USDC address
}

fn create_test_mpc_address() -> [u8; 20] {
	// Use the address derived from our test public key
	let public_key = get_test_public_key();
	public_key_to_eth_address(&public_key)
}

fn compute_request_id(
	requester: u64,
	erc20_address: [u8; 20],
	amount: u128,
	tx_params: &EvmTransactionParams,
) -> [u8; 32] {
	use alloy_sol_types::SolValue;
	use sp_core::crypto::Ss58Codec;

	let recipient = Address::from_slice(&crate::SEPOLIA_VAULT_ADDRESS);
	let call = crate::IERC20::transferCall {
		to: recipient,
		amount: U256::from(amount),
	};

	let rlp_encoded = pallet_build_evm_tx::Pallet::<Test>::build_evm_tx(
		frame_system::RawOrigin::Signed(requester).into(),
		Some(sp_core::H160::from(erc20_address)),
		tx_params.value,
		call.abi_encode(),
		tx_params.nonce,
		tx_params.gas_limit,
		tx_params.max_fee_per_gas,
		tx_params.max_priority_fee_per_gas,
		vec![],
		11155111,
	)
	.expect("build_evm_tx should succeed");

	// Use PALLET account as sender (not requester)
	let pallet_account = Erc20Vault::account_id();

	let encoded = pallet_account.encode();
	let mut account_bytes = [0u8; 32];
	let len = encoded.len().min(32);
	account_bytes[..len].copy_from_slice(&encoded[..len]);

	let account_id32 = sp_runtime::AccountId32::from(account_bytes);
	let sender_ss58 = account_id32.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(0));

	// Path uses requester (this is correct)
	let path = format!("0x{}", hex::encode(requester.encode()));

	let encoded = (
		sender_ss58.as_str(),
		rlp_encoded.as_slice(),
		"eip155:11155111",
		0u32,
		path.as_str(),
		"ecdsa",
		"ethereum",
		"",
	)
		.abi_encode_packed();

	keccak_256(&encoded)
}

// ========================================
// INITIALIZATION TESTS
// ========================================

#[test]
fn test_initialize_works() {
	new_test_ext().execute_with(|| {
		let initializer = 2u64;
		let mpc_address = create_test_mpc_address();

		// Vault should not be initialized yet
		assert_eq!(Erc20Vault::vault_config(), None);

		// Anyone can initialize
		assert_ok!(Erc20Vault::initialize(RuntimeOrigin::signed(initializer), mpc_address));

		// Check storage
		assert_eq!(
			Erc20Vault::vault_config(),
			Some(VaultConfigData {
				mpc_root_signer_address: mpc_address
			})
		);

		// Check event
		System::assert_last_event(
			Event::VaultInitialized {
				mpc_address,
				initialized_by: initializer,
			}
			.into(),
		);
	});
}

#[test]
fn test_cannot_initialize_twice() {
	new_test_ext().execute_with(|| {
		let mpc_address = create_test_mpc_address();

		// First initialization succeeds
		assert_ok!(Erc20Vault::initialize(RuntimeOrigin::signed(1), mpc_address));

		// Second initialization fails
		assert_noop!(
			Erc20Vault::initialize(
				RuntimeOrigin::signed(2),
				[4u8; 20] // Different address
			),
			Error::<Test>::AlreadyInitialized
		);

		// Config remains unchanged
		assert_eq!(
			Erc20Vault::vault_config(),
			Some(VaultConfigData {
				mpc_root_signer_address: mpc_address
			})
		);
	});
}

#[test]
fn test_any_account_can_initialize() {
	new_test_ext().execute_with(|| {
		let random_account = 3u64;
		let mpc_address = create_test_mpc_address();

		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(random_account),
			mpc_address
		));

		System::assert_last_event(
			Event::VaultInitialized {
				mpc_address,
				initialized_by: random_account,
			}
			.into(),
		);
	});
}

// ========================================
// DEPOSIT TESTS
// ========================================

#[test]
fn test_deposit_erc20_fails_without_initialization() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();

		let requester = 1u64;
		let request_id = [1u8; 32];

		assert_noop!(
			Erc20Vault::deposit_erc20(
				RuntimeOrigin::signed(requester),
				request_id,
				create_test_erc20_address(),
				1_000_000u128,
				create_test_tx_params(),
			),
			Error::<Test>::NotInitialized
		);
	});
}

#[test]
fn test_deposit_erc20_success() {
	new_test_ext().execute_with(|| {
		// Initialize vault
		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(1),
			create_test_mpc_address()
		));

		let requester = 2u64;
		let erc20_address = create_test_erc20_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		// Compute the correct request ID
		let request_id = compute_request_id(requester, erc20_address, amount, &tx_params);
		let balance_before = <Balances as CurrencyTrait<_>>::free_balance(&requester);

		assert_ok!(Erc20Vault::deposit_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			erc20_address,
			amount,
			tx_params,
		));

		// Check pending deposit was stored
		let pending = Erc20Vault::pending_deposits(&request_id);
		assert!(pending.is_some());
		let pending = pending.unwrap();
		assert_eq!(pending.requester, requester);
		assert_eq!(pending.amount, amount);
		assert_eq!(pending.erc20_address, erc20_address);

		// Check deposit event was emitted
		let events = System::events();
		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Erc20Vault(Event::DepositRequested {
					request_id: rid,
					requester: req,
					erc20_address: erc20,
					amount: amt,
				}) if rid == &request_id
					&& req == &requester
					&& erc20 == &erc20_address
					&& amt == &amount
			)
		}));

		// Check signet event was emitted
		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Signet(pallet_signet::Event::SignBidirectionalRequested { .. })
			)
		}));

		// Check deposit was taken (signet deposit)
		assert_eq!(
			<Balances as CurrencyTrait<_>>::free_balance(&requester),
			balance_before - 100 // 100 for signet deposit
		);
	});
}

#[test]
fn test_deposit_with_invalid_request_id_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(1),
			create_test_mpc_address()
		));

		let requester = 2u64;
		let wrong_request_id = [99u8; 32];

		assert_noop!(
			Erc20Vault::deposit_erc20(
				RuntimeOrigin::signed(requester),
				wrong_request_id,
				create_test_erc20_address(),
				1_000_000u128,
				create_test_tx_params(),
			),
			Error::<Test>::InvalidRequestId
		);

		assert!(Erc20Vault::pending_deposits(&wrong_request_id).is_none());
	});
}

#[test]
fn test_deposit_duplicate_request_id_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(1),
			create_test_mpc_address()
		));

		let requester = 2u64;
		let erc20_address = create_test_erc20_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		// Compute correct request ID
		let request_id = compute_request_id(requester, erc20_address, amount, &tx_params);

		// First deposit succeeds
		assert_ok!(Erc20Vault::deposit_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			erc20_address,
			amount,
			tx_params.clone(),
		));

		// Second deposit with same request ID fails
		assert_noop!(
			Erc20Vault::deposit_erc20(
				RuntimeOrigin::signed(requester),
				request_id,
				erc20_address,
				2_000_000u128, // Different amount
				tx_params,
			),
			Error::<Test>::InvalidRequestId
		);
	});
}

// ========================================
// CLAIM TESTS
// ========================================

#[test]
fn test_claim_nonexistent_deposit_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(1),
			create_test_mpc_address()
		));

		let claimer = 2u64;
		let request_id = [99u8; 32];

		assert_noop!(
			Erc20Vault::claim_erc20(
				RuntimeOrigin::signed(claimer),
				request_id,
				bounded_u8::<65536>(vec![1u8]),
				create_test_signature(),
			),
			Error::<Test>::DepositNotFound
		);
	});
}

#[test]
fn test_claim_by_non_requester_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(1),
			create_test_mpc_address()
		));

		let requester = 2u64;
		let wrong_claimer = 1u64;
		let erc20_address = create_test_erc20_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		// Compute correct request ID
		let request_id = compute_request_id(requester, erc20_address, amount, &tx_params);

		// Create deposit
		assert_ok!(Erc20Vault::deposit_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			erc20_address,
			amount,
			tx_params,
		));

		// Try to claim with different account
		assert_noop!(
			Erc20Vault::claim_erc20(
				RuntimeOrigin::signed(wrong_claimer),
				request_id,
				bounded_u8::<65536>(vec![1u8]),
				create_test_signature(),
			),
			Error::<Test>::UnauthorizedClaimer
		);
	});
}

#[test]
fn test_claim_with_invalid_signature_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(Erc20Vault::initialize(
			RuntimeOrigin::signed(1),
			create_test_mpc_address()
		));

		let requester = 2u64;
		let erc20_address = create_test_erc20_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		// Compute correct request ID
		let request_id = compute_request_id(requester, erc20_address, amount, &tx_params);

		assert_ok!(Erc20Vault::deposit_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			erc20_address,
			amount,
			tx_params,
		));

		// Create invalid signature
		let mut bad_signature = create_test_signature();
		bad_signature.recovery_id = 5;

		assert_noop!(
			Erc20Vault::claim_erc20(
				RuntimeOrigin::signed(requester),
				request_id,
				bounded_u8::<65536>(vec![1u8]),
				bad_signature,
			),
			Error::<Test>::InvalidSignature
		);
	});
}

#[test]
fn test_claim_with_error_response_fails() {
	new_test_ext().execute_with(|| {
		let mpc_address = create_test_mpc_address();
		assert_ok!(Erc20Vault::initialize(RuntimeOrigin::signed(1), mpc_address));

		let requester = 2u64;
		let erc20_address = create_test_erc20_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		// Compute correct request ID
		let request_id = compute_request_id(requester, erc20_address, amount, &tx_params);

		assert_ok!(Erc20Vault::deposit_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			erc20_address,
			amount,
			tx_params,
		));

		// Error response with magic prefix
		let error_output = vec![0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3];

		// Create valid signature for the error response
		let message_hash = {
			let mut data = Vec::with_capacity(32 + error_output.len());
			data.extend_from_slice(&request_id);
			data.extend_from_slice(&error_output);
			keccak_256(&data)
		};

		let valid_signature = create_valid_signature(&message_hash);

		// Should fail with TransferFailed because error prefix is detected
		assert_noop!(
			Erc20Vault::claim_erc20(
				RuntimeOrigin::signed(requester),
				request_id,
				bounded_u8::<65536>(error_output),
				valid_signature,
			),
			Error::<Test>::TransferFailed
		);

		// Balance should not change
		assert_eq!(Erc20Vault::user_balances(requester, erc20_address), 0);
	});
}

#[test]
fn test_claim_successful_with_valid_signature() {
	new_test_ext().execute_with(|| {
		let mpc_address = create_test_mpc_address();
		assert_ok!(Erc20Vault::initialize(RuntimeOrigin::signed(1), mpc_address));

		let requester = 2u64;
		let erc20_address = create_test_erc20_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		// Compute correct request ID
		let request_id = compute_request_id(requester, erc20_address, amount, &tx_params);

		assert_ok!(Erc20Vault::deposit_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			erc20_address,
			amount,
			tx_params,
		));

		// Success response: Borsh-encoded true (1u8)
		let success_output = vec![1u8];

		// Create valid signature
		let message_hash = {
			let mut data = Vec::with_capacity(32 + success_output.len());
			data.extend_from_slice(&request_id);
			data.extend_from_slice(&success_output);
			keccak_256(&data)
		};

		let valid_signature = create_valid_signature(&message_hash);

		// Initial balance should be 0
		assert_eq!(Erc20Vault::user_balances(requester, erc20_address), 0);

		// Claim should succeed
		assert_ok!(Erc20Vault::claim_erc20(
			RuntimeOrigin::signed(requester),
			request_id,
			bounded_u8::<65536>(success_output),
			valid_signature,
		),);

		// Balance should be updated
		assert_eq!(Erc20Vault::user_balances(requester, erc20_address), amount);

		// Pending deposit should be removed
		assert!(Erc20Vault::pending_deposits(&request_id).is_none());

		// Check event was emitted
		System::assert_has_event(
			Event::DepositClaimed {
				request_id,
				claimer: requester,
				erc20_address,
				amount,
			}
			.into(),
		);
	});
}
