use crate::{self as pallet_btc_vault, *};
use frame_support::{assert_noop, assert_ok, parameter_types, traits::Currency as CurrencyTrait, PalletId, BoundedVec};
use frame_system as system;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

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

fn public_key_to_btc_address(public_key: &PublicKey) -> [u8; 20] {
    let uncompressed = public_key.serialize_uncompressed();
    let hash = keccak_256(&uncompressed[1..]);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

// Create a valid signature for testing using secp256k1 directly
fn create_valid_signature(message_hash: &[u8; 32]) -> pallet_signet::Signature {
    let secp = Secp256k1::new();
    let secret_key = get_test_secret_key();
    let message = Message::from_slice(message_hash).expect("Valid message hash");

    let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
    let (recovery_id, sig_bytes) = sig.serialize_compact();

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig_bytes[0..32]);
    s.copy_from_slice(&sig_bytes[32..64]);

    pallet_signet::Signature {
        big_r: pallet_signet::AffinePoint {
            x: r,
            y: [0u8; 32],
        },
        s,
        recovery_id: recovery_id.to_i32() as u8,
    }
}

type Block = frame_system::mocking::MockBlock<Test>;

// Mock runtime construction
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Signet: pallet_signet,
        BtcVault: pallet_btc_vault,
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
    type Block = Block;
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
    pub const MaxDataLength: u32 = 100_000;
    pub const MaxSignatureDeposit: u128 = 10_000_000;
}

impl pallet_signet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = SignetPalletId;
    type MaxChainIdLength = MaxChainIdLength;
    type WeightInfo = pallet_signet::weights::WeightInfo<Test>;
    type MaxDataLength = MaxDataLength;
    type UpdateOrigin = frame_system::EnsureRoot<u64>;
    type MaxSignatureDeposit = MaxSignatureDeposit;
}

// BTC Vault config
parameter_types! {
    pub const BtcVaultPalletId: PalletId = PalletId(*b"py/btcvt");
    pub const MaxBtcInputs: u32 = 100;
    pub const MaxBtcOutputs: u32 = 100;
}

impl pallet_btc_vault::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type VaultPalletId = BtcVaultPalletId;
    type MaxBtcInputs = MaxBtcInputs;
    type MaxBtcOutputs = MaxBtcOutputs;
}

// Helper to build test externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // Fund test accounts
        let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&1, 1_000_000);
        let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&2, 1_000_000);
        let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&3, 100);
        // Initialize signet pallet
        let _ = pallet_signet::Pallet::<Test>::initialize(
            RuntimeOrigin::root(),
            1,
            100,
            bounded_chain_id(b"test-chain".to_vec()),
        );
        // Fund pallet account
        let pallet_account = BtcVault::account_id();
        let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&pallet_account, 10_000);
    });
    ext
}

fn create_test_mpc_address() -> [u8; 20] {
    let public_key = get_test_public_key();
    public_key_to_btc_address(&public_key)
}

// ========================================
// INITIALIZATION TESTS
// ========================================

#[test]
fn test_initialize_works() {
    new_test_ext().execute_with(|| {
        let initializer = 2u64;
        let mpc_address = create_test_mpc_address();

        assert_eq!(BtcVault::vault_config(), None);

        assert_ok!(BtcVault::initialize(RuntimeOrigin::signed(initializer), mpc_address));

        assert_eq!(
            BtcVault::vault_config(),
            Some(VaultConfigData {
                mpc_root_signer_address: mpc_address
            })
        );

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

        assert_ok!(BtcVault::initialize(RuntimeOrigin::signed(1), mpc_address));

        assert_noop!(
            BtcVault::initialize(RuntimeOrigin::signed(2), [4u8; 20]),
            Error::<Test>::AlreadyInitialized
        );
    });
}

// ========================================
// DEPOSIT TESTS
// ========================================

#[test]
fn test_deposit_btc_fails_without_initialization() {
    new_test_ext().execute_with(|| {
        let requester = 1u64;
        let request_id = [1u8; 32];

        let inputs = vec![pallet_signet::UtxoInput {
            txid: [0x42; 32],
            vout: 0,
            value: 100_000_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
            sequence: 0xFFFFFFFF,
        }];

        let outputs = vec![pallet_signet::BitcoinOutput {
            value: 99_900_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
        }];

        assert_noop!(
            BtcVault::deposit_btc(RuntimeOrigin::signed(requester), request_id, inputs, outputs, 0),
            Error::<Test>::NotInitialized
        );
    });
}

