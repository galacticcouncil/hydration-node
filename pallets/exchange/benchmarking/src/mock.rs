#![cfg(test)]

use super::*;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use pallet_amm::AssetPairAccountIdFor;
use primitives::{AssetId, Balance};

pub type Amount = i128;
pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
pub const DAVE: AccountId = 4;
pub const FERDIE: AccountId = 5;
pub const GEORGE: AccountId = 6;

pub const HDX: AssetId = 1000;
pub const DOT: AssetId = 2000;
pub const ETH: AssetId = 3000;

/*impl_outer_dispatch! {
	pub enum Call for Test where origin: Origin {
		exchange::ExchangeModule,
	}
}*/

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

// For testing the pallet, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of pallets we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);

	pub const HDXAssetId: AssetId = HDX;
}
impl system::Trait for Test {
	type BaseCallFilter = ();
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
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

impl orml_tokens::Trait for Test {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type OnReceived = ();
	type WeightInfo = ();
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

impl pallet_asset_registry::Trait for Test {
	type AssetId = AssetId;
}

impl pallet_amm::Trait for Test {
	type Event = ();
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currency;
	type HDXAssetId = HDXAssetId;
	type WeightInfo = ();
}

pub type AMMModule = pallet_amm::Module<Test>;
pub type System = system::Module<Test>;

impl pallet_exchange::Trait for Test {
	type Event = ();
	type AMMPool = AMMModule;
	type Currency = Currency;
	type Resolver = pallet_exchange::Module<Test>;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl crate::Trait for Test {}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1000_000_000_000_000u128),
				(BOB, HDX, 1000_000_000_000_000u128),
				(CHARLIE, HDX, 1000_000_000_000_000u128),
				(DAVE, HDX, 1000_000_000_000_000u128),
				(FERDIE, HDX, 1000_000_000_000_000u128),
				(GEORGE, HDX, 1000_000_000_000_000u128),
				(ALICE, ETH, 1000_000_000_000_000u128),
				(BOB, ETH, 1000_000_000_000_000u128),
				(CHARLIE, ETH, 1000_000_000_000_000u128),
				(DAVE, ETH, 1000_000_000_000_000u128),
				(FERDIE, ETH, 1000_000_000_000_000u128),
				(GEORGE, ETH, 1000_000_000_000_000u128),
				(ALICE, DOT, 1000_000_000_000_000u128),
				(BOB, DOT, 1000_000_000_000_000u128),
				(CHARLIE, DOT, 1000_000_000_000_000u128),
				(DAVE, DOT, 1000_000_000_000_000u128),
				(FERDIE, DOT, 1000_000_000_000_000u128),
				(GEORGE, DOT, 1000_000_000_000_000u128),
			],
		}
	}
}

impl ExtBuilder {
	// builds genesis config

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

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}
