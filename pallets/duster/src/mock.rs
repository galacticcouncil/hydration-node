use crate as duster;

use frame_support::parameter_types;
use frame_support::traits::{Everything, GenesisBuild, Nothing, OnKilledAccount};

use orml_traits::parameter_type_with_key;
use pallet_currencies::BasicCurrencyAdapter;

use crate::Config;
use frame_system as system;

use sp_core::H256;

use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

use frame_support::weights::Weight;
use frame_system::EnsureRoot;
use sp_std::cell::RefCell;
use sp_std::vec::Vec;

type AccountId = u64;
pub type AssetId = u32;
type Balance = u128;
type Amount = i128;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

lazy_static::lazy_static! {
pub static ref ALICE: AccountId = 100;
pub static ref BOB: AccountId = 200;
pub static ref DUSTER: AccountId = 300;
pub static ref TREASURY: AccountId = 400;
}

parameter_types! {
    pub TreasuryAccount: AccountId = *TREASURY;
}

frame_support::construct_runtime!(
    pub enum Test where
    Block = Block,
    NodeBlock = Block,
    UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        Duster: duster,
        Tokens: orml_tokens,
        Currencies: pallet_currencies,
        Balances: pallet_balances,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_ref_time(1024);
    pub const MaximumBlockLength: u32 = 2 * 1024;

    pub const SS58Prefix: u8 = 63;
    pub const MaxLocks: u32 = 50;

    pub const NativeExistentialDeposit: u128 = 100;

    pub NativeCurrencyId: AssetId = 0;
    pub Reward: Balance = 10_000;
}

thread_local! {
    pub static KILLED: RefCell<Vec<u64>> = RefCell::new(vec![]);
}

pub struct RecordKilled;
impl OnKilledAccount<u64> for RecordKilled {
    fn on_killed_account(who: &u64) {
        KILLED.with(|r| r.borrow_mut().push(*who))
    }
}

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = RecordKilled;
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        1u128
    };
}

parameter_type_with_key! {
    pub MinDeposits: |currency_id: AssetId| -> Balance {
        match currency_id {
            0 => 1000,
            1 => 100_000,
            _ => 0
        }
    };
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type MultiCurrency = Currencies;
    type MinCurrencyDeposits = MinDeposits;
    type Reward = Reward;
    type NativeCurrencyId = NativeCurrencyId;
    type BlacklistUpdateOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
}

impl orml_tokens::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type MaxLocks = ();
    type DustRemovalWhitelist = Nothing;
    type ReserveIdentifier = ();
    type MaxReserves = ();
    type CurrencyHooks = ();
}

impl pallet_currencies::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
    type GetNativeCurrencyId = NativeCurrencyId;
    type WeightInfo = ();
}

impl pallet_balances::Config for Test {
    type MaxLocks = MaxLocks;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = NativeExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    native_balances: Vec<(AccountId, Balance)>,
}
impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![],
            native_balances: vec![(*TREASURY, 1_000_000)],
        }
    }
}

impl ExtBuilder {
    pub fn with_balance(mut self, account: AccountId, currency_id: AssetId, amount: Balance) -> Self {
        self.endowed_accounts.push((account, currency_id, amount));
        self
    }
    pub fn with_native_balance(mut self, account: AccountId, amount: Balance) -> Self {
        self.native_balances.push((account, amount));
        self
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: self.native_balances,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        orml_tokens::GenesisConfig::<Test> {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        duster::GenesisConfig::<Test> {
            account_blacklist: vec![*TREASURY],
            reward_account: Some(*TREASURY),
            dust_account: Some(*TREASURY),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
