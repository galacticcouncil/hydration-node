use crate as pallet_collator_rotation;
use frame_support::{construct_runtime, parameter_types, traits::Everything};
use frame_system as system;
use pallet_session::SessionManager;
use sp_core::H256;
use sp_runtime::{traits::BlakeTwo256, BuildStorage};
use sp_staking::SessionIndex;
use std::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;

construct_runtime!(
	pub enum Test {
		System: frame_system,
		CollatorRotation: pallet_collator_rotation,
	}
);

thread_local! {
	pub static INNER_SET: RefCell<Option<Vec<AccountId>>> = RefCell::new(Some(vec![1, 2, 3, 4, 5]));
	pub static END_CALLS: RefCell<Vec<SessionIndex>> = const { RefCell::new(Vec::new()) };
	pub static START_CALLS: RefCell<Vec<SessionIndex>> = const { RefCell::new(Vec::new()) };
}

pub struct MockInner;
impl SessionManager<AccountId> for MockInner {
	fn new_session(_index: SessionIndex) -> Option<Vec<AccountId>> {
		INNER_SET.with(|c| c.borrow().clone())
	}

	fn end_session(index: SessionIndex) {
		END_CALLS.with(|c| c.borrow_mut().push(index));
	}

	fn start_session(index: SessionIndex) {
		START_CALLS.with(|c| c.borrow_mut().push(index));
	}
}

pub fn set_inner(set: Option<Vec<AccountId>>) {
	INNER_SET.with(|c| *c.borrow_mut() = set);
}

pub fn end_calls() -> Vec<SessionIndex> {
	END_CALLS.with(|c| c.borrow().clone())
}

pub fn start_calls() -> Vec<SessionIndex> {
	START_CALLS.with(|c| c.borrow().clone())
}

parameter_types! {}

impl pallet_collator_rotation::Config for Test {
	type Inner = MockInner;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = frame_support::traits::ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = frame_support::traits::ConstU16<63>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
	type ExtensionsWeightInfo = ();
}

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		let mut ext: sp_io::TestExternalities = storage.into();
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