#[test]
fn test_deposit_btc_fails_without_vault_output() {
    new_test_ext().execute_with(|| {
        assert_ok!(BtcVault::initialize(RuntimeOrigin::signed(1), create_test_mpc_address()));

        let requester = 2u64;
        let request_id = [1u8; 32];

        let inputs = vec![pallet_signet::UtxoInput {
            txid: [0x42; 32],
            vout: 0,
            value: 100_000_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
            sequence: 0xFFFFFFFF,
        }];

        // Output that doesn't go to vault
        let outputs = vec![pallet_signet::BitcoinOutput {
            value: 99_900_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14, 0x99, 0x99]).unwrap(),
        }];

        assert_noop!(
            BtcVault::deposit_btc(RuntimeOrigin::signed(requester), request_id, inputs, outputs, 0),
            Error::<Test>::NoVaultOutput
        );
    });
}

// ========================================
// CLAIM TESTS
// ========================================

#[test]
fn test_claim_nonexistent_deposit_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(BtcVault::initialize(RuntimeOrigin::signed(1), create_test_mpc_address()));

        let claimer = 2u64;
        let request_id = [99u8; 32];

        assert_noop!(
            BtcVault::claim_btc(
                RuntimeOrigin::signed(claimer),
                request_id,
                bounded_u8::<4194304>(vec![1u8]),
                pallet_signet::Signature {
                    big_r: pallet_signet::AffinePoint {
                        x: [1u8; 32],
                        y: [2u8; 32],
                    },
                    s: [3u8; 32],
                    recovery_id: 0,
                },
            ),
            Error::<Test>::DepositNotFound
        );
    });
}

#[test]
fn test_claim_with_error_response_fails() {
    new_test_ext().execute_with(|| {
        let mpc_address = create_test_mpc_address();
        assert_ok!(BtcVault::initialize(RuntimeOrigin::signed(1), mpc_address));

        // Manually insert a pending deposit for testing
        let requester = 2u64;
        let request_id = [1u8; 32];
        let amount = 50_000_000u64;

        PendingDeposits::<Test>::insert(
            &request_id,
            PendingDepositData {
                requester,
                amount,
                path: bounded_u8::<256>(b"test".to_vec()),
            },
        );

        // Error response with magic prefix
        let error_output = vec![0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3];

        let message_hash = {
            let mut data = Vec::with_capacity(32 + error_output.len());
            data.extend_from_slice(&request_id);
            data.extend_from_slice(&error_output);
            keccak_256(&data)
        };

        let valid_signature = create_valid_signature(&message_hash);

        assert_noop!(
            BtcVault::claim_btc(
                RuntimeOrigin::signed(requester),
                request_id,
                bounded_u8::<4194304>(error_output),
                valid_signature,
            ),
            Error::<Test>::TransferFailed
        );

        assert_eq!(BtcVault::user_balances(requester), 0);
    });
}

#[test]
fn test_claim_successful_with_valid_signature() {
    new_test_ext().execute_with(|| {
        let mpc_address = create_test_mpc_address();
        assert_ok!(BtcVault::initialize(RuntimeOrigin::signed(1), mpc_address));

        let requester = 2u64;
        let request_id = [1u8; 32];
        let amount = 50_000_000u64;

        PendingDeposits::<Test>::insert(
            &request_id,
            PendingDepositData {
                requester,
                amount,
                path: bounded_u8::<256>(b"test".to_vec()),
            },
        );

        // Success response: Borsh-encoded true (1u8)
        let success_output = vec![1u8];

        let message_hash = {
            let mut data = Vec::with_capacity(32 + success_output.len());
            data.extend_from_slice(&request_id);
            data.extend_from_slice(&success_output);
            keccak_256(&data)
        };

        let valid_signature = create_valid_signature(&message_hash);

        assert_eq!(BtcVault::user_balances(requester), 0);

        assert_ok!(BtcVault::claim_btc(
            RuntimeOrigin::signed(requester),
            request_id,
            bounded_u8::<4194304>(success_output),
            valid_signature,
        ));

        assert_eq!(BtcVault::user_balances(requester), amount);
        assert!(BtcVault::pending_deposits(&request_id).is_none());

        System::assert_has_event(
            Event::DepositClaimed {
                request_id,
                claimer: requester,
                amount,
            }
            .into(),
        );
    });
}
