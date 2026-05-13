// SPDX-License-Identifier: Apache-2.0

#![cfg(test)]

use crate as pallet_gigahdx;

use frame_support::sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError,
};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64, Everything, LockIdentifier},
	PalletId,
};
use frame_system::EnsureRoot;
use hydradx_traits::gigahdx::MoneyMarketOperations;
use orml_traits::parameter_type_with_key;
use primitives::{AssetId, Balance};
use sp_core::H256;
use std::cell::RefCell;
use std::collections::HashMap;

pub type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;

#[allow(dead_code)]
pub const HDX: AssetId = 0;
pub const ST_HDX: AssetId = 670;
pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const TREASURY: AccountId = 99;

pub const GIGAHDX_LOCK_ID: LockIdentifier = *b"ghdxlock";

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		GigaHdx: pallet_gigahdx,
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
	type AccountId = AccountId;
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
	type ExtensionsWeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
	pub const MaxLocks: u32 = 20;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

parameter_type_with_key! {
	pub StHdxExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

impl orml_tokens::Config for Test {
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = StHdxExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ConstU32<10>;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

// ---------- TestMoneyMarket ----------

thread_local! {
	pub static MM_BALANCES: RefCell<HashMap<AccountId, Balance>> = RefCell::new(HashMap::new());
	/// When set, `supply` returns `input * num / den` (rounding test). 1/1 = identity.
	pub static MM_SUPPLY_ROUND_NUM: RefCell<u128> = const { RefCell::new(1) };
	pub static MM_SUPPLY_ROUND_DEN: RefCell<u128> = const { RefCell::new(1) };
	/// When true, `supply` errors.
	pub static MM_SUPPLY_FAILS: RefCell<bool> = const { RefCell::new(false) };
	pub static MM_WITHDRAW_FAILS: RefCell<bool> = const { RefCell::new(false) };
}

pub struct TestMoneyMarket;

impl TestMoneyMarket {
	pub fn reset() {
		MM_BALANCES.with(|m| m.borrow_mut().clear());
		MM_SUPPLY_ROUND_NUM.with(|v| *v.borrow_mut() = 1);
		MM_SUPPLY_ROUND_DEN.with(|v| *v.borrow_mut() = 1);
		MM_SUPPLY_FAILS.with(|v| *v.borrow_mut() = false);
		MM_WITHDRAW_FAILS.with(|v| *v.borrow_mut() = false);
	}
	pub fn set_supply_rounding(num: u128, den: u128) {
		MM_SUPPLY_ROUND_NUM.with(|v| *v.borrow_mut() = num);
		MM_SUPPLY_ROUND_DEN.with(|v| *v.borrow_mut() = den);
	}
	pub fn fail_supply() {
		MM_SUPPLY_FAILS.with(|v| *v.borrow_mut() = true);
	}
	pub fn fail_withdraw() {
		MM_WITHDRAW_FAILS.with(|v| *v.borrow_mut() = true);
	}
}

impl MoneyMarketOperations<AccountId, AssetId, Balance> for TestMoneyMarket {
	fn supply(who: &AccountId, _asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		if MM_SUPPLY_FAILS.with(|v| *v.borrow()) {
			return Err(DispatchError::Other("MM supply failed"));
		}
		let num = MM_SUPPLY_ROUND_NUM.with(|v| *v.borrow());
		let den = MM_SUPPLY_ROUND_DEN.with(|v| *v.borrow());
		let actual = amount.saturating_mul(num) / den;
		MM_BALANCES.with(|m| *m.borrow_mut().entry(*who).or_default() += actual);
		Ok(actual)
	}

	fn withdraw(who: &AccountId, _asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		if MM_WITHDRAW_FAILS.with(|v| *v.borrow()) {
			return Err(DispatchError::Other("MM withdraw failed"));
		}
		MM_BALANCES.with(|m| {
			let mut map = m.borrow_mut();
			let bal = map.entry(*who).or_default();
			*bal = bal.saturating_sub(amount);
		});
		Ok(amount)
	}

	fn balance_of(who: &AccountId) -> Balance {
		MM_BALANCES.with(|m| *m.borrow().get(who).unwrap_or(&0))
	}
}

// ---------- pallet-gigahdx config ----------

parameter_types! {
	pub const StHdxAssetIdConst: AssetId = ST_HDX;
	pub const GigaHdxPalletId: PalletId = PalletId(*b"gigahdx!");
	pub const GigaHdxLockId: LockIdentifier = GIGAHDX_LOCK_ID;
	pub const GigaHdxMinStake: Balance = ONE; // 1 HDX
	pub const GigaHdxCooldownPeriod: u64 = 100; // 100 blocks
	pub const GigaHdxMaxPendingUnstakes: u32 = 10;
}

impl pallet_gigahdx::Config for Test {
	type NativeCurrency = Balances;
	type MultiCurrency = Tokens;
	type StHdxAssetId = StHdxAssetIdConst;
	type MoneyMarket = TestMoneyMarket;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type PalletId = GigaHdxPalletId;
	type LockId = GigaHdxLockId;
	type MinStake = GigaHdxMinStake;
	type CooldownPeriod = GigaHdxCooldownPeriod;
	type MaxPendingUnstakes = GigaHdxMaxPendingUnstakes;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

// ---------- Test helpers ----------

/// Flattened view of a single pending-unstake entry, used by tests that
/// assume exactly one position exists.
#[allow(dead_code)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PendingView {
	pub id: u64,
	pub amount: Balance,
	pub expires_at: u64,
}

/// Return the only pending-unstake entry for `who`. Panics if zero or more
/// than one. Multi-position tests should iterate the storage directly.
#[allow(dead_code)]
pub fn only_pending(who: AccountId) -> PendingView {
	let mut iter = pallet_gigahdx::PendingUnstakes::<Test>::iter_prefix(who);
	let (id, p) = iter.next().expect("expected one pending position, got none");
	assert!(iter.next().is_none(), "expected exactly one pending position");
	PendingView {
		id,
		amount: p.amount,
		expires_at: id + GigaHdxCooldownPeriod::get(),
	}
}

#[allow(dead_code)]
pub fn pending_count(who: AccountId) -> u16 {
	pallet_gigahdx::Stakes::<Test>::get(who)
		.map(|s| s.unstaking_count)
		.unwrap_or(0)
}

// ---------- Test ext builder ----------

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, Balance)>,
	pot_balance: Balance,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, 1_000 * ONE), (BOB, 1_000 * ONE), (TREASURY, 1_000 * ONE)],
			pot_balance: 0,
		}
	}
}

impl ExtBuilder {
	pub fn with_pot_balance(mut self, balance: Balance) -> Self {
		self.pot_balance = balance;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut balances = self.endowed_accounts.clone();
		if self.pot_balance > 0 {
			use frame_support::sp_runtime::traits::AccountIdConversion;
			let pot: AccountId = GigaHdxPalletId::get().into_account_truncating();
			balances.push((pot, self.pot_balance));
		}
		pallet_balances::GenesisConfig::<Test> {
			balances,
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			TestMoneyMarket::reset();
			System::set_block_number(1);
		});
		ext
	}
}
