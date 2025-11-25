mod tests;
mod utils;

use crate::tests::utils::{acct, bounded_chain_id};
use crate::{self as pallet_dispenser, *};

use frame_support::{
	parameter_types,
	traits::{Currency as CurrencyTrait, Nothing},
	PalletId,
};
use frame_system as system;
use orml_traits::parameter_type_with_key;
use orml_traits::MultiCurrency;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use sp_core::H256;
use sp_runtime::{traits::Verify, MultiSignature};
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	AccountId32, BuildStorage,
};

extern crate alloc;

pub type NamedReserveIdentifier = [u8; 8];
pub type Amount = i128;
pub const HDX: AssetId = 0;

pub const MIN_WEI_BALANCE: u128 = 1_000_000_000_000_000_000_000;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Currencies: pallet_currencies,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Signet: pallet_signet,
		Dispenser: pallet_dispenser,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = frame_system::mocking::MockBlock<Test>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = ();
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

parameter_types! {
	pub const SignetPalletId: PalletId = PalletId(*b"py/signt");
	pub const MaxChainIdLength: u32 = 128;

	pub const MaxReserves: u32 = 50;

	pub const ExistentialDeposit: u128 = 1;

	pub const HDXAssetId: AssetId = HDX;

  pub const TreasuryPalletId: PalletId = PalletId(*b"py/treas");
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

parameter_types! {
	pub TreasuryAccount: AccountId32 = TreasuryPalletId::get().into_account_truncating();
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = HDXAssetId;
	type WeightInfo = ();
}

impl frame_system::offchain::SigningTypes for Test {
	type Public = <MultiSignature as Verify>::Signer;
	type Signature = MultiSignature;
}

parameter_types! {
	pub const MaxDataLength: u32 = 1024;
	pub const MaxSignatureDeposit: u32 = 0;
}

impl pallet_signet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = SignetPalletId;
	type MaxChainIdLength = MaxChainIdLength;
	type WeightInfo = ();
	type MaxDataLength = MaxDataLength;
	type UpdateOrigin = frame_system::EnsureRoot<AccountId32>;
	type MaxSignatureDeposit = MaxSignatureDeposit;
}

parameter_types! {
	pub const DispenserPalletId: PalletId = PalletId(*b"py/erc20");
	pub const SigEthFaucetDispenserFee: u128 = 500;
	pub const SigEthFaucetMaxDispense: u128 = 1_000_000_000;
	pub const SigEthFaucetMinRequest: u128 = 100;
	pub const SigEthFaucetFeeAssetId: AssetId = 1;
	pub const SigEthFaucetFaucetAssetId: AssetId = 2;
	pub const SigEthMinFaucetThreshold: u128 = 1;

}

// MPC “root signer” (Ethereum address expected to sign Signet responses)
pub struct SigEthFaucetMpcRoot;
impl frame_support::traits::Get<[u8; 20]> for SigEthFaucetMpcRoot {
	fn get() -> [u8; 20] {
		[
			0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58, 0x5d, 0xd2, 0x99, 0xe0, 0x3d, 0x12, 0xfa, 0x42,
			0x93, 0xbc,
		]
	}
}

impl frame_system::offchain::SendTransactionTypes<RuntimeCall> for Test {
	type OverarchingCall = RuntimeCall;
	type Extrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
}

impl pallet_dispenser::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type VaultPalletId = DispenserPalletId;
	type Currency = FungibleCurrencies<Test>;
	type MinimumRequestAmount = SigEthFaucetMinRequest;
	type MaxDispenseAmount = SigEthFaucetMaxDispense;
	type DispenserFee = SigEthFaucetDispenserFee;
	type FeeAsset = SigEthFaucetFeeAssetId;
	type FaucetAsset = SigEthFaucetFaucetAssetId;
	type TreasuryAddress = TreasuryAccount;
	type FaucetAddress = SigEthFaucetMpcRoot;
	type MinFaucetEthThreshold = SigEthMinFaucetThreshold;
	type WeightInfo = crate::weights::WeightInfo<Test>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let alice = &acct(1);
	let bob = &acct(2);
	let charlie = &acct(3);
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		let _ = Currencies::deposit(1, alice, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(1, bob, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(1, charlie, 1_000_000_000_000_000_000_000);

		let _ = Currencies::deposit(2, alice, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(2, bob, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(2, charlie, 1_000_000_000_000_000_000_000);
		let requester = acct(1);
		let _ = pallet_signet::Pallet::<Test>::initialize(
			RuntimeOrigin::root(),
			requester,
			100,
			bounded_chain_id(b"test-chain".to_vec()),
		);
		let pallet_account = Dispenser::account_id();
		let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&pallet_account, 10_000);
	});
	ext
}
