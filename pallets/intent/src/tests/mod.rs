mod intents;

use crate as pallet_ice;
use crate::types::{Balance, IntentId, Moment};
use frame_support::pallet_prelude::ConstU32;
use frame_support::traits::{ConstU128, ConstU64, Everything, Time};
use frame_support::{construct_runtime, parameter_types, PalletId};
use orml_traits::parameter_type_with_key;
use pallet_currencies::{BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AssetId = u32;
pub(crate) type AccountId = u64;
type NamedReserveIdentifier = [u8; 8];

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub(crate) const LRNA: AssetId = 1;

pub const DEFAULT_NOW: Moment = 1689844300000; // unix time in milliseconds
thread_local! {
	pub static NOW: RefCell<Moment> = RefCell::new(DEFAULT_NOW);
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Currencies: pallet_currencies,
		Tokens: orml_tokens,
		ICE: pallet_ice,
	}
);

impl frame_system::Config for Test {
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
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
		if *currency_id == LRNA{
			400_000_000
		}else{
			1
		}
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = NamedReserveIdentifier;
	type CurrencyHooks = ();
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, i128, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
}

parameter_types! {
	pub const MaxReserves: u32 = 50;
	pub const NativeAssetId: AssetId = 0;
	pub const HubAssetId: AssetId = LRNA;
	pub const MaxCallData: u32 = 4 * 1024 * 1024;
	pub const IntentPalletId: PalletId = PalletId(*b"testintn");
	pub const MaxAllowdIntentDuration: Moment = 86_400_000; //1day
	pub const NativeCurrencyId: AssetId = 0;
	pub NamedReserveId: NamedReserveIdentifier = *b"iceinten";
}

pub struct DummyTimestampProvider;

impl Time for DummyTimestampProvider {
	type Moment = u64;

	fn now() -> Self::Moment {
		//TODO: perhaps use some static value which is possible to set as part of test
		NOW.with(|now| *now.borrow())
	}
}

pub struct MockBlockNumberProvider;

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u64;

	fn current_block_number() -> Self::BlockNumber {
		System::block_number()
	}
}

impl pallet_ice::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type HubAssetId = HubAssetId;
	type TimestampProvider = DummyTimestampProvider;
	type MaxAllowedIntentDuration = MaxAllowdIntentDuration;
	type BlockNumberProvider = MockBlockNumberProvider;
	type ReservableCurrency = Currencies;
	type PalletId = IntentPalletId;
	type MaxCallData = MaxCallData;
	type NamedReserveId = NamedReserveId;
	type WeightInfo = ();
}

#[derive(Default)]
pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	native_amounts: Vec<(u64, Balance)>,
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		pallet_balances::GenesisConfig::<Test> {
			balances: self.native_amounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();

		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}

pub(crate) fn get_intent_id(moment: Moment, increment: u64) -> IntentId {
	crate::Pallet::<Test>::get_intent_id(moment, increment)
}
