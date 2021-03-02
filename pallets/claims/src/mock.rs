use crate::{Config, EthereumAddress, GenesisConfig, Module};
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use frame_system;
use hex_literal::hex;
use primitives::Balance;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

impl_outer_origin! {
	pub enum Origin for Test {}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Test;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type Event = ();
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type AccountStore = frame_system::Module<Test>;
	type WeightInfo = ();
}

parameter_types! {
	pub Prefix: &'static [u8] = b"I hereby claim all my xHDX tokens to wallet:";
}

impl Config for Test {
	type Event = ();
	type Currency = Balances;
	type Prefix = Prefix;
	type WeightInfo = ();
	type CurrencyBalance = Balance;
}

impl_outer_event! {
	pub enum Event for Test{
		frame_system<T>,
		pallet_balances<T>,
	}
}

pub type System = frame_system::Module<Test>;
pub type ClaimsModule = Module<Test>;
pub type Balances = pallet_balances::Module<Test>;

pub type AccountId = u64;
pub const ALICE: AccountId = 42;
pub const BOB: AccountId = 43;
pub const CHARLIE: AccountId = 44;

pub struct ExtBuilder;

impl ExtBuilder {
	// builds genesis config
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(42, 0), (43, 0), (44, primitives::Balance::MAX - 1)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		GenesisConfig::<Test> {
			claims: vec![(
				// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
				EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]),
				50_000,
			)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {}
	}
}
