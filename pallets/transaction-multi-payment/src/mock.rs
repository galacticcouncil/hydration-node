use super::*;
use crate::{Config, Module, MultiCurrencyAdapter};
use frame_support::{
	impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types, weights::DispatchClass,
};
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
	Perbill,
};

use frame_support::weights::IdentityFee;
use orml_currencies::BasicCurrencyAdapter;
use primitives::{Amount, AssetId, Balance};

use pallet_amm::AssetPairAccountIdFor;
use std::cell::RefCell;

pub type AccountId = u64;

pub const INITIAL_BALANCE: Balance = 1000_000_000_000_000u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub const HDX: AssetId = 0;
pub const SUPPORTED_CURRENCY_NO_BALANCE: AssetId = 2000;
pub const SUPPORTED_CURRENCY_WITH_BALANCE: AssetId = 3000;
pub const NOT_SUPPORTED_CURRENCY: AssetId = 4000;

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
const MAX_BLOCK_WEIGHT: Weight = 1024;

thread_local! {
		static EXTRINSIC_BASE_WEIGHT: RefCell<u64> = RefCell::new(0);
}

pub struct ExtrinsicBaseWeight;
impl Get<u64> for ExtrinsicBaseWeight {
	fn get() -> u64 {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow())
	}
}

impl_outer_origin! {
	pub enum Origin for Test {}
}

mod multi_payment {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Test{
		system<T>,
		multi_payment<T>,
		pallet_amm<T>,
		pallet_balances<T>,
		orml_tokens<T>,
		orml_currencies<T>,
	}
}
impl_outer_dispatch! {
	pub enum Call for Test where origin: Origin {
		pallet_balances::Balances,
		frame_system::System,
	}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;

	pub const HdxAssetId: u32 = 0;
	pub const ExistentialDeposit: u128 = 0;
	pub const MaxLocks: u32 = 50;
	pub const TransactionByteFee: Balance = 1;
	pub NonNativeAssets: Vec<AssetId> = vec![SUPPORTED_CURRENCY_NO_BALANCE, SUPPORTED_CURRENCY_WITH_BALANCE];

	pub RuntimeBlockWeights: system::limits::BlockWeights = system::limits::BlockWeights::builder()
		.base_block(10)
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAX_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAX_BLOCK_WEIGHT);
			weights.reserved = Some(
				MAX_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAX_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(Perbill::from_percent(0))
		.build_or_panic();
}

impl system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = RuntimeBlockWeights;
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
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

impl Config for Test {
	type Event = TestEvent;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type AMMPool = AMMModule;
	type NonNativeAcceptedAssetId = NonNativeAssets;
	type WeightInfo = ();
}

impl pallet_asset_registry::Config for Test {
	type AssetId = AssetId;
}

impl pallet_balances::Config for Test {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = TestEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
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
	type Event = TestEvent;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currencies;
	type HDXAssetId = HdxAssetId;
	type WeightInfo = ();
}

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

impl orml_currencies::Config for Test {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = HdxAssetId;
	type WeightInfo = ();
}

pub type AMMModule = pallet_amm::Module<Test>;
pub type Tokens = orml_tokens::Module<Test>;
pub type Currencies = orml_currencies::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;

pub type PaymentModule = Module<Test>;
pub type System = system::Module<Test>;

pub struct ExtBuilder {
	base_weight: u64,
	native_balances: Vec<(AccountId, Balance)>,
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			base_weight: 0,
			native_balances: vec![(ALICE, INITIAL_BALANCE), (BOB, 0)],
			endowed_accounts: vec![
				(ALICE, HDX, INITIAL_BALANCE),
				(ALICE, SUPPORTED_CURRENCY_NO_BALANCE, 0u128), // Used for insufficient balance testing
				(ALICE, SUPPORTED_CURRENCY_WITH_BALANCE, INITIAL_BALANCE),
			],
		}
	}
}

impl ExtBuilder {
	pub fn base_weight(mut self, base_weight: u64) -> Self {
		self.base_weight = base_weight;
		self
	}
	pub fn account_native_balance(mut self, account: AccountId, balance: Balance) -> Self {
		self.native_balances.push((account, balance));
		self
	}
	pub fn account_tokens(mut self, account: AccountId, asset: AssetId, balance: Balance) -> Self {
		self.endowed_accounts.push((account, asset, balance));
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

		t.into()
	}
}
