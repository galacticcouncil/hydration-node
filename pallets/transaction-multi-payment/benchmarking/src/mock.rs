#![cfg(test)]

use crate::Config;
use frame_support::parameter_types;
use frame_support::traits::GenesisBuild;
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
};

use frame_support::weights::IdentityFee;
use orml_currencies::BasicCurrencyAdapter;
use pallet_transaction_multi_payment::MultiCurrencyAdapter;
use primitives::{Amount, AssetId, Balance};

use frame_support::traits::Get;
use pallet_amm::AssetPairAccountIdFor;
use std::cell::RefCell;

use frame_benchmarking::frame_support::weights::Pays;
use orml_utilities::OrderedSet;
use primitives::fee;

pub type AccountId = u64;

thread_local! {
		static EXTRINSIC_BASE_WEIGHT: RefCell<u64> = RefCell::new(0);
}

pub struct ExtrinsicBaseWeight;
impl Get<u64> for ExtrinsicBaseWeight {
	fn get() -> u64 {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow())
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
		pub enum Test where
		 Block = Block,
		 NodeBlock = Block,
		 UncheckedExtrinsic = UncheckedExtrinsic,
		 {
				 System: frame_system::{Module, Call, Config, Storage, Event<T>},
				 PaymentModule: pallet_transaction_multi_payment::{Module, Call, Storage, Event<T>},
				 AMMModule: pallet_amm::{Module, Call, Storage, Event<T>},
				 Balances: pallet_balances::{Module,Call, Storage,Config<T>, Event<T>},
				 Currencies: orml_currencies::{Module, Event<T>},
				 Tokens: orml_tokens::{Module, Event<T>},
				 AssetRegistry: pallet_asset_registry::{Module, Storage},
		 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;

	pub const HdxAssetId: u32 = 0;
	pub const ExistentialDeposit: u128 = 0;
	pub const MaxLocks: u32 = 50;
	pub const TransactionByteFee: Balance = 1;
	pub ExchangeFeeRate: fee::Fee = fee::Fee::default();
	pub PayForSetCurrency : Pays = Pays::No;
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
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}
impl Config for Test {}

impl pallet_transaction_multi_payment::Config for Test {
	type Event = Event;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type AMMPool = AMMModule;
	type WeightInfo = ();
	type WithdrawFeeForSetCurrency = PayForSetCurrency;
	type WeightToFee = IdentityFee<Balance>;
}

impl pallet_asset_registry::Config for Test {
	type AssetId = AssetId;
}

impl pallet_balances::Config for Test {
	/// The type for recording an account's balance.
	type Balance = Balance;
	type DustRemoval = ();
	/// The ubiquitous event type.
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
}

impl pallet_transaction_payment::Config for Test {
	type OnChargeTransaction = MultiCurrencyAdapter<Balances, (), PaymentModule>;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
}

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

impl pallet_amm::Config for Test {
	type Event = Event;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currencies;
	type HDXAssetId = HdxAssetId;
	type WeightInfo = ();
	type GetExchangeFee = ExchangeFeeRate;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		Zero::zero()
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

impl orml_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = HdxAssetId;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	base_weight: u64,
	native_balances: Vec<(AccountId, Balance)>,
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			base_weight: 0,
			native_balances: vec![(1, 100_000)],
			endowed_accounts: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn base_weight(mut self, base_weight: u64) -> Self {
		self.base_weight = base_weight;
		self
	}
	fn set_constants(&self) {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow_mut() = self.base_weight);
	}
	pub fn build(self) -> sp_io::TestExternalities {
		self.set_constants();
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: self.native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let core_asset: u32 = 0;
		let mut buf: Vec<u8> = Vec::new();

		buf.extend_from_slice(&core_asset.to_le_bytes());
		buf.extend_from_slice(b"HDT");
		buf.extend_from_slice(&core_asset.to_le_bytes());

		pallet_asset_registry::GenesisConfig::<Test> {
			core_asset_id: 0,
			next_asset_id: 2,
			asset_ids: vec![(buf.to_vec(), 1)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_transaction_multi_payment::GenesisConfig::<Test> {
			currencies: OrderedSet::from(vec![]),
			authorities: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
