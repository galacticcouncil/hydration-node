use crate as pallet_fee_processor;
use frame_support::{
	parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage, DispatchError, Permill,
	},
	traits::{Everything, Nothing},
	PalletId,
};
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::gigahdx::{Convert as ConvertTrait, FeeReceiver};
use hydradx_traits::price::PriceProvider;
use orml_traits::parameter_type_with_key;
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_currencies::{BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use sp_core::H256;
use sp_runtime::traits::AccountIdConversion;
use sp_std::cell::RefCell;
use sp_std::vec::Vec;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DOT: AssetId = 2;
pub const DAI: AssetId = 3;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const FEE_SOURCE: AccountId = 100;

pub const STAKING_POT: AccountId = 200;
pub const REFERRALS_POT: AccountId = 201;

// HDX path pots
pub const HDX_STAKING_POT: AccountId = 200;
pub const HDX_GIGAPOT: AccountId = 202;
pub const HDX_REWARD_POT: AccountId = 203;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: pallet_currencies,
		FeeProcessor: pallet_fee_processor,
	}
);

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
	pub const MaxReserves: u32 = 50;
	pub const NativeAssetId: AssetId = HDX;
	pub const LrnaAssetId: AssetId = LRNA;
	pub const FeeProcessorPalletId: PalletId = PalletId(*b"feeproc/");
	pub const MaxConversionsPerBlock: u32 = 5;
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
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
	type BlockHashCount = frame_support::traits::ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
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
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
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

impl pallet_currencies::Config for Test {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = NativeAssetId;
	type RegistryInspect = MockBoundErc20<Test>;
	type EgressHandler = pallet_currencies::MockEgressHandler<Test>;
	type WeightInfo = ();
}

// --- Mock Convert ---
thread_local! {
	static CONVERT_RESULT: RefCell<Option<Balance>> = RefCell::new(Some(1000 * ONE));
	static CONVERT_CALLS: RefCell<Vec<(AccountId, AssetId, AssetId, Balance)>> = RefCell::new(Vec::new());
	static PRE_DEPOSIT_CALLS: RefCell<Vec<(AccountId, Balance)>> = RefCell::new(Vec::new());
	static DEPOSIT_CALLS: RefCell<Vec<Balance>> = RefCell::new(Vec::new());
	static HDX_PRE_DEPOSIT_CALLS: RefCell<Vec<(AccountId, Balance)>> = RefCell::new(Vec::new());
	static HDX_DEPOSIT_CALLS: RefCell<Vec<Balance>> = RefCell::new(Vec::new());
	static HDX_GIGAPOT_PRE_DEPOSIT_CALLS: RefCell<Vec<(AccountId, Balance)>> = RefCell::new(Vec::new());
	static HDX_GIGAPOT_DEPOSIT_CALLS: RefCell<Vec<Balance>> = RefCell::new(Vec::new());
	static HDX_REWARD_POT_PRE_DEPOSIT_CALLS: RefCell<Vec<(AccountId, Balance)>> = RefCell::new(Vec::new());
	static HDX_REWARD_POT_DEPOSIT_CALLS: RefCell<Vec<Balance>> = RefCell::new(Vec::new());
	static PRE_DEPOSIT_FAIL: RefCell<bool> = RefCell::new(false);
}

pub struct MockConvert;

impl ConvertTrait<AccountId, AssetId, Balance> for MockConvert {
	type Error = DispatchError;

	fn convert(
		who: AccountId,
		asset_from: AssetId,
		asset_to: AssetId,
		amount: Balance,
	) -> Result<Balance, Self::Error> {
		CONVERT_CALLS.with(|c| c.borrow_mut().push((who, asset_from, asset_to, amount)));
		CONVERT_RESULT.with(|r| r.borrow().ok_or(DispatchError::Other("ConvertFailed")))
	}
}

