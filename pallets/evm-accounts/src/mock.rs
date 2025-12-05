use crate as pallet_evm_accounts;
use crate::{Balance, Config, EvmNonceProvider, Signature};
use frame_support::{parameter_types, BoundedVec};
use frame_support::sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	BuildStorage,
};
use frame_support::traits::Everything;
use frame_support::PalletId;
use frame_support::dispatch::DispatchResult;
use frame_system::{EnsureRoot, EnsureSigned};
use hydradx_traits::evm::InspectEvmAccounts;
use hydradx_traits::AccountFeeCurrency;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use orml_traits::parameter_type_with_key;
pub use sp_core::{H160, H256, U256};
use std::cell::RefCell;
use std::collections::HashMap;

pub type AssetId = u32;
pub type Amount = i128;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type Block = frame_system::mocking::MockBlock<Test>;
pub type NamedReserveIdentifier = [u8; 8];
type AssetLocation = u8;

pub const ONE: Balance = 1_000_000_000_000;
pub const INITIAL_BALANCE: Balance = 1_000_000_000_000 * ONE;

pub const ALICE: AccountId = AccountId::new([1; 32]);

pub const HDX: AssetId = 0;
pub const DOT: AssetId = 3;

thread_local! {
	pub static NONCE: RefCell<HashMap<H160, U256>> = RefCell::new(HashMap::default());
	pub static FEE_ASSET: RefCell<HashMap<AccountId, AssetId>> = RefCell::new(HashMap::default());
}

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 Tokens: orml_tokens,
		 AssetRegistry: pallet_asset_registry,
		 EVMAccounts: pallet_evm_accounts,
	 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const NativeAssetId: AssetId = HDX;
}

pub struct EvmNonceProviderMock;
impl EvmNonceProvider for EvmNonceProviderMock {
	fn get_nonce(evm_address: H160) -> U256 {
		NONCE
			.with(|v| v.borrow().get(&evm_address).copied())
			.unwrap_or_default()
	}
}

pub struct FeeCurrencyMock;
impl AccountFeeCurrency<AccountId> for FeeCurrencyMock {
	type AssetId = AssetId;

	fn get(a: &AccountId) -> Self::AssetId {
		FEE_ASSET
			.with(|v| v.borrow().get(&a).copied())
			.unwrap_or_default()
	}
	fn set(who: &AccountId, asset_id: Self::AssetId) -> DispatchResult {
		FEE_ASSET.with(|v| {
			v.borrow_mut().insert(who.clone(), asset_id);
		});
		Ok(())
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type FeeMultiplier = sp_core::ConstU32<10>;
	type EvmNonceProvider = EvmNonceProviderMock;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type AssetId = AssetId;
	type Currency = FungibleCurrencies<Test>;
	type ExistentialDeposits = ExistentialDeposits;
	type FeeCurrency = FeeCurrencyMock;
	type WeightInfo = ();
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
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_asset_id: AssetId| -> Balance {
		1
	};
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxReserves: u32 = 50;
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
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
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = HDXAssetId;
	type RegistryInspect = MockBoundErc20<Test>;
	type WeightInfo = ();
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = NamedReserveIdentifier;
	type DustRemovalWhitelist = Everything;
}

parameter_types! {
	#[derive(PartialEq, Debug)]
	pub RegistryStringLimit: u32 = 100;
	#[derive(PartialEq, Debug)]
	pub MinRegistryStringLimit: u32 = 2;
	pub const SequentialIdOffset: u32 = 1_000_000;
}


impl pallet_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = EnsureRoot<AccountId>;
	type Currency = Tokens;
	type UpdateOrigin = EnsureSigned<AccountId>;
	type AssetId = AssetId;
	type AssetNativeLocation = AssetLocation;
	type StringLimit = RegistryStringLimit;
	type MinStringLimit = MinRegistryStringLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type RegExternalWeightMultiplier = frame_support::traits::ConstU64<1>;
	type RegisterAssetHook = ();
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		NONCE.with(|v| {
			v.borrow_mut().clear();
		});
		FEE_ASSET.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![(ALICE, HDX, INITIAL_BALANCE)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let registered_assets = vec![
			(
				Some(DOT),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DOT".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DOT".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
		];

		pallet_asset_registry::GenesisConfig::<Test> {
			registered_assets,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();
		r.execute_with(|| System::set_block_number(1));
		r
	}

	pub fn with_non_zero_nonce(self, account_id: AccountId) -> Self {
		let evm_address = EVMAccounts::evm_address(&account_id);
		NONCE.with(|v| {
			let mut m = v.borrow_mut();
			m.insert(evm_address, U256::one());
		});
		self
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}
