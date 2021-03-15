use crate as claims;
use crate::{Config, EthereumAddress};
use frame_support::parameter_types;
use frame_system;
use hex_literal::hex;
use primitives::Balance;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use frame_support::traits::GenesisBuild;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system::{Module, Call, Config, Storage, Event<T>},
		 ClaimsModule: claims::{Module, Call, Storage, Event<T>},
		 Balances: pallet_balances::{Module, Event<T>},
	 }
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type AccountStore = frame_system::Module<Test>;
	type WeightInfo = ();
}

parameter_types! {
	pub Prefix: &'static [u8] = b"I hereby claim all my xHDX tokens to wallet:";
}

impl Config for Test {
	type Event = Event;
	type Currency = Balances;
	type Prefix = Prefix;
	type WeightInfo = ();
	type CurrencyBalance = Balance;
}

pub type AccountId = u64;
pub const ALICE: AccountId = 42;
pub const BOB: AccountId = 43;
pub const CHARLIE: AccountId = 44;

pub const CLAIM_AMOUNT: Balance = 1_000_000_000_000;

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

		claims::GenesisConfig::<Test> {
			claims: vec![(
				// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
				// private key (m/44'/60'/0'/0/0) : 0xdd75dd5f4a9e964d1c4cc929768947859a98ae2c08100744878a4b6b6d853cc0
				EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]),
				CLAIM_AMOUNT,
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