pub fn set_convert_result(result: Option<Balance>) {
	CONVERT_RESULT.with(|r| *r.borrow_mut() = result);
}

pub fn convert_calls() -> Vec<(AccountId, AssetId, AssetId, Balance)> {
	CONVERT_CALLS.with(|c| c.borrow().clone())
}

pub fn pre_deposit_calls() -> Vec<(AccountId, Balance)> {
	PRE_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn deposit_calls() -> Vec<Balance> {
	DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn hdx_pre_deposit_calls() -> Vec<(AccountId, Balance)> {
	HDX_PRE_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn hdx_deposit_calls() -> Vec<Balance> {
	HDX_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn hdx_gigapot_pre_deposit_calls() -> Vec<(AccountId, Balance)> {
	HDX_GIGAPOT_PRE_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn hdx_gigapot_deposit_calls() -> Vec<Balance> {
	HDX_GIGAPOT_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn hdx_reward_pot_pre_deposit_calls() -> Vec<(AccountId, Balance)> {
	HDX_REWARD_POT_PRE_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn hdx_reward_pot_deposit_calls() -> Vec<Balance> {
	HDX_REWARD_POT_DEPOSIT_CALLS.with(|c| c.borrow().clone())
}

pub fn set_pre_deposit_fail(fail: bool) {
	PRE_DEPOSIT_FAIL.with(|f| *f.borrow_mut() = fail);
}

// --- Mock PriceProvider ---
thread_local! {
	static MOCK_PRICE: RefCell<Option<EmaPrice>> = RefCell::new(Some(EmaPrice::new(2, 1)));
}

pub struct MockPriceProvider;

impl PriceProvider<AssetId> for MockPriceProvider {
	type Price = EmaPrice;

	fn get_price(_asset_a: AssetId, _asset_b: AssetId) -> Option<Self::Price> {
		MOCK_PRICE.with(|p| *p.borrow())
	}
}

pub fn set_mock_price(price: Option<EmaPrice>) {
	MOCK_PRICE.with(|p| *p.borrow_mut() = price);
}

// --- Mock FeeReceivers ---

pub struct StakingFeeReceiver;

impl FeeReceiver<AccountId, Balance> for StakingFeeReceiver {
	type Error = DispatchError;

	fn destination() -> AccountId {
		STAKING_POT
	}

	fn percentage() -> Permill {
		Permill::from_percent(70)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().push((trader, amount)));
		if PRE_DEPOSIT_FAIL.with(|f| *f.borrow()) {
			return Err(DispatchError::Other("pre_deposit_failed"));
		}
		Ok(())
	}

	fn on_fee_received(amount: Balance) -> Result<(), Self::Error> {
		DEPOSIT_CALLS.with(|c| c.borrow_mut().push(amount));
		Ok(())
	}
}

pub struct ReferralsFeeReceiver;

impl FeeReceiver<AccountId, Balance> for ReferralsFeeReceiver {
	type Error = DispatchError;

	fn destination() -> AccountId {
		REFERRALS_POT
	}

	fn percentage() -> Permill {
		Permill::from_percent(30)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().push((trader, amount)));
		Ok(())
	}

	fn on_fee_received(amount: Balance) -> Result<(), Self::Error> {
		DEPOSIT_CALLS.with(|c| c.borrow_mut().push(amount));
		Ok(())
	}
}

// --- HDX-specific FeeReceivers (70/20/10 split, no referrals) ---

pub struct HdxGigaHdxFeeReceiver;

impl FeeReceiver<AccountId, Balance> for HdxGigaHdxFeeReceiver {
	type Error = DispatchError;

	fn destination() -> AccountId {
		HDX_GIGAPOT
	}

	fn percentage() -> Permill {
		Permill::from_percent(70)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		HDX_GIGAPOT_PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().push((trader, amount)));
		Ok(())
	}

	fn on_fee_received(amount: Balance) -> Result<(), Self::Error> {
		HDX_GIGAPOT_DEPOSIT_CALLS.with(|c| c.borrow_mut().push(amount));
		Ok(())
	}
}

pub struct HdxGigaRewardFeeReceiver;

impl FeeReceiver<AccountId, Balance> for HdxGigaRewardFeeReceiver {
	type Error = DispatchError;

	fn destination() -> AccountId {
		HDX_REWARD_POT
	}

	fn percentage() -> Permill {
		Permill::from_percent(20)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		HDX_REWARD_POT_PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().push((trader, amount)));
		Ok(())
	}

	fn on_fee_received(amount: Balance) -> Result<(), Self::Error> {
		HDX_REWARD_POT_DEPOSIT_CALLS.with(|c| c.borrow_mut().push(amount));
		Ok(())
	}
}

pub struct HdxStakingFeeReceiver;

impl FeeReceiver<AccountId, Balance> for HdxStakingFeeReceiver {
	type Error = DispatchError;

	fn destination() -> AccountId {
		HDX_STAKING_POT
	}

	fn percentage() -> Permill {
		Permill::from_percent(10)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		HDX_PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().push((trader, amount)));
		Ok(())
	}

	fn on_fee_received(amount: Balance) -> Result<(), Self::Error> {
		HDX_DEPOSIT_CALLS.with(|c| c.borrow_mut().push(amount));
		Ok(())
	}
}

impl pallet_fee_processor::Config for Test {
	type AssetId = AssetId;
	type Currency = FungibleCurrencies<Test>;
	type Convert = MockConvert;
	type PriceProvider = MockPriceProvider;
	type PalletId = FeeProcessorPalletId;
	type HdxAssetId = NativeAssetId;
	type LrnaAssetId = LrnaAssetId;
	type MaxConversionsPerBlock = MaxConversionsPerBlock;
	type FeeReceivers = (StakingFeeReceiver, ReferralsFeeReceiver);
	type HdxFeeReceivers = (HdxGigaHdxFeeReceiver, HdxGigaRewardFeeReceiver, HdxStakingFeeReceiver);
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 100_000 * ONE),
				(BOB, HDX, 50_000 * ONE),
				(FEE_SOURCE, HDX, 100_000 * ONE),
				(FEE_SOURCE, DOT, 100_000 * ONE),
				(FEE_SOURCE, DAI, 100_000 * ONE),
				// ED for pots
				(STAKING_POT, HDX, ONE),
				(REFERRALS_POT, HDX, ONE),
				(HDX_GIGAPOT, HDX, ONE),
				(HDX_REWARD_POT, HDX, ONE),
				// ED for fee processor pot
				(FeeProcessor::pot_account_id(), HDX, ONE),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let native_endowed: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|(_, asset, _)| *asset == HDX)
			.map(|(who, _, amount)| (*who, *amount))
			.collect();

		pallet_balances::GenesisConfig::<Test> {
			balances: native_endowed,
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let non_native: Vec<(AccountId, AssetId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|(_, asset, _)| *asset != HDX)
			.cloned()
			.collect();

		orml_tokens::GenesisConfig::<Test> { balances: non_native }
			.assimilate_storage(&mut t)
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
			// Reset thread_local state
			CONVERT_RESULT.with(|r| *r.borrow_mut() = Some(1000 * ONE));
			CONVERT_CALLS.with(|c| c.borrow_mut().clear());
			PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			HDX_PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			HDX_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			HDX_GIGAPOT_PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			HDX_GIGAPOT_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			HDX_REWARD_POT_PRE_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			HDX_REWARD_POT_DEPOSIT_CALLS.with(|c| c.borrow_mut().clear());
			PRE_DEPOSIT_FAIL.with(|f| *f.borrow_mut() = false);
			MOCK_PRICE.with(|p| *p.borrow_mut() = Some(EmaPrice::new(2, 1)));
		});
		ext
	}
}
