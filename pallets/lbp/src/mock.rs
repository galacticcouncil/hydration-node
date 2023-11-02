#![cfg(test)]
use super::*;

use crate as lbp;
use crate::{
	types::{AssetId, AssetPair, Balance},
	AssetPairAccountIdFor, Config,
};
use frame_support::parameter_types;
use frame_support::traits::{Everything, LockIdentifier, Nothing};
use hydradx_traits::LockedBalance;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};
use std::collections::BTreeMap;

pub type Amount = i128;
pub type AccountId = u64;
pub type BlockNumber = u64;
type Block = frame_system::mocking::MockBlock<Test>;

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000_000u128;
pub const INITIAL_ETH_BALANCE: Balance = 1_000_000_000_000_000_000_000u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;

pub const HDX: AssetId = 0;
pub const KUSD: AssetId = 2_000;
pub const BSX: AssetId = 3_000;
pub const ETH: AssetId = 4_000;

pub const EXISTENTIAL_DEPOSIT: Balance = 100;
pub const SALE_START: Option<BlockNumber> = Some(10);
pub const SALE_END: Option<BlockNumber> = Some(40);

pub const HDX_BSX_POOL_ID: AccountId = 3_000;
pub const KUSD_BSX_POOL_ID: AccountId = 2_003_000;

pub const DEFAULT_FEE: (u32, u32) = (2, 1_000);

pub const SAMPLE_POOL_DATA: Pool<AccountId, BlockNumber> = Pool {
	owner: ALICE,
	start: SALE_START,
	end: SALE_END,
	assets: (KUSD, BSX),
	initial_weight: 10_000_000,
	final_weight: 90_000_000,
	weight_curve: WeightCurveType::Linear,
	fee: DEFAULT_FEE,
	fee_collector: CHARLIE,
	repay_target: 0,
};

pub const SAMPLE_AMM_TRANSFER: AMMTransfer<AccountId, AssetId, AssetPair, Balance> = AMMTransfer {
	origin: ALICE,
	assets: AssetPair {
		asset_in: KUSD,
		asset_out: BSX,
	},
	amount: 1000,
	amount_b: 10000,
	discount: false,
	discount_amount: 0_u128,
	fee: (KUSD, 200),
};

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 LBPPallet: lbp,
		 Currency: orml_tokens,
	 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		EXISTENTIAL_DEPOSIT
	};
}

parameter_types! {
	pub const MaxLocks: u32 = 1;
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = ();
	type MaxReserves = ();
	type CurrencyHooks = ();
}

pub struct AssetPairAccountIdTest();

impl AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, _: &str) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			std::mem::swap(&mut a, &mut b);
		}
		(a * 1000 + b) as u64
	}
}

parameter_types! {
	pub const NativeAssetId: AssetId = HDX;
	pub const MinTradingLimit: Balance = 1_000;
	pub const MinPoolLiquidity: Balance = 1_000;
	pub const MaxInRatio: u128 = 3;
	pub const MaxOutRatio: u128 = 3;
}

pub struct MultiLockedBalance();

impl LockedBalance<AssetId, AccountId, Balance> for MultiLockedBalance {
	fn get_by_lock(lock_id: LockIdentifier, asset: AssetId, account: AccountId) -> Balance {
		match Currency::locks(account, asset)
			.into_iter()
			.find(|lock| lock.id == lock_id)
		{
			Some(lock) => lock.amount,
			None => Zero::zero(),
		}
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Currency;
	type LockedBalance = MultiLockedBalance;
	type CreatePoolOrigin = frame_system::EnsureRoot<u64>;
	type LBPWeightFunction = lbp::LBPWeightFunction;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type WeightInfo = ();
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type BlockNumberProvider = System;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, INITIAL_BALANCE),
				(ALICE, BSX, INITIAL_BALANCE),
				(ALICE, KUSD, INITIAL_BALANCE),
				(ALICE, ETH, INITIAL_ETH_BALANCE),
				(BOB, HDX, INITIAL_BALANCE),
				(BOB, BSX, INITIAL_BALANCE),
				(BOB, KUSD, INITIAL_BALANCE),
				(BOB, ETH, INITIAL_ETH_BALANCE),
			],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts.extend_from_slice(&accounts);
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

pub fn set_block_number(n: u64) {
	frame_system::Pallet::<Test>::set_block_number(n);
}

pub fn run_to_sale_start() {
	set_block_number(SALE_START.unwrap());
}

pub fn run_to_sale_end() {
	set_block_number(SALE_END.unwrap() + 1);
}

pub fn generate_trades(
	start: BlockNumber,
	end: BlockNumber,
	sale_rate: u128,
	sell_ratio: u128,
) -> BTreeMap<BlockNumber, (bool, u128)> {
	let mut trades = BTreeMap::new();
	let intervals: u64 = 72;

	let buy_amount = sale_rate / 24;
	let sell_amount = sale_rate / sell_ratio / 24;

	let skip = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
	let sells = vec![19, 20, 21, 33, 34, 35, 48, 49, 50, 62, 63, 64];
	for i in 0..=intervals {
		let block_num = start + (i * ((end - start) / intervals));

		if skip.contains(&i) {
			continue;
		}

		let (is_buy, amount) = if sells.contains(&i) {
			(false, sell_amount)
		} else {
			(true, buy_amount)
		};

		trades.insert(block_num, (is_buy, amount));
	}
	trades
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}
