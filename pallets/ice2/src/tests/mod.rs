mod submit;
mod submit_errors;

use crate as pallet_ice;
use crate::traits::Trader;
use crate::types::*;
use frame_support::pallet_prelude::ConstU32;
use frame_support::traits::{ConstU64, Everything, Time};
use frame_support::{construct_runtime, parameter_types, PalletId};
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::price::PriceProvider;
use orml_traits::{parameter_type_with_key, MultiCurrency};
use pallet_intent::types::Moment;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup};
use sp_runtime::{BuildStorage, DispatchError};
use std::cell::RefCell;
use std::collections::HashMap;

type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const EXECUTOR: AccountId = 3;

pub(crate) const LRNA: AssetId = 1;

pub const DEFAULT_NOW: Moment = 1689844300000; // unix time in milliseconds

thread_local! {
	pub static NOW: RefCell<Moment> = RefCell::new(DEFAULT_NOW);
	pub static PRICES: RefCell<HashMap<(AssetId, AssetId), (Balance, Balance)>> = RefCell::new(HashMap::new());
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Tokens: orml_tokens,
		Intents: pallet_intent,
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
	type AccountData = ();
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
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
}

impl pallet_intent::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type TimestampProvider = MockTimestampProvider;
	type HubAssetId = HubAssetId;
	type MaxAllowedIntentDuration = MaxAllowedIntentDuration;
	type MaxCallData = MaxCallData;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxReserves: u32 = 50;
	pub const HubAssetId: AssetId = LRNA;
	pub const MaxCallData: u32 = 4 * 1024 * 1024;
	pub const ICEPalletId: PalletId = PalletId(*b"testicer");
	pub const MaxAllowedIntentDuration: Moment = 86_400_000; //1day
	pub const NativeCurrencyId: AssetId = 0;
	pub const ProposalBond: Balance = 1_000_000_000_000;
}

impl pallet_ice::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ICEPalletId;
	type BlockNumberProvider = MockBlockNumberProvider;
	type Currency = Tokens;
	type PriceProvider = MockPriceProvider;
	type Trader = TestTrader;
	type WeightInfo = ();
}

pub struct MockTimestampProvider;

impl Time for MockTimestampProvider {
	type Moment = u64;

	fn now() -> Self::Moment {
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

pub struct MockPriceProvider;

impl PriceProvider<AssetId> for MockPriceProvider {
	type Price = Ratio;

	fn get_price(_asset_a: AssetId, _asset_b: AssetId) -> Option<Ratio> {
		Some(Ratio::new(1, 1))
	}
}

pub struct TestTrader;

impl Trader<AccountId> for TestTrader {
	type Outcome = ();

	fn trade(account: AccountId, assets: Vec<(AssetId, (Balance, Balance))>) -> Result<Self::Outcome, DispatchError> {
		for (asset, (amount_in, amount_out)) in assets {
			let balance = Tokens::free_balance(asset, &account);
			if balance < amount_in {
				return Err(DispatchError::from("Insufficient balance"));
			}
			Tokens::withdraw(asset, &account, amount_in)?;
			Tokens::deposit(asset, &account, amount_out)?;
		}
		Ok(())
	}
}

#[derive(Default)]
pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_prices(self, prices: Vec<((AssetId, AssetId), (Balance, Balance))>) -> Self {
		PRICES.with(|p| {
			let mut pm = p.borrow_mut();
			for ((a, b), (va, vb)) in prices {
				pm.insert((a, b), (va, vb));
				pm.insert((b, a), (vb, va));
			}
		});
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
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
