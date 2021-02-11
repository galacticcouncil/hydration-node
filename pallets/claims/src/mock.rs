use crate::{Config, EthereumAddress, GenesisConfig, Module};
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use frame_system;
use orml_traits::parameter_type_with_key;
use primitives::{AssetId, Balance};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
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
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

pub type Amount = i128;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
		Zero::zero()
	};
}

impl orml_tokens::Config for Test {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

pub type Currency = orml_tokens::Module<Test>;

parameter_types! {
	pub Prefix: &'static [u8] = b"I hereby claim all my xHDX tokens to wallet:";
}

impl Config for Test {
	type Event = ();
	type Currency = Currency;
	type Prefix = Prefix;
}

impl_outer_event! {
	pub enum TestEvent for Test{
		frame_system<T>,
		orml_tokens<T>,
	}
}

pub type System = frame_system::Module<Test>;
pub type Claims = Module<Test>;
pub type AccountId = u64;

pub const ALICE: AccountId = 42;
pub const BOB: AccountId = 142;
pub const HDX: AssetId = 1000;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl ExtBuilder {
	// builds genesis config
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

 		GenesisConfig::<Test> {
			claims: vec![(
				EthereumAddress([
					130, 2, 192, 175, 89, 98, 183, 80, 18, 60, 225, 169, 177, 46, 28, 48, 164, 151, 53, 87,
				]),
				50_000,
			)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, HDX, 1000u128)],
		}
	}
}
