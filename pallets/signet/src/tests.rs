use crate::{self as pallet_signet, *};
use frame_support::{
    assert_noop, assert_ok,
    parameter_types,
    traits::{ConstU16, ConstU64},
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
    type AccountId = u64;  // Using u64 for simple test accounts
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
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

impl pallet_signet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();  // Using () for tests
}

// Build test environment
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

// ========================================
// 🧪 TESTS START HERE
// ========================================

#[test]
fn test_initialize_works() {
    new_test_ext().execute_with(|| {
        // Account 1 will be our admin
        let admin_account = 1u64;
        
        // Before initialization, admin should be None
        assert_eq!(Signet::admin(), None);
        
        // Initialize the pallet (must use root origin)
        assert_ok!(Signet::initialize(
            RuntimeOrigin::root(),
            admin_account
        ));
        
        // After initialization, admin should be set
        assert_eq!(Signet::admin(), Some(admin_account));
        
        // Check that the event was emitted
        System::assert_last_event(
            Event::Initialized { admin: admin_account }.into()
        );
    });
}

#[test]
fn test_cannot_initialize_twice() {
    new_test_ext().execute_with(|| {
        // First initialization should work
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), 1));
        
        // Second initialization should fail
        assert_noop!(
            Signet::initialize(RuntimeOrigin::root(), 2),
            Error::<Test>::AlreadyInitialized
        );
        
        // Admin should still be the first one
        assert_eq!(Signet::admin(), Some(1));
    });
}

#[test]
fn test_cannot_use_before_initialization() {
    new_test_ext().execute_with(|| {
        // Try to emit event before initialization
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
        // Initialize first
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), 1));
        
        // Now we can emit events
        let sender = 2u64;
        let message = b"Hello World".to_vec();
        let value = 12345u128;
        
        assert_ok!(Signet::emit_custom_event(
            RuntimeOrigin::signed(sender),
            message.clone(),
            value
        ));
        
        // Check the event
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
        // Regular user (not root) tries to initialize
        assert_noop!(
            Signet::initialize(RuntimeOrigin::signed(1), 1),
            sp_runtime::DispatchError::BadOrigin
        );
        
        // Root can initialize
        assert_ok!(Signet::initialize(RuntimeOrigin::root(), 1));
    });
}