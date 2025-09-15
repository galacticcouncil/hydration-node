use crate::{self as pallet_signet, *};
use frame_support::{
    assert_noop, assert_ok,
    parameter_types,
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
}

impl pallet_signet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = SignetPalletId;
    type WeightInfo = ();
}

// Build test environment with initial balances
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // Fund accounts directly in tests instead of using GenesisConfig
        let _ = Balances::deposit_creating(&1, 1_000_000);  // Admin has 1M tokens
        let _ = Balances::deposit_creating(&2, 1_000_000);  // User has 1M tokens
        let _ = Balances::deposit_creating(&3, 100);        // Poor user has only 100 tokens
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
        let deposit = 1000u128;  // Changed from u128 to u64
        
        assert_eq!(Signet::admin(), None);
        
        assert_ok!(Signet::initialize(
            RuntimeOrigin::root(),
            admin_account,
            deposit
        ));
        
        assert_eq!(Signet::admin(), Some(admin_account));
        assert_eq!(Signet::signature_deposit(), deposit);
        
        System::assert_last_event(
            Event::Initialized { 
                admin: admin_account,
                signature_deposit: deposit
            }.into()
        );
    });
}

#[test]
fn test_cannot_initialize_twice() {
    new_test_ext().execute_with(|| {
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), 1, 1000));
        
        assert_noop!(
            Signet::initialize(RuntimeOrigin::root(), 2, 2000),
            Error::<Test>::AlreadyInitialized
        );
        
        assert_eq!(Signet::admin(), Some(1));
    });
}

#[test]
fn test_cannot_use_before_initialization() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Signet::emit_custom_event(
                RuntimeOrigin::signed(1),
                b"hello".to_vec(),
                123
            ),
            Error::<Test>::NotInitialized
        );
    });
}

#[test]
fn test_emit_event_after_initialization() {
    new_test_ext().execute_with(|| {
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), 1, 1000));
        
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
            }.into()
        );
    });
}

#[test]
fn test_only_root_can_initialize() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Signet::initialize(RuntimeOrigin::signed(1), 1, 1000),
            sp_runtime::DispatchError::BadOrigin
        );
        
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), 1, 1000));
    });
}

#[test]
fn test_initialize_sets_deposit() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        let initial_deposit = 1000u128;  // Changed from u128 to u64
        
        assert_ok!(Signet::initialize(
            RuntimeOrigin::root(),
            admin,
            initial_deposit
        ));
        
        assert_eq!(Signet::signature_deposit(), initial_deposit);
        
        System::assert_last_event(
            Event::Initialized { 
                admin,
                signature_deposit: initial_deposit,
            }.into()
        );
    });
}

#[test]
fn test_update_deposit_as_admin() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        let initial_deposit = 1000u128;  // Changed from u128 to u64
        let new_deposit = 2000u128;  // Changed from u128 to u64
        
        assert_ok!(Signet::initialize(
            RuntimeOrigin::root(),
            admin,
            initial_deposit
        ));
        
        assert_ok!(Signet::update_deposit(
            RuntimeOrigin::signed(admin),
            new_deposit
        ));
        
        assert_eq!(Signet::signature_deposit(), new_deposit);
        
        System::assert_last_event(
            Event::DepositUpdated {
                old_deposit: initial_deposit,
                new_deposit,
            }.into()
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
            1000
        ));
        
        assert_noop!(
            Signet::update_deposit(
                RuntimeOrigin::signed(non_admin),
                2000
            ),
            Error::<Test>::Unauthorized
        );
        
        assert_eq!(Signet::signature_deposit(), 1000);
    });
}

#[test]
fn test_cannot_update_deposit_before_initialization() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Signet::update_deposit(
                RuntimeOrigin::signed(1),
                1000
            ),
            Error::<Test>::NotInitialized
        );
    });
}

#[test]
fn test_withdraw_funds_as_admin() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        let recipient = 2u64;
        let amount = 5000u128;  // Changed from u128 to u64
        
        // Initialize
        assert_ok!(Signet::initialize(
            RuntimeOrigin::root(),
            admin,
            1000
        ));
        
        // Fund the pallet account (simulate deposits)
        let pallet_account = Signet::account_id();
        let _ = Balances::deposit_creating(&pallet_account, 10_000);
        
        // Check initial balances
        let recipient_balance_before = Balances::free_balance(&recipient);
        assert_eq!(Balances::free_balance(&pallet_account), 10_000);
        
        // Admin withdraws funds
        assert_ok!(Signet::withdraw_funds(
            RuntimeOrigin::signed(admin),
            recipient,
            amount
        ));
        
        // Check balances after withdrawal
        assert_eq!(Balances::free_balance(&pallet_account), 5_000);  // 10k - 5k
        assert_eq!(Balances::free_balance(&recipient), recipient_balance_before + amount);
        
        // Check event
        System::assert_last_event(
            Event::FundsWithdrawn {
                amount,
                recipient,
            }.into()
        );
    });
}

#[test]
fn test_non_admin_cannot_withdraw() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        let non_admin = 2u64;
        
        // Initialize and fund pallet
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), admin, 1000));
        let pallet_account = Signet::account_id();
        let _ = Balances::deposit_creating(&pallet_account, 10_000);
        
        // Non-admin tries to withdraw
        assert_noop!(
            Signet::withdraw_funds(
                RuntimeOrigin::signed(non_admin),
                non_admin,
                5000
            ),
            Error::<Test>::Unauthorized
        );
    });
}

#[test]
fn test_cannot_withdraw_more_than_balance() {
    new_test_ext().execute_with(|| {
        let admin = 1u64;
        
        // Initialize and fund pallet with 10k
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), admin, 1000));
        let pallet_account = Signet::account_id();
        let _ = Balances::deposit_creating(&pallet_account, 10_000);
        
        // Try to withdraw 20k (more than balance)
        assert_noop!(
            Signet::withdraw_funds(
                RuntimeOrigin::signed(admin),
                admin,
                20_000
            ),
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