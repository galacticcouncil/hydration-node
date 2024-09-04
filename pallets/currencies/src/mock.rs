//! Mocks for the currencies module.

#![cfg(test)]

use super::*;
use crate as currencies;
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::AssetKind;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	traits::{AccountIdConversion, IdentityLookup},
	AccountId32, BuildStorage,
};

pub type AccountId = AccountId32;
impl frame_system::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeTask = RuntimeTask;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

type CurrencyId = u32;
type Balance = u64;

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU64<2>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type MaxReserves = ConstU32<2>;
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		3
	};
}

parameter_types! {
	pub DustAccount: AccountId = PalletId(*b"orml/dst").into_account_truncating();
}

pub type ReserveIdentifier = [u8; 8];

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	//type OnDust = orml_tokens::TransferDust<Runtime, DustAccount>; // TODO: implement the hook
	type MaxLocks = ConstU32<100_000>;
	type MaxReserves = ConstU32<100_000>;
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = Nothing;
}

pub const NATIVE_CURRENCY_ID: CurrencyId = 1;
pub const X_TOKEN_ID: CurrencyId = 2;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type Erc20Currency = Erc20Currency<Runtime>;
	type BoundErc20 = Runtime;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type NativeCurrency = NativeCurrencyOf<Runtime>;
pub type AdaptedBasicCurrency = BasicCurrencyAdapter<Runtime, PalletBalances, i64, u64>;

pub struct Erc20Currency<T>(PhantomData<T>);

impl<T: Config> MultiCurrency<T::AccountId> for Erc20Currency<T>
where
	T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	type CurrencyId = EvmAddress;
	type Balance = Balance;

	fn minimum_balance(_currency_id: Self::CurrencyId) -> Self::Balance {
		todo!()
	}

	fn total_issuance(_currency_id: Self::CurrencyId) -> Self::Balance {
		todo!()
	}

	fn total_balance(_currency_id: Self::CurrencyId, _who: &T::AccountId) -> Self::Balance {
		todo!()
	}

	fn free_balance(_currency_id: Self::CurrencyId, _who: &T::AccountId) -> Self::Balance {
		todo!()
	}

	fn ensure_can_withdraw(
		_currency_id: Self::CurrencyId,
		_who: &T::AccountId,
		_amount: Self::Balance,
	) -> DispatchResult {
		todo!()
	}

	fn transfer(
		_currency_id: Self::CurrencyId,
		_from: &T::AccountId,
		_to: &T::AccountId,
		_amount: Self::Balance,
	) -> DispatchResult {
		todo!()
	}

	fn deposit(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> DispatchResult {
		todo!()
	}

	fn withdraw(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> DispatchResult {
		todo!()
	}

	fn can_slash(_currency_id: Self::CurrencyId, _who: &T::AccountId, _value: Self::Balance) -> bool {
		todo!()
	}

	fn slash(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> Self::Balance {
		todo!()
	}
}

impl hydradx_traits::Inspect for Runtime {
	type AssetId = CurrencyId;
	type Location = ();

	fn is_sufficient(_id: Self::AssetId) -> bool {
		todo!()
	}

	fn exists(_id: Self::AssetId) -> bool {
		todo!()
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		todo!()
	}

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		todo!()
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		todo!()
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		todo!()
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		todo!()
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		todo!()
	}
}

impl BoundErc20 for Runtime {
	fn contract_address(_id: Self::AssetId) -> Option<EvmAddress> {
		None
	}
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime
	{
		System: frame_system,
		Currencies: currencies,
		Tokens: orml_tokens,
		PalletBalances: pallet_balances,
	}
);

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const EVA: AccountId = AccountId32::new([5u8; 32]);
pub const ID_1: LockIdentifier = *b"1       ";
pub const RID_1: ReserveIdentifier = [1u8; 8];
pub const RID_2: ReserveIdentifier = [2u8; 8];

#[derive(Default)]
pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn one_hundred_for_alice_n_bob(self) -> Self {
		self.balances(vec![
			(ALICE, NATIVE_CURRENCY_ID, 100),
			(BOB, NATIVE_CURRENCY_ID, 100),
			(ALICE, X_TOKEN_ID, 100),
			(BOB, X_TOKEN_ID, 100),
		])
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == NATIVE_CURRENCY_ID)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != NATIVE_CURRENCY_ID)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
