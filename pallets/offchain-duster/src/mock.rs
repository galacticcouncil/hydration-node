use super::*;
use crate as offchain_duster;

use frame_support::parameter_types;
use frame_support::traits::GenesisBuild;

use orml_traits::parameter_type_with_key;

use crate::Config;
use frame_system as system;

use sp_core::{sr25519::Signature, H256};

use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
};

use frame_support::weights::Weight;
use primitives::Amount;
use primitives::{AssetId, Balance};
use sp_std::vec::Vec;

type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

lazy_static::lazy_static! {
pub static ref ALICE: AccountId = sp_core::sr25519::Public(*b"qwertyuiopasdfghjklzxcvbnmqwerty").into_account();
pub static ref BOB: AccountId = sp_core::sr25519::Public(*b"qwertyuiopasdfghjklzxcvbnmbobbob").into_account();
pub static ref DUSTER: AccountId = sp_core::sr25519::Public(*b"qwertyuiopasdfghjklzxcvnmbduster").into_account();
pub static ref TREASURY: AccountId = sp_core::sr25519::Public(*b"treasyyuiopasfghjklzxcvbnmqwerty").into_account();
}

parameter_types! {
	pub TreasuryAccount: AccountId = TREASURY.into_account();
}

frame_support::construct_runtime!(
	pub enum Test where
	Block = Block,
	NodeBlock = Block,
	UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Duster: pallet_duster::{Pallet, Call, Storage, Event<T>},
		OffchainDuster: offchain_duster::{Pallet},
		Tokens: orml_tokens::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;

	pub const SS58Prefix: u8 = 63;

	pub NativeCurrencyId: AssetId = 0;
	pub Reward: Balance = 10_000;
}

impl system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = sp_core::sr25519::Public;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

pub type Extrinsic = TestXt<Call, ()>;

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: Call,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(Call, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

use sp_runtime::traits::Zero;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		Zero::zero()
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
	type AuthorityId = crypto::TestAuthId;
}

impl pallet_duster::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = AssetId;
	type MultiCurrency = Tokens;
	type MinCurrencyDeposits = MinDeposits;
	type DustAccount = TreasuryAccount;
	type RewardAccount = TreasuryAccount;
	type Reward = Reward;
	type NativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}
impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_balance(mut self, account: AccountId, currency_id: AssetId, amount: Balance) -> Self {
		self.endowed_accounts.push((account, currency_id, amount));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
