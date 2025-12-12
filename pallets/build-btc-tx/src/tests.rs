use crate::{self as pallet_build_btc_tx, *};
use frame_support::{assert_noop, assert_ok, parameter_types, traits::{ConstU16, ConstU32, ConstU64}, BoundedVec};
use sp_core::H256;
use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        BuildBitcoinTx: pallet_build_btc_tx,
    }
);

impl frame_system::Config for Test {
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
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
    type RuntimeTask = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

parameter_types! {
    pub const MaxInputs: u32 = 10;
    pub const MaxOutputs: u32 = 10;
}

impl pallet_build_btc_tx::Config for Test {
    type MaxInputs = MaxInputs;
    type MaxOutputs = MaxOutputs;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

#[test]
fn test_build_simple_transaction() {
    new_test_ext().execute_with(|| {
        let inputs = BoundedVec::try_from(vec![UtxoInput {
            txid: [0x42; 32],
            vout: 0,
            value: 100_000_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
            sequence: 0xFFFFFFFF,
        }])
        .unwrap();

        let outputs = BoundedVec::try_from(vec![BitcoinOutput {
            value: 99_900_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
        }])
        .unwrap();

        let result = BuildBitcoinTx::build_bitcoin_tx(
            RuntimeOrigin::signed(1),
            inputs,
            outputs,
            0
        );
        
        assert_ok!(&result);
        let psbt = result.unwrap();
        assert!(!psbt.is_empty(), "PSBT should not be empty");
    });
}

#[test]
fn test_get_txid() {
    new_test_ext().execute_with(|| {
        let inputs = BoundedVec::try_from(vec![UtxoInput {
            txid: [0x42; 32],
            vout: 0,
            value: 100_000_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
            sequence: 0xFFFFFFFF,
        }])
        .unwrap();

        let outputs = BoundedVec::try_from(vec![BitcoinOutput {
            value: 99_900_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
        }])
        .unwrap();

        let result = BuildBitcoinTx::get_txid(
            RuntimeOrigin::signed(1),
            inputs.clone(),
            outputs.clone(),
            0
        );
        
        assert_ok!(&result);
        let txid = result.unwrap();
        assert_eq!(txid.len(), 32, "Txid should be 32 bytes");

        // Verify txid is deterministic
        let result2 = BuildBitcoinTx::get_txid(
            RuntimeOrigin::signed(1),
            inputs,
            outputs,
            0
        );
        assert_eq!(txid, result2.unwrap(), "Same inputs should produce same txid");
    });
}

#[test]
fn test_no_inputs_fails() {
    new_test_ext().execute_with(|| {
        let outputs = BoundedVec::try_from(vec![BitcoinOutput {
            value: 100_000_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
        }])
        .unwrap();

        assert_noop!(
            BuildBitcoinTx::build_bitcoin_tx(
                RuntimeOrigin::signed(1),
                BoundedVec::default(),
                outputs.clone(),
                0
            ),
            Error::<Test>::NoInputs
        );

        assert_noop!(
            BuildBitcoinTx::get_txid(
                RuntimeOrigin::signed(1),
                BoundedVec::default(),
                outputs,
                0
            ),
            Error::<Test>::NoInputs
        );
    });
}

#[test]
fn test_no_outputs_fails() {
    new_test_ext().execute_with(|| {
        let inputs = BoundedVec::try_from(vec![UtxoInput {
            txid: [0x42; 32],
            vout: 0,
            value: 100_000_000,
            script_pubkey: BoundedVec::try_from(vec![0x00, 0x14]).unwrap(),
            sequence: 0xFFFFFFFF,
        }])
        .unwrap();

        assert_noop!(
            BuildBitcoinTx::build_bitcoin_tx(
                RuntimeOrigin::signed(1),
                inputs.clone(),
                BoundedVec::default(),
                0
            ),
            Error::<Test>::NoOutputs
        );

        assert_noop!(
            BuildBitcoinTx::get_txid(
                RuntimeOrigin::signed(1),
                inputs,
                BoundedVec::default(),
                0
            ),
            Error::<Test>::NoOutputs
        );
    });
}