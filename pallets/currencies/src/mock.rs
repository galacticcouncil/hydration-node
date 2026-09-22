//! Mocks for the currencies module.

#![cfg(test)]

use super::*;
use crate as currencies;
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	traits::{AccountIdConversion, IdentityLookup},
	AccountId32, BuildStorage,
};
use std::cell::RefCell;
use std::collections::BTreeMap;

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
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
	type ExtensionsWeightInfo = ();
}

pub type CurrencyId = u32;
pub type Balance = u64;

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
	type DoneSlashHandler = ();
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
/// Asset bound to `ERC20_CONTRACT` - the only one `TestBoundErc20` reports as an erc20.
pub const ERC20_TOKEN_ID: CurrencyId = 3;
pub const ERC20_CONTRACT: EvmAddress = EvmAddress::repeat_byte(0xee);

thread_local! {
	static ERC20_BALANCES: RefCell<BTreeMap<(EvmAddress, AccountId), Balance>> = const { RefCell::new(BTreeMap::new()) };
	/// Set to make every erc20 transfer fail, standing in for a reverting evm call.
	static ERC20_TRANSFERS_REVERT: RefCell<bool> = const { RefCell::new(false) };
}

pub fn set_erc20_balance(who: &AccountId, amount: Balance) {
	ERC20_BALANCES.with(|b| b.borrow_mut().insert((ERC20_CONTRACT, who.clone()), amount));
}

pub fn revert_erc20_transfers(revert: bool) {
	ERC20_TRANSFERS_REVERT.with(|f| *f.borrow_mut() = revert);
}

/// Ledger-backed stand-in for the real erc20 contract, so tests can see custody move.
pub struct TestErc20Currency;
impl MultiCurrency<AccountId> for TestErc20Currency {
	type CurrencyId = EvmAddress;
	type Balance = Balance;

	fn minimum_balance(_currency_id: Self::CurrencyId) -> Self::Balance {
		0
	}

	fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
		ERC20_BALANCES.with(|b| {
			b.borrow()
				.iter()
				.filter(|((c, _), _)| *c == currency_id)
				.map(|(_, v)| *v)
				.sum()
		})
	}

	fn total_balance(currency_id: Self::CurrencyId, who: &AccountId) -> Self::Balance {
		Self::free_balance(currency_id, who)
	}

	fn free_balance(currency_id: Self::CurrencyId, who: &AccountId) -> Self::Balance {
		ERC20_BALANCES.with(|b| b.borrow().get(&(currency_id, who.clone())).copied().unwrap_or(0))
	}

	fn ensure_can_withdraw(currency_id: Self::CurrencyId, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		if Self::free_balance(currency_id, who) < amount {
			return Err(crate::Error::<Runtime>::BalanceTooLow.into());
		}
		Ok(())
	}

	fn transfer(
		currency_id: Self::CurrencyId,
		from: &AccountId,
		to: &AccountId,
		amount: Self::Balance,
		_existence_requirement: ExistenceRequirement,
	) -> DispatchResult {
		if ERC20_TRANSFERS_REVERT.with(|f| *f.borrow()) {
			return Err(DispatchError::Other("erc20 reverted"));
		}
		Self::ensure_can_withdraw(currency_id, from, amount)?;
		Self::withdraw(currency_id, from, amount, ExistenceRequirement::AllowDeath)?;
		Self::deposit(currency_id, to, amount)
	}

	fn deposit(currency_id: Self::CurrencyId, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		ERC20_BALANCES.with(|b| {
			let mut b = b.borrow_mut();
			let e = b.entry((currency_id, who.clone())).or_default();
			*e = e
				.checked_add(amount)
				.ok_or(DispatchError::Arithmetic(sp_runtime::ArithmeticError::Overflow))?;
			Ok(())
		})
	}

	fn withdraw(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		amount: Self::Balance,
		_existence_requirement: ExistenceRequirement,
	) -> DispatchResult {
		ERC20_BALANCES.with(|b| {
			let mut b = b.borrow_mut();
			let e = b.entry((currency_id, who.clone())).or_default();
			*e = e.checked_sub(amount).ok_or(crate::Error::<Runtime>::BalanceTooLow)?;
			Ok(())
		})
	}

	fn can_slash(_currency_id: Self::CurrencyId, _who: &AccountId, _value: Self::Balance) -> bool {
		false
	}

	fn slash(_currency_id: Self::CurrencyId, _who: &AccountId, _amount: Self::Balance) -> Self::Balance {
		0
	}
}

pub struct TestBoundErc20;
impl hydradx_traits::Inspect for TestBoundErc20 {
	type AssetId = CurrencyId;
	type Location = ();

	fn is_sufficient(_id: Self::AssetId) -> bool {
		false
	}

	fn exists(_id: Self::AssetId) -> bool {
		false
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		None
	}

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		None
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		false
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		None
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		None
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		None
	}
}

impl BoundErc20 for TestBoundErc20 {
	fn contract_address(id: Self::AssetId) -> Option<EvmAddress> {
		(id == ERC20_TOKEN_ID).then_some(ERC20_CONTRACT)
	}
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
	pub const ReserveAccount: AccountId32 = AccountId32::new([9u8; 32]);
}

impl Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type Erc20Currency = TestErc20Currency;
	type BoundErc20 = TestBoundErc20;
	type ReserveAccount = ReserveAccount;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type RegistryInspect = TestBoundErc20;
	type EgressHandler = MockEgressHandler<Runtime>;
	type WeightInfo = ();
}
pub type NativeCurrency = NativeCurrencyOf<Runtime>;
pub type AdaptedBasicCurrency = BasicCurrencyAdapter<Runtime, PalletBalances, i64, u64>;

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
	erc20_balances: Vec<(AccountId, Balance)>,
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	/// Seeds the erc20 ledger behind `ERC20_TOKEN_ID`.
	pub fn erc20_balances(mut self, balances: Vec<(AccountId, Balance)>) -> Self {
		self.erc20_balances = balances;
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
		// thread_locals outlive a single test on a reused thread.
		ERC20_BALANCES.with(|b| b.borrow_mut().clear());
		revert_erc20_transfers(false);
		for (who, amount) in self.erc20_balances.iter() {
			set_erc20_balance(who, *amount);
		}

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
			dev_accounts: None,
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
