// Creating mock runtime here

use super::*;
use crate::{AssetPairAccountIdFor, Config, Module};
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
};

use frame_support::traits::GenesisBuild;
use primitives::{fee, AssetId, Balance};

pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub const HDX: AssetId = 1000;
pub const DOT: AssetId = 2000;
pub const ACA: AssetId = 3000;

mod amm {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Test{
		system<T>,
		amm<T>,
		orml_tokens<T>,
	}
}

impl_outer_origin! {
	pub enum Origin for Test {}
}

// For testing the pallet, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of pallets we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;

	pub const HDXAssetId: AssetId = HDX;

	pub ExchangeFeeRate: fee::Fee = fee::Fee::default();
}

impl pallet_asset_registry::Config for Test {
	type AssetId = AssetId;
}

impl system::Config for Test {
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
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
}

pub type Amount = i128;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
		Zero::zero()
	};
}

impl orml_tokens::Config for Test {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

pub type Currency = orml_tokens::Module<Test>;

pub struct AssetPairAccountIdTest();

impl AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			let tmp = a;
			a = b;
			b = tmp;
		}
		return (a * 1000 + b) as u64;
	}
}

impl Config for Test {
	type Event = TestEvent;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currency;
	type HDXAssetId = HDXAssetId;
	type WeightInfo = ();
	type GetExchangeFee = ExchangeFeeRate;
}
pub type AMM = Module<Test>;
pub type System = system::Module<Test>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

// Returns default values for genesis config
impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1000_000_000_000_000u128),
				(BOB, HDX, 1000_000_000_000_000u128),
				(ALICE, ACA, 1000_000_000_000_000u128),
				(BOB, ACA, 1000_000_000_000_000u128),
				(ALICE, DOT, 1000_000_000_000_000u128),
				(BOB, DOT, 1000_000_000_000_000u128),
			],
		}
	}
}

impl ExtBuilder {
	// builds genesis config

	pub fn with_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
