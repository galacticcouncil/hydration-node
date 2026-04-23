use crate as pallet_gigahdx;
use crate::*;
use frame_support::{
	parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage,
	},
	traits::{Everything, Nothing},
	PalletId,
};
use orml_traits::parameter_type_with_key;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use sp_core::H256;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const ST_HDX: AssetId = 100;
pub const GIGAHDX: AssetId = 101;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: pallet_currencies,
		GigaHdx: pallet_gigahdx,
	}
);

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
	pub const ExistentialDeposit: u128 = 1;
}

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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
	type ExtensionsWeightInfo = ();
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = NamedReserveIdentifier;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

impl orml_tokens::Config for Test {
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = frame_support::traits::ConstU32<50>;
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl pallet_currencies::Config for Test {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = HDXAssetId;
	type RegistryInspect = MockBoundErc20<Test>;
	type EgressHandler = pallet_currencies::MockEgressHandler<Test>;
	type WeightInfo = ();
}

parameter_types! {
	pub const GigaHdxPalletId: PalletId = PalletId(*b"gigahdx!");
	pub const HdxAssetId: AssetId = HDX;
	pub const StHdxAssetId: AssetId = ST_HDX;
	pub const GigaHdxAssetId: AssetId = GIGAHDX;
	pub const CooldownPeriod: u64 = 100; // 100 blocks for tests
	pub const MinStake: Balance = ONE;
	pub const MinUnstake: Balance = ONE;
	pub const MaxUnstakePositions: u32 = 10;
}

impl pallet_gigahdx::Config for Test {
	type Currency = FungibleCurrencies<Test>;
	type LockableCurrency = Currencies;
	type MoneyMarket = (); // No-op: supply/withdraw are identity
	type Hooks = (); // No-op: all hooks pass
	type PalletId = GigaHdxPalletId;
	type HdxAssetId = HdxAssetId;
	type StHdxAssetId = StHdxAssetId;
	type GigaHdxAssetId = GigaHdxAssetId;
	type CooldownPeriod = CooldownPeriod;
	type MinStake = MinStake;
	type MinUnstake = MinUnstake;
	type MaxUnstakePositions = MaxUnstakePositions;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1_000 * ONE),
				(BOB, HDX, 1_000 * ONE),
				(CHARLIE, HDX, 1_000 * ONE),
			],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		// Native (HDX) goes through pallet_balances
		let native_balances: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|(_, id, _)| *id == HDX)
			.map(|(acc, _, bal)| (*acc, *bal))
			.collect();

		pallet_balances::GenesisConfig::<Test> {
			balances: native_balances,
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		// Non-native tokens go through orml_tokens (exclude native HDX)
		let non_native_balances: Vec<(AccountId, AssetId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|(_, id, _)| *id != HDX)
			.cloned()
			.collect();

		orml_tokens::GenesisConfig::<Test> {
			balances: non_native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			System::set_block_number(1);
		});

		ext
	}
}

/// Advance to given block.
pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		System::set_block_number(System::block_number() + 1);
	}
}
