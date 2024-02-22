use crate as collator_rewards;
use crate::Config;
use std::cell::RefCell;

use frame_support::{
	parameter_types,
	traits::{Everything, Nothing},
};

use frame_system as system;
use orml_traits::parameter_type_with_key;
use pallet_session::SessionManager;

use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};
use sp_staking::SessionIndex;
use sp_std::vec::Vec;

pub type AccountId = u64;
type Balance = u128;
type Amount = i128;
type AssetId = u32;

type Block = frame_system::mocking::MockBlock<Test>;

pub const GC_COLL_1: AccountId = 1;
pub const GC_COLL_2: AccountId = 2;
pub const GC_COLL_3: AccountId = 3;
pub const ALICE: AccountId = 4;
pub const BOB: AccountId = 5;
pub const CHARLIE: AccountId = 6;
pub const DAVE: AccountId = 7;

pub const NATIVE_TOKEN: AssetId = 0;

pub const COLLATOR_REWARD: Balance = 10_000;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Tokens: orml_tokens,
		Balances: pallet_balances,
		CollatorRewards: collator_rewards,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1u128
	};
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
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_types! {
	pub const RewardPerCollator: Balance = COLLATOR_REWARD;
	pub const RewardCurrencyId: AssetId = NATIVE_TOKEN;
	pub GcCollators: Vec<AccountId> = vec![GC_COLL_1, GC_COLL_2, GC_COLL_3];
	pub const MaxCandidates: u32 = 50;
}

thread_local! {
	pub static SESSION_ENDED: RefCell<bool> = RefCell::new(false);
}

pub struct MockSessionManager {}
impl SessionManager<AccountId> for MockSessionManager {
	fn new_session(_: SessionIndex) -> Option<Vec<AccountId>> {
		Some(vec![ALICE, BOB, GC_COLL_1, CHARLIE, GC_COLL_2, DAVE, GC_COLL_3])
	}
	fn start_session(_: SessionIndex) {}
	fn end_session(_: SessionIndex) {
		SESSION_ENDED.with(|e| *e.borrow_mut() = true);
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = AssetId;
	type Currency = Tokens;
	type RewardPerCollator = RewardPerCollator;
	type RewardCurrencyId = RewardCurrencyId;
	type ExcludedCollators = GcCollators;
	type SessionManager = MockSessionManager;
	type MaxCandidates = MaxCandidates;
}

#[derive(Default)]
pub struct ExtBuilder {}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		frame_system::GenesisConfig::<Test>::default()
			.build_storage()
			.unwrap()
			.into()
	}
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}
